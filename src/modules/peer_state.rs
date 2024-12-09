use crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::prelude::Stylize;
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Borders;
use ratatui::widgets::List;
use ratatui::widgets::ListItem;
use ratatui::widgets::Paragraph;
use ratatui::widgets::StatefulWidget;
use ratatui::widgets::Widget;
use ratatui::{
    buffer::Buffer,
    widgets::{Block, ListState},
};

use std::ops::Deref;

use super::{networking::*, protocol::*};
use cli_log::*;
use rand::RngCore;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::task::JoinHandle;

use tokio::sync::mpsc;

pub struct MessageContext {
    pub was_received: bool, // Whether it was sent or received.
    pub message: Message,
}

pub struct PeerState {
    pub name: String,
    pub addr: SocketAddr,
    pub conversation: Vec<MessageContext>,
    conversation_state: ListState,
    pub next_message: Message,
    conversation_buffer: Arc<Mutex<Vec<MessageContext>>>,
    message_writer_queue: mpsc::UnboundedSender<Message>,
    message_writer_handle: JoinHandle<Result<(), StreamSerializerError>>,
    message_reader_handle: JoinHandle<Result<(), StreamSerializerError>>,
}

impl PeerState {
    pub fn is_active(&self) -> bool {
        !self.message_writer_handle.is_finished() && !self.message_reader_handle.is_finished()
    }

    pub fn update(&mut self) {
        let mut msg_buffer = self.conversation_buffer.lock().unwrap();
        self.conversation.append(&mut msg_buffer);
        self.conversation_state
            .select(Some(self.conversation.len()));
        info!("{:?}", self.conversation_state);
    }

    pub fn send(&mut self, msg: Message) {
        let _ = self.message_writer_queue.send(msg);
    }

    pub fn handle_event(&mut self, keycode: &KeyCode) {
        match &keycode {
            KeyCode::Char(c) => match &mut self.next_message.content {
                MessageContent::Text(msg) => {
                    msg.push(*c);
                }
                _ => {}
            },
            KeyCode::Backspace => match &mut self.next_message.content {
                MessageContent::Text(msg) => {
                    msg.pop();
                }
                _ => {}
            },
            KeyCode::Enter => {
                let mut msg = Message {
                    content: MessageContent::Text(String::new()),
                    msg_id: rand::thread_rng().next_u64(),
                };
                std::mem::swap(&mut msg, &mut self.next_message);

                self.send(msg);
            }
            _ => {}
        }
    }
}

impl<'a> IntoIterator for &'a PeerState {
    type Item = &'a MessageContext; // Borrows the items
    type IntoIter = std::slice::Iter<'a, MessageContext>;

    fn into_iter(self) -> Self::IntoIter {
        self.conversation.iter()
    }
}

impl From<ConnectionData> for PeerState {
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
            conversation: Vec::new(),
            conversation_state: ListState::default(),
            next_message: Message {
                content: MessageContent::Text(String::new()),
                msg_id: rand::thread_rng().next_u64(),
            },
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

impl PeerState {
    // Render conversation.
    pub fn render_conv(&mut self, block: &mut Rect, buf: &mut Buffer) {
        let msg_items: Vec<ListItem> = self
            .conversation
            .iter()
            .map(|msg| { 
                let line = match &msg.message.content {
                    MessageContent::Text(txt) => {Line::from(vec![(txt.deref()).bold()])},
                    MessageContent::Empty() => Line::from(vec!["empty msg".bold()]),
                };
                if msg.was_received {
                    ListItem::from(line.left_aligned())
                } else {
                    ListItem::from(line.right_aligned())
                }
            })
            .collect();

        let msg_list = List::new(msg_items).block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("Conversation with {}:", &self.name)),
        );
        StatefulWidget::render(msg_list, *block, buf, &mut self.conversation_state);
    }

    // Render text input box.
    pub fn render_edit(&mut self, block: &mut Rect, buf: &mut Buffer, is_active: bool) {
        let text: &str = match &self.next_message.content {
            MessageContent::Text(txt) => txt,
            MessageContent::Empty() => "",
        };

        let paragraph = Paragraph::new(Span::raw(text)).block(
            Block::default()
                .title("Text Viewer")
                .borders(Borders::ALL)
                .border_style(if is_active {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default()
                }),
        );

        Widget::render(paragraph, *block, buf);
    }

    // Render if no peer was choosen.
    pub fn render_empty(block: &mut Rect, buf: &mut Buffer) {
        let block2 = Block::default()
            .title("No conversation picked!")
            .borders(ratatui::widgets::Borders::ALL);
        Widget::render(block2, *block, buf);
    }
}
