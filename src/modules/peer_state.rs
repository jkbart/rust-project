use crossterm::event::KeyModifiers;
use rand::Rng;
use tokio::io::AsyncReadExt;
use std::collections::HashMap;
use std::path::PathBuf;
use ratatui::prelude::Layout;
use ratatui::prelude::Constraint;
use ratatui::prelude::Rect;
use ratatui::prelude::Buffer;
use ratatui::widgets::Block;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use ratatui::style::{Color, Style};
use ratatui::text::Span;
use ratatui::widgets::Borders;
use ratatui::widgets::Widget;

use ratatui::text::Line;
use tokio::io::AsyncWriteExt;
use unicode_width::UnicodeWidthStr;

use super::{networking::*, protocol::*};
use crate::config::*;

use cli_log::*;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::task::JoinHandle;

use crate::modules::widgets::message_bubble::*;
use crate::modules::widgets::list_component::*;
use crate::modules::tui::AppPosition;

use tokio::sync::mpsc;

use tui_textarea::TextArea;

use copypasta::{ClipboardContext, ClipboardProvider};

pub enum EditorMode {
    Text,
    File,
}

type DownloadedFilesMap = Arc<Mutex<HashMap<u64, mpsc::UnboundedSender<InternalMessage>>>>;
type OwnedFilesMap = Arc<Mutex<HashMap<u64, PathBuf>>>;

pub struct MessageContext {
    pub was_received: bool, // Whether it was sent or received.
    pub message: UserMessage,
}

pub struct PeerState<'a> {
    pub name: String,
    pub addr: SocketAddr,
    render_cache: Option<ListCache<'a>>,

    pub messages: ListComponent<'a, MsgBubble<'a>>,
    pub editor: TextArea<'a>,
    pub editor_mode: EditorMode,
    downloaded_files: DownloadedFilesMap,
    owned_files: OwnedFilesMap,
    conversation_buffer: Arc<Mutex<Vec<MessageContext>>>,
    message_writer_queue: mpsc::UnboundedSender<Message>,
    message_writer_handle: JoinHandle<Result<(), StreamSerializerError>>,
    message_reader_handle: JoinHandle<Result<(), StreamSerializerError>>,
}

impl PeerState<'_> {
    pub fn is_active(&self) -> bool {
        !self.message_writer_handle.is_finished() && !self.message_reader_handle.is_finished()
    }

    pub fn update(&mut self) {
        let mut msg_buffer = self.conversation_buffer.lock().unwrap();
        self.messages.list.extend(msg_buffer.drain(..).map(|mc| {
            MsgBubble::new(
                match mc.was_received {
                    true => Some(self.name.clone()),
                    false => None,
                },
                mc.message,
                match mc.was_received {
                    true => MsgBubbleAllignment::Left,
                    false => MsgBubbleAllignment::Right,
                }
            )
        }));
    }

    pub fn send(&self, msg: Message) {
        let _ = self.message_writer_queue.send(msg);
    }

    pub fn download_file(&self, file_id: u64, file_name: String, file_size: u64, loading_bar: Arc<Mutex<LoadingBar>>) {
        let (tx, rx) = mpsc::unbounded_channel::<InternalMessage>();

        self.downloaded_files.lock().unwrap().insert(file_id, tx);

        tokio::task::spawn(file_downloader(rx, file_name, file_size, loading_bar));

        self.send(Message::Internal(InternalMessage::FileRequest(file_id)));
    }

    pub fn handle_event(&mut self, key: KeyEvent, _current_screen: &mut AppPosition) -> bool {
        if self.messages.is_selected() {
            // Currently listing conversation.
            match key {
                key if key.code == KeyCode::Esc => {
                    if key.kind == crossterm::event::KeyEventKind::Press {
                        self.messages.reset();
                    }
                },
                key if key.code == KeyCode::Up => {
                    if key.kind == crossterm::event::KeyEventKind::Press {
                        self.messages.go_up();
                    }
                },
                key if key.code == KeyCode::Down => {
                    if key.kind == crossterm::event::KeyEventKind::Press {
                        if !self.messages.go_down() {
                            self.messages.reset();
                        }
                    }
                },
                key if key.code == KeyCode::Enter => {
                    if key.kind == crossterm::event::KeyEventKind::Press {
                        let message_bubble = self.messages.get_selected().unwrap();

                        match &message_bubble.message {
                            UserMessage::Text(text) => {
                                ClipboardContext::new().unwrap().set_contents(text.clone()).unwrap();
                            },
                            UserMessage::FileHeader(file_name, file_size, file_id) => {
                                if message_bubble.received_from.is_some() && message_bubble.loading_bar.is_none() {
                                    let loading_bar = Arc::new(Mutex::new(LoadingBar { position: 0, end: 0, changed: true }));
                                    message_bubble.loading_bar = Some(loading_bar.clone());

                                    // Copy fist to allow for mut borrow of self - droping message_bubble.
                                    let file_name = file_name.clone();
                                    let file_size = *file_size;
                                    let file_id = *file_id;

                                    self.download_file(file_id, file_name, file_size, loading_bar);
                                }
                            },
                        }
                    }
                }
                _ => {},
            }
        } else {
            // Currently editing next msg.
            match key {
                key if key.code == KeyCode::Esc => {
                    if key.kind == crossterm::event::KeyEventKind::Press {
                        return true;
                    }
                },
                key if key.code == KeyCode::Up => {
                    if key.kind == crossterm::event::KeyEventKind::Press {
                        self.messages.go_down();    // First select is little diffrent.
                    }
                },
                key if key.code == KeyCode::Tab => {
                    if key.kind == crossterm::event::KeyEventKind::Press {
                        self.editor_mode = match self.editor_mode {
                            EditorMode::Text => EditorMode::File,
                            EditorMode::File => EditorMode::Text,
                        }
                    }
                },
                key if key.code == KeyCode::Enter => {
                    if key.kind == crossterm::event::KeyEventKind::Press {
                        match self.editor_mode {
                            EditorMode::Text => {
                                let msg = Message::User(
                                    UserMessage::Text(self.editor.lines().join("\n")),
                                );

                                self.editor = TextArea::default();

                                self.send(msg);
                            },
                            EditorMode::File => {
                                let file_path = PathBuf::from(&self.editor.lines()[0]);
                                let file_id: u64 = rand::thread_rng().gen();

                                if std::fs::metadata(&file_path).map(|metadata| metadata.is_file()).unwrap_or(false) {
                                    let file_name: String = file_path.file_name().unwrap().to_string_lossy().to_string();
                                    let file_size: u64 = std::fs::metadata(&file_path).map(|metadata| metadata.len()).unwrap_or(0);

                                    self.owned_files.lock().unwrap().insert(file_id, file_path);

                                    self.editor = TextArea::default();

                                    self.send(Message::User(
                                        UserMessage::FileHeader(file_name, file_size, file_id)
                                    ));
                                }
                            }
                        }
                    }
                }
                key if key.code == KeyCode::Char('v') && key.modifiers == KeyModifiers::CONTROL => {
                    #[cfg(not(target_os = "windows"))]
                    {
                        if let Ok(mut clipboard) = ClipboardContext::new() {
                            if let Ok(contents) = clipboard.get_contents() {
                                self.editor.insert_str(contents);
                            }
                        }
                    }
                },
                editor_input => {
                    self.editor.input(editor_input);
                },
            }
        }

        false
    }
}

impl From<ConnectionData> for PeerState<'_> {
    fn from(connection_data: ConnectionData) -> Self {
        let conversation_buffer: Arc<Mutex<Vec<MessageContext>>> = Arc::new(Vec::new().into());

        let (rx_stream, tx_stream) = connection_data.stream.into_split();
        let (tx_queue, rx_queue) = mpsc::unbounded_channel::<Message>();

        let downloaded_files = Arc::new(Mutex::new(HashMap::new()));
        let owned_files = Arc::new(Mutex::new(HashMap::new()));

        let message_reader_handle =
            tokio::task::spawn(message_reader(rx_stream, tx_queue.clone(), conversation_buffer.clone(), downloaded_files.clone(), owned_files.clone()));
        let message_writer_handle = tokio::task::spawn(message_writer(
            tx_stream,
            conversation_buffer.clone(),
            rx_queue,
        ));

        PeerState {
            name: connection_data.peer_name,
            addr: connection_data.peer_address,
            render_cache: None,

            messages: ListComponent::new(ListBegin::Bottom, ListTop::Last),
            editor: TextArea::default(),
            editor_mode: EditorMode::Text,
            downloaded_files,
            owned_files,
            conversation_buffer,
            message_writer_queue: tx_queue,
            message_writer_handle,
            message_reader_handle,
        }
    }
}

async fn message_reader(
    mut stream: tokio::net::tcp::OwnedReadHalf,
    tx_message: mpsc::UnboundedSender<Message>,
    msgs: Arc<Mutex<Vec<MessageContext>>>,
    downloaded_files: DownloadedFilesMap,
    owned_files: OwnedFilesMap,
) -> Result<(), StreamSerializerError> {
    loop {
        let message = Message::read(&mut stream).await?;
        info!("Message received via tcp!");
        match message {
            Message::User(user_message) => {
                msgs.lock().unwrap().push(MessageContext {
                    was_received: true,
                    message: user_message,
                });
            },
            Message::Internal(internal_message) => {
                match internal_message {
                    InternalMessage::FileRequest(id) => {
                        if let Some(file_path) = owned_files.lock().unwrap().get(&id) {
                            tokio::task::spawn(
                                file_uploader(tx_message.clone(), file_path.clone(), id)
                            );
                        }
                    }
                    InternalMessage::FileContent(id, _, _) => {
                        if let Some(tx) = downloaded_files.lock().unwrap().get(&id) {
                            let _ = tx.send(internal_message);
                        }
                    }
                }
            },
        }
    }
}

async fn file_downloader(
    mut packets: mpsc::UnboundedReceiver<InternalMessage>,
    file_name: String,
    file_size: u64, 
    loading_bar: Arc<Mutex<LoadingBar>>,
) {
    let mut file_path = DOWNLOAD_PATH.clone();
    file_path.push(&file_name);

    *loading_bar.lock().unwrap() = LoadingBar {
        position: 0,
        end: file_size,
        changed: true,
    };

    // If files [name, name (1), ..., name (9)] exists in download dir, abandon download.
    'main: for i in 0..10 {
        let mut file = match tokio::fs::OpenOptions::new()
            .create_new(true) // Ensures the file doesn't already exist
            .write(true)
            .open(&file_path)
            .await 
        {
            Ok(file) => file,
            Err(_) => {
                file_path = DOWNLOAD_PATH.clone();
                file_path.push(format!("{} ({})", file_name, i + 1));

                continue;
            }
        };

        let mut byte_cnt = 0;

        while let Some(packet) = packets.recv().await {
            match packet {
                InternalMessage::FileContent(_, byte_idx, bytes) => {
                    if byte_idx != byte_cnt || byte_idx + bytes.len() as u64 > file_size {
                        break 'main;
                    }

                    byte_cnt += bytes.len() as u64;

                    // For some reason writing drop explicitly doenst work. Have to use {} instead.
                    {
                        let mut loading_bar_lock = loading_bar.lock().unwrap();

                        loading_bar_lock.position = byte_cnt;
                        loading_bar_lock.changed = true;
                    }

                    if file.write_all(&bytes).await.is_err() {
                        break 'main;
                    }
                },
                _ => {},
            }
        }
    }
}

async fn file_uploader(
    packets: mpsc::UnboundedSender<Message>,
    file_name: PathBuf,
    file_id: u64, 
) -> std::io::Result<()> {
    // Open the file in read-only mode
    let mut file = tokio::fs::File::open(file_name).await?;

    let mut buffer = vec![0; 4096]; // Buffer size 4096 bytes

    let mut byte_idx = 0;

    loop {
        let n = file.read(&mut buffer).await?;

        if n == 0 {
            break; // End of file
        }

        // Trim the buffer to the size of the data read
        let chunk = buffer[..n].to_vec();

        // Send the chunk through the sender
        let message = Message::Internal(InternalMessage::FileContent(file_id, byte_idx, chunk));
        byte_idx += n as u64;

        if let Err(_) = packets.send(message) {
            break;
        }
    }

    Ok(())
}

async fn message_writer(
    mut stream: tokio::net::tcp::OwnedWriteHalf,
    msgs: Arc<Mutex<Vec<MessageContext>>>,
    mut msg_queue: mpsc::UnboundedReceiver<Message>,
) -> Result<(), StreamSerializerError> {
    loop {
        match msg_queue.recv().await {
            Some(message) => {
                message.send(&mut stream).await?;
                info!("Message sended via tcp!");

                match message {
                    Message::User(message) => {
                        msgs.lock().unwrap().push(MessageContext {
                            was_received: false,
                            message,
                        });
                    },
                    _ => {}
                }

            }
            None => {
                break Ok(());
            }
        }
    }
}

impl PeerState<'_> {
    pub fn render(&mut self, rect: &mut Rect, buf: &mut Buffer, is_active: bool) {
        // Devide conversation to include editor box.
        let [mut conv_block, mut edit_block] =
            Layout::vertical([Constraint::Min(3), Constraint::Length(3)])
                .areas(*rect);

        self.render_conv(&mut conv_block, buf, is_active && self.messages.is_selected());
        self.render_edit(&mut edit_block, buf, is_active && !self.messages.is_selected());
    }

    // Render conversation.
    fn render_conv(&mut self, rect: &mut Rect, buf: &mut Buffer, is_active: bool) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(if is_active {
                Style::default().fg(Color::Green)
            } else {
                Style::default()
            })
            .title(format!("Conversation with {}:", &self.name));

        let conv_rect = block.inner(*rect);

        Widget::render(block, *rect, buf);

        self.messages.render(conv_rect, buf);
    }

    // Render text input box.
    fn render_edit(&mut self, block: &mut Rect, buf: &mut Buffer, is_active: bool) {
        self.editor.set_block(
            Block::default()
                .title(match self.editor_mode {
                    EditorMode::Text => "Enter msg:",
                    EditorMode::File => "Enter file path:",
                })
                .borders(Borders::ALL)
                .border_style(if is_active {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default()
                })
        );

        Widget::render(&self.editor, *block, buf);
    }

    // Render if no peer was choosen.
    pub fn render_empty(block: &mut Rect, buf: &mut Buffer) {
        let block2 = Block::default()
            .title("No conversation picked!")
            .borders(ratatui::widgets::Borders::ALL);
        Widget::render(block2, *block, buf);
    }
}


impl<'a> ListItem<'a> for PeerState<'a> {
    fn get_cache(&mut self) -> &mut Option<ListCache<'a>> {
        &mut self.render_cache
    }

    fn prerender(&mut self, window_max_width: u16, selected: bool) {
        let bottom_address_length = UnicodeWidthStr::width(self.addr.to_string().as_str()).min(window_max_width as usize - 2);
        let middle_name_length    = UnicodeWidthStr::width(self.name.as_str()).min(window_max_width as usize - 2);
        let bottom_address: String = format!("{:─<width$}", &self.addr.to_string()[..bottom_address_length], width = window_max_width as usize - 2);
        let middle_name:    String = format!("{: <width$}", &self.name[..middle_name_length], width = window_max_width as usize - 2);

        let style = if selected {
            Style::default().bg(Color::DarkGray) // Change background color to Yellow if selected
        } else {
            Style::default()
        };

        let top_bar    = "┌".to_string() + &"─".repeat(window_max_width as usize - 2) + "┐";
        let middle_bar = "│".to_string() + &middle_name                               + "│";
        let bottom_bar = "└".to_string() + &bottom_address                            + "┘";

        let top_bar = Span::styled(top_bar, style);
        let middle_bar = Span::styled(middle_bar, style);
        let bottom_bar = Span::styled(bottom_bar, style);

        let block_lines: Vec<Line> = vec![
            Line::from(vec![top_bar]),
            Line::from(vec![middle_bar]),
            Line::from(vec![bottom_bar]),
        ];

        self.render_cache = Some(ListCache::new(
            block_lines,
            window_max_width,
            3,
            selected,
        ));
    }
}