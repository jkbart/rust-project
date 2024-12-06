use super::{networking::*, protocol::*};
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
    conversation_buffer: Arc<Mutex<Vec<MessageContext>>>,
    message_writer_queue: mpsc::UnboundedSender<Message>,
    message_writer_handle: JoinHandle<Result<(), StreamSerializerError>>,
    message_reader_handle: JoinHandle<Result<(), StreamSerializerError>>,
}

pub struct PeerList {
    pub peer_list: Vec<PeerState>,
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

    pub async fn send(&mut self, msg: Message) {
        let _ = self.message_writer_queue.send(msg);
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
            peer_buffer,
            peer_updator,
        }
    }

    pub fn update(&mut self) {
        let mut peer_buffer = self.peer_buffer.lock().unwrap();
        self.peer_list.append(&mut peer_buffer);
    }
}

pub async fn peer_list_updator(
    peers: Arc<Mutex<Vec<PeerState>>>,
    mut peer_queue: mpsc::UnboundedReceiver<ConnectionData>,
) -> Result<(), StreamSerializerError> {
    loop {
        match peer_queue.recv().await {
            Some(connection_data) => {
                peers.lock().unwrap().push(connection_data.into());
            }
            None => {
                break Ok(());
            }
        }
    }
}
