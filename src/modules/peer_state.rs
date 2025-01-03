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
use unicode_width::UnicodeWidthStr;

use super::{networking::*, protocol::*};
use cli_log::*;
use rand::RngCore;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::task::JoinHandle;

use crate::modules::widgets::message_bubble::*;
use crate::modules::widgets::list_component::*;
use crate::modules::tui::AppPosition;

use tokio::sync::mpsc;

use tui_textarea::TextArea;

pub struct MessageContext {
    pub was_received: bool, // Whether it was sent or received.
    pub message: Message,
}

pub struct PeerState<'a> {
    pub name: String,
    pub addr: SocketAddr,
    render_cache: Option<ListCache>,

    pub messages: ListComponent<MsgBubble>,
    pub editor: TextArea<'a>,
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
                    true => self.name.clone(),
                    false => "You".to_string(),
                },
                mc.message,
                match mc.was_received {
                    true => MsgBubbleAllignment::Left,
                    false => MsgBubbleAllignment::Right,
                }
            )
        }));
    }

    pub fn send(&mut self, msg: Message) {
        let _ = self.message_writer_queue.send(msg);
    }

    pub fn handle_event(&mut self, key: KeyEvent, _current_screen: &mut AppPosition) -> bool {
        match key {
            key if key.code == KeyCode::Esc => {
                if key.kind == crossterm::event::KeyEventKind::Press {
                    if self.messages.is_selected() {
                        self.messages.reset();
                    } else {
                        return true;
                    }
                }
            },
            key if key.code == KeyCode::Up => {
                if key.kind == crossterm::event::KeyEventKind::Press {
                    if !self.messages.is_selected() {
                        self.messages.go_down();    // First select is little diffrent.
                    } else {
                        self.messages.go_up();
                    }
                }
            },
            key if key.code == KeyCode::Down => {
                if key.kind == crossterm::event::KeyEventKind::Press && self.messages.is_selected() {
                    if !self.messages.go_down() {
                        self.messages.reset();
                    }
                }
            },
            key if key.code == KeyCode::Enter && key.kind == crossterm::event::KeyEventKind::Press => {
                let msg = Message {
                    content: MessageContent::Text(self.editor.lines().join("\n")),
                    msg_id: rand::thread_rng().next_u64(),
                };

                self.editor = TextArea::default();

                self.send(msg);
            }
            editor_input if !self.messages.is_selected() =>  {
                self.editor.input(editor_input);
            },
            _ => {},
        }

        false
    }
}

impl From<ConnectionData> for PeerState<'_> {
    fn from(connection_data: ConnectionData) -> Self {
        let conversation_buffer: Arc<Mutex<Vec<MessageContext>>> = Arc::new(Vec::new().into());

        let (rx_stream, tx_stream) = connection_data.stream.into_split();
        let (tx_queue, rx_queue) = mpsc::unbounded_channel::<Message>();

        let message_reader_handle =
            tokio::task::spawn(message_reader(rx_stream, conversation_buffer.clone()));
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
            conversation_buffer,
            message_writer_queue: tx_queue,
            message_writer_handle,
            message_reader_handle,
        }
    }
}

async fn message_reader(
    mut stream: tokio::net::tcp::OwnedReadHalf,
    msgs: Arc<Mutex<Vec<MessageContext>>>,
) -> Result<(), StreamSerializerError> {
    loop {
        let message = Message::read(&mut stream).await?;
        info!("Message received via tcp!");
        msgs.lock().unwrap().push(MessageContext {
            was_received: true,
            message,
        });
    }
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
                msgs.lock().unwrap().push(MessageContext {
                    was_received: false,
                    message,
                });
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
                .title("Editor")
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


impl ListItem for PeerState<'_> {
    fn get_cache(&mut self) -> &mut Option<ListCache> {
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