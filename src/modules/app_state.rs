use super::{networking::*, protocol::*};
use tokio::sync::mpsc::error::TryRecvError;
use std::net::SocketAddr;
use tokio::net::TcpStream;
use tokio::task::JoinHandle;

use tokio::sync::mpsc;

struct MessageContext {
    was_received: bool,
    message: Message,
}

struct PeerState {
    name: String,
    addr: SocketAddr,
    is_active: bool,
    conversation: Vec<MessageContext>,
    message_buffor: MessageContent,
    message_queue: mpsc::UnboundedReceiver<Message>,
    message_reader_handle: JoinHandle<Result<(), StreamSerializerError>>,
}

impl PeerState {
    fn update(&mut self) {
        match self.message_queue.try_recv() {
            Ok(message) => {
                self.conversation.push(MessageContext { was_received: true, message });
            }, 
            Err(TryRecvError::Empty) => {},
            Err(_) => {
                self.is_active = false;
            },
        }
    }
}

impl From<ConnectionData> for PeerState {
    fn from(connection_data: ConnectionData) -> Self {
        let (tx, rx) = mpsc::unbounded_channel::<Message>();

        let message_reader_handle = tokio::task::spawn(message_reader(connection_data.stream, tx));

        PeerState {
            name: connection_data.peer_name,
            addr: connection_data.peer_address,
            is_active: true,
            conversation: Vec::new(),
            message_buffor: MessageContent::Text(String::new()), // Will be default message type.
            message_queue: rx,
            message_reader_handle,
        }
    }
}

async fn message_reader(
    mut stream: TcpStream,
    tx: mpsc::UnboundedSender<Message>,
) -> Result<(), StreamSerializerError> {
    loop {
        let msg = Message::read(&mut stream).await?;
        if let Err(e) = tx.send(msg) {
            return Err(StreamSerializerError::StrError(format!("{:?}", e)));
        }
    }
}

enum AppPosition {
    WaitingView,
    PeerList,
    TextEdit,
}

struct App {
    peers: Vec<PeerState>, // TODO: Hashmap by id.
    current_peer: Option<usize>, // If it is unset, then there are no users.
    current_position: AppPosition,
    peer_queue: mpsc::UnboundedReceiver<ConnectionData>,
    broadcast_handle: JoinHandle<Result<(), StreamSerializerError>>,
}

impl App {
    fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel::<ConnectionData>();
        App {
            peers: Vec::new(),
            current_peer: None,
            current_position: AppPosition::WaitingView,
            peer_queue: rx,
            broadcast_handle: tokio::task::spawn(detect_new_users(tx)),
        }
    }
}