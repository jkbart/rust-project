use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use rand::Rng;
use ratatui::prelude::Buffer;
use ratatui::prelude::Constraint;
use ratatui::prelude::Layout;
use ratatui::prelude::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Widget;
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::io::AsyncReadExt;

use ratatui::text::Line;
use tokio::io::AsyncWriteExt;
use unicode_width::UnicodeWidthStr;

use crate::config::*;
use crate::modules::{networking::*, protocol::*};

use cli_log::*;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::task::JoinHandle;

use crate::modules::message_bubble::*;
use crate::modules::tui::AppPosition;
use crate::modules::widgets::list_component::*;

use tokio::sync::mpsc;

use tui_textarea::TextArea;

use copypasta::{ClipboardContext, ClipboardProvider};

pub enum EditorMode {
    Text,
    File,
}

type DownloadedFilesMap = Arc<Mutex<HashMap<FileID, mpsc::UnboundedSender<InternalMessage>>>>;
type OwnedFilesMap = Arc<Mutex<HashMap<FileID, PathBuf>>>;

/// Struct for messages to be displayed with context.
pub struct MessageContext {
    pub was_received: bool, // Whether it was sent or received.
    pub message: UserMessage,
}

/// Main struct holding all information about connected peer.
pub struct PeerState<'a> {
    pub name: String,                               // Name of connected peer
    pub addr: SocketAddr,                           // Addres of connected peer
    render_cache: Option<ListCache<'a>>,            // Cache for UI rendering
    is_connected: bool,                             // If peer is connected.
    pub messages: ListComponent<'a, MsgBubble<'a>>, // List of user messages exchanged
    pub editor: TextArea<'a>,                       // Editor element
    pub editor_mode: EditorMode,                    // If entering file or text
    downloaded_files: DownloadedFilesMap,           // Files currently being downloaded
    owned_files: OwnedFilesMap,                     // Files shared with user.
    conversation_buffer: Arc<Mutex<Vec<MessageContext>>>,
    message_writer_queue: mpsc::UnboundedSender<Message>,
    message_writer_handle: JoinHandle<Result<(), StreamSerializerError>>,
    message_reader_handle: JoinHandle<Result<(), StreamSerializerError>>,
}

impl PeerState<'_> {
    pub fn is_active(&self) -> bool {
        !self.message_writer_handle.is_finished() && !self.message_reader_handle.is_finished()
    }

    // Merge buffored msgs for rendering.
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
                },
            )
        }));
    }

    pub fn send(&self, msg: Message) {
        let _ = self.message_writer_queue.send(msg);
    }

    pub fn download_file(
        &self,
        file_id: FileID,
        file_name: String,
        file_size: FileSize,
        loading_bar: Arc<Mutex<LoadingBarWrap>>,
        downloaded_msgs: DownloadedFilesMap,
    ) {
        let (tx, rx) = mpsc::unbounded_channel::<InternalMessage>();

        self.downloaded_files.lock().unwrap().insert(file_id, tx);

        tokio::task::spawn(file_downloader(
            rx,
            file_id,
            file_name,
            file_size,
            loading_bar,
            downloaded_msgs,
        ));

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
                }
                key if key.code == KeyCode::Up => {
                    if key.kind == crossterm::event::KeyEventKind::Press {
                        self.messages.go_up();
                    }
                }
                key if key.code == KeyCode::Down => {
                    if key.kind == crossterm::event::KeyEventKind::Press && !self.messages.go_down()
                    {
                        self.messages.reset();
                    }
                }
                key if key.code == KeyCode::Enter => {
                    if key.kind == crossterm::event::KeyEventKind::Press {
                        let message_bubble = self.messages.get_selected().unwrap();

                        match &message_bubble.message {
                            UserMessage::Text(text) => {
                                let _ = CLIPBOARD.lock().unwrap().set_contents(text.clone());
                            }
                            UserMessage::FileHeader(file_name, file_size, file_id) => {
                                if message_bubble.received_from.is_some()
                                    && is_loading_bar_free(&message_bubble.loading_bar)
                                {
                                    // Loading bar will be loaded later, those are placeholder values.
                                    let loading_bar = Arc::new(Mutex::new(LoadingBarWrap {
                                        loadingbar: LoadingBar::Status(LoadingBarStatus {
                                            position: 0,
                                            end: 1,
                                        }),
                                        changed: true,
                                    }));
                                    message_bubble.loading_bar = Some(loading_bar.clone());

                                    // Copy fist to allow for mut borrow of self in self.download_file call - droping message_bubble refs.
                                    let file_name = file_name.clone();
                                    let file_size = *file_size;
                                    let file_id = *file_id;

                                    self.download_file(
                                        file_id,
                                        file_name,
                                        file_size,
                                        loading_bar,
                                        self.downloaded_files.clone(),
                                    );
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        } else {
            // Currently editing next msg.
            match key {
                key if key.code == KeyCode::Esc => {
                    if key.kind == crossterm::event::KeyEventKind::Press {
                        return true;
                    }
                }
                key if key.code == KeyCode::Up => {
                    if key.kind == crossterm::event::KeyEventKind::Press {
                        self.messages.go_down(); // First entry to conversation is little diffrent - go_down on up key.
                    }
                }
                key if key.code == KeyCode::Tab => {
                    if key.kind == crossterm::event::KeyEventKind::Press {
                        self.editor_mode = match self.editor_mode {
                            EditorMode::Text => EditorMode::File,
                            EditorMode::File => EditorMode::Text,
                        }
                    }
                }
                key if key.code == KeyCode::Enter && key.modifiers == KeyModifiers::NONE => {
                    if key.kind == crossterm::event::KeyEventKind::Press {
                        match self.editor_mode {
                            EditorMode::Text => {
                                let msg = Message::User(UserMessage::Text(
                                    self.editor.lines().join("\n"),
                                ));

                                self.editor = TextArea::default();

                                self.send(msg);
                            }
                            EditorMode::File => {
                                let file_path = PathBuf::from(&self.editor.lines()[0]);
                                let file_id: FileID = rand::thread_rng().gen();

                                if std::fs::metadata(&file_path)
                                    .map(|metadata| metadata.is_file())
                                    .unwrap_or(false)
                                {
                                    let file_name: String = file_path
                                        .file_name()
                                        .unwrap()
                                        .to_string_lossy() // Maybe this could be improved.
                                        .to_string();
                                    let file_size: FileSize = std::fs::metadata(&file_path)
                                        .map(|metadata| metadata.len())
                                        .unwrap_or(0);

                                    self.owned_files.lock().unwrap().insert(file_id, file_path);

                                    self.editor = TextArea::default();

                                    self.send(Message::User(UserMessage::FileHeader(
                                        file_name, file_size, file_id,
                                    )));
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
                }
                editor_input => {
                    self.editor.input(editor_input);
                }
            }
        }

        false
    }
}

// Create new peer state from incoming connection
impl From<ConnectionData> for PeerState<'_> {
    fn from(connection_data: ConnectionData) -> Self {
        let conversation_buffer: Arc<Mutex<Vec<MessageContext>>> = Arc::new(Vec::new().into());

        let (rx_stream, tx_stream) = connection_data.stream.into_split();
        let (tx_queue, rx_queue) = mpsc::unbounded_channel::<Message>();

        let downloaded_files = Arc::new(Mutex::new(HashMap::new()));
        let owned_files = Arc::new(Mutex::new(HashMap::new()));

        let message_reader_handle = tokio::task::spawn(message_reader(
            rx_stream,
            tx_queue.clone(),
            conversation_buffer.clone(),
            downloaded_files.clone(),
            owned_files.clone(),
        ));

        let message_writer_handle = tokio::task::spawn(message_writer(
            tx_stream,
            conversation_buffer.clone(),
            rx_queue,
        ));

        PeerState {
            name: connection_data.peer_name,
            addr: connection_data.peer_address,
            render_cache: None,
            is_connected: true,
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

// Function responsible for reading incoming msgs in the background.
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
            }
            Message::Internal(internal_message) => match internal_message {
                InternalMessage::FileRequest(id) => {
                    if let Some(file_path) = owned_files.lock().unwrap().get(&id) {
                        tokio::task::spawn(file_uploader(
                            tx_message.clone(),
                            file_path.clone(),
                            id,
                        ));
                    }
                }
                InternalMessage::FileContent(id, _, _) => {
                    if let Some(tx) = downloaded_files.lock().unwrap().get(&id) {
                        let _ = tx.send(internal_message);
                    }
                }
                InternalMessage::FileContentError(id, _) => {
                    if let Some(tx) = downloaded_files.lock().unwrap().get(&id) {
                        let _ = tx.send(internal_message);
                    }
                }
            },
        }
    }
}

// Function responsible for sending msgs in the background.
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

                if let Message::User(message) = message {
                    msgs.lock().unwrap().push(MessageContext {
                        was_received: false,
                        message,
                    });
                }
            }
            None => {
                break Ok(());
            }
        }
    }
}

// Function responsible for downloading given file in the background.
async fn file_downloader(
    mut packets: mpsc::UnboundedReceiver<InternalMessage>,
    file_id: FileID,
    file_name: String,
    file_size: FileSize,
    loading_bar: Arc<Mutex<LoadingBarWrap>>,
    downloaded_files: DownloadedFilesMap,
) {
    let mut file_path = DOWNLOAD_PATH.clone();
    file_path.push(&file_name);

    *loading_bar.lock().unwrap() = LoadingBarWrap {
        loadingbar: LoadingBar::Status(LoadingBarStatus {
            position: 0,
            end: file_size,
        }),
        changed: true,
    };

    let mut byte_cnt = 0;

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

        while let Some(packet) = packets.recv().await {
            match packet {
                InternalMessage::FileContent(_, byte_idx, bytes) => {
                    if byte_idx != byte_cnt || byte_idx + bytes.len() as FileSize > file_size {
                        break 'main;
                    }

                    byte_cnt += bytes.len() as FileSize;

                    // For some reason writing drop explicitly doesnt work. Have to use {} instead.
                    {
                        let mut loading_bar_lock = loading_bar.lock().unwrap();

                        if let LoadingBar::Status(LoadingBarStatus { position, .. }) =
                            &mut loading_bar_lock.loadingbar
                        {
                            *position = byte_cnt;
                            loading_bar_lock.changed = true;
                        }
                    }

                    if let Err(e) = file.write_all(&bytes).await {
                        *loading_bar.lock().unwrap() = LoadingBarWrap {
                            loadingbar: LoadingBar::Error(e.to_string()),
                            changed: true,
                        };

                        break 'main;
                    }
                }
                InternalMessage::FileContentError(_, e) => {
                    *loading_bar.lock().unwrap() = LoadingBarWrap {
                        loadingbar: LoadingBar::Error(e),
                        changed: true,
                    };

                    break 'main;
                }
                _ => {}
            }

            if byte_cnt == file_size {
                break 'main;
            }
        }
    }

    if byte_cnt != file_size {
        *loading_bar.lock().unwrap() = LoadingBarWrap {
            loadingbar: LoadingBar::Error(format!(
                "Download error! Status: {}/{}",
                byte_cnt, file_size
            )),
            changed: true,
        };
    }

    // Clean map after yourself.
    let _ = downloaded_files.lock().unwrap().remove(&file_id);
}

// Function responsible for uploading given file in the background.
async fn file_uploader(
    packets: mpsc::UnboundedSender<Message>,
    file_name: PathBuf,
    file_id: FileID,
) {
    // Open the file in read-only mode
    let Ok(mut file) = tokio::fs::File::open(file_name).await else {
        let _ = packets.send(Message::Internal(InternalMessage::FileContentError(
            file_id,
            "File does not exsists anymore!".to_string(),
        )));
        return;
    };

    let mut buffer = vec![0; 4096]; // Buffer size 4096 bytes

    let mut byte_idx = 0;

    loop {
        let Ok(n) = file.read(&mut buffer).await else {
            let _ = packets.send(Message::Internal(InternalMessage::FileContentError(
                file_id,
                "Error reading file!".to_string(),
            )));
            return;
        };

        if n == 0 {
            break; // End of file
        }

        // Trim the buffer to the size of the data read
        let chunk = buffer[..n].to_vec();

        // Send the chunk through the sender
        let message = Message::Internal(InternalMessage::FileContent(file_id, byte_idx, chunk));
        byte_idx += n as FileSize;

        if packets.send(message).is_err() {
            break;
        }
    }
}

// Implentation of rendering functions.
impl PeerState<'_> {
    pub fn render(&mut self, rect: &mut Rect, buf: &mut Buffer, is_active: bool) {
        // Devide conversation to include editor box.
        let [mut conv_block, mut edit_block] =
            Layout::vertical([Constraint::Percentage(70), Constraint::Percentage(30)]).areas(*rect);

        self.render_conv(
            &mut conv_block,
            buf,
            is_active && self.messages.is_selected(),
        );
        self.render_edit(
            &mut edit_block,
            buf,
            is_active && !self.messages.is_selected(),
        );
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
                }),
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
        if self.is_connected != self.is_active() {
            self.is_connected = self.is_active();
            self.render_cache = None;
        }

        &mut self.render_cache
    }

    fn prerender(&mut self, window_max_width: u16, selected: bool) {
        let window_max_width = window_max_width.max(7); // On smaller windows this will cause to mess up visuals but will keep it from panicing.

        let bottom_address_length = UnicodeWidthStr::width(self.addr.to_string().as_str())
            .min(window_max_width as usize - 2);
        let middle_name_length =
            UnicodeWidthStr::width(self.name.as_str()).min(window_max_width as usize - 2);
        let bottom_address: String = format!(
            "{:─<width$}",
            &self.addr.to_string()[..bottom_address_length],
            width = window_max_width as usize - 2
        );
        let middle_name: String = format!(
            "{: <width$}",
            &self.name[..middle_name_length],
            width = window_max_width as usize - 2
        );

        let fg_color = if self.is_connected {
            Color::LightGreen
        } else {
            Color::LightRed
        };

        let style = if selected {
            Style::default().bg(Color::DarkGray)
        } else {
            Style::default()
        }
        .fg(fg_color);

        let top_bar = "┌".to_string() + &"─".repeat(window_max_width as usize - 2) + "┐";
        let middle_bar = "│".to_string() + &middle_name + "│";
        let bottom_bar = "└".to_string() + &bottom_address + "┘";

        let top_bar = Span::styled(top_bar, style);
        let middle_bar = Span::styled(middle_bar, style);
        let bottom_bar = Span::styled(bottom_bar, style);

        let block_lines: Vec<Line> = vec![
            Line::from(vec![top_bar]),
            Line::from(vec![middle_bar]),
            Line::from(vec![bottom_bar]),
        ];

        self.render_cache = Some(ListCache::new(block_lines, window_max_width, 3, selected));
    }
}
