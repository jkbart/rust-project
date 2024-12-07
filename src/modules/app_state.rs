use super::{networking::*, protocol::*};
use rand::RngCore;
use ratatui::widgets::ListState;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::task::JoinHandle;
use cli_log::*;

use tokio::sync::mpsc;

pub struct MessageContext {
    pub was_received: bool, // Whether it was sent or received.
    pub message: Message,
}

pub struct PeerState {
    pub name: String,
    pub addr: SocketAddr,
    pub conversation: Vec<MessageContext>,
    pub next_message: Message,
    conversation_buffer: Arc<Mutex<Vec<MessageContext>>>,
    message_writer_queue: mpsc::UnboundedSender<Message>,
    message_writer_handle: JoinHandle<Result<(), StreamSerializerError>>,
    message_reader_handle: JoinHandle<Result<(), StreamSerializerError>>,
}

pub struct PeerList {
    pub peer_list: Vec<PeerState>,
    pub state: ListState,
    peer_buffer: Arc<Mutex<Vec<PeerState>>>,
    peer_updator: JoinHandle<Result<(), StreamSerializerError>>,
}

impl PeerState {
    pub fn is_active(&self) -> bool {
        !self.message_writer_handle.is_finished() && !self.message_reader_handle.is_finished()
    }

    pub fn update(&mut self) {
        let mut msg_buffer = self.conversation_buffer.lock().unwrap();
        self.conversation.append(&mut msg_buffer);
    }

    pub fn send(&mut self, msg: Message) {
        let _ = self.message_writer_queue.send(msg);
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
        msgs.lock()
            .map_err(|e| StreamSerializerError::StrError(format!("{:?}", e)))?
            .push(MessageContext {
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

impl PeerList {
    pub fn new() -> Self {
        let peer_buffer: Arc<Mutex<Vec<PeerState>>> = Arc::new(Mutex::new(Vec::new()));

        let (tx_peer_list, rx_peer_list) = mpsc::unbounded_channel::<ConnectionData>();
        tokio::task::spawn(search_for_users(tx_peer_list));

        let peer_updator = tokio::task::spawn(peer_list_updator(peer_buffer.clone(), rx_peer_list));

        PeerList {
            peer_list: Vec::new(),
            state: ListState::default(),
            peer_buffer,
            peer_updator,
        }
    }

    pub fn update(&mut self) {
        let mut peer_buffer = self.peer_buffer.lock().unwrap();
        self.peer_list.append(&mut peer_buffer);
        if self.state.selected().is_none() && self.peer_list.len() > 1 {
            self.state.select(Some(0));
        }
    }

    pub fn select_next(&mut self) {
        if let Some(idx) = self.state.selected() {
            self.state.select(Some((idx + 1) % self.peer_list.len()));
        }
    }

    pub fn select_previous(&mut self) {
        if let Some(idx) = self.state.selected() {
            self.state.select(Some(
                (idx - 1 + self.peer_list.len()) % self.peer_list.len(),
            ));
        }
    }

    pub fn get_selected(&mut self) -> Option<&mut PeerState> {
        self.state.selected().map(|idx| &mut self.peer_list[idx])
    }
}

impl<'a> IntoIterator for &'a PeerList {
    type Item = &'a PeerState; // Borrows the items
    type IntoIter = std::slice::Iter<'a, PeerState>;

    fn into_iter(self) -> Self::IntoIter {
        self.peer_list.iter()
    }
}

pub async fn peer_list_updator(
    peers: Arc<Mutex<Vec<PeerState>>>,
    mut peer_queue: mpsc::UnboundedReceiver<ConnectionData>,
) -> Result<(), StreamSerializerError> {
    loop {
        match peer_queue.recv().await {
            Some(connection_data) => {
                info!("New user detected!");
                peers.lock().unwrap().push(connection_data.into());
            }
            None => {
                break Ok(());
            }
        }
    }
}
