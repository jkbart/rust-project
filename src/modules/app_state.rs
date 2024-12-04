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
    pub conversation: Arc<Mutex<Vec<MessageContext>>>,
    pub message_writer_queue: mpsc::UnboundedSender<Message>,
    pub message_writer_handle: JoinHandle<Result<(), StreamSerializerError>>,
    pub message_reader_handle: JoinHandle<Result<(), StreamSerializerError>>,
}

impl PeerState {
    pub fn is_active(&self) -> bool {
        !self.message_writer_handle.is_finished() && !self.message_reader_handle.is_finished()
    }
}

impl From<ConnectionData> for PeerState {
    fn from(connection_data: ConnectionData) -> Self {
        let conversation: Arc<Mutex<Vec<MessageContext>>> = Arc::new(Vec::new().into());

        let (rx_stream, tx_stream) = connection_data.stream.into_split();
        let (tx_queue, rx_queue) = mpsc::unbounded_channel::<Message>();

        let message_reader_handle =
            tokio::task::spawn(message_reader(rx_stream, conversation.clone()));
        let message_writer_handle =
            tokio::task::spawn(message_writer(tx_stream, conversation.clone(), rx_queue));

        PeerState {
            name: connection_data.peer_name,
            addr: connection_data.peer_address,
            conversation,
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
                msgs.lock()
                    .map_err(|e| StreamSerializerError::StrError(format!("{:?}", e)))?
                    .push(MessageContext {
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

pub async fn peer_list_updator(
    peers: Arc<Mutex<Vec<PeerState>>>,
    mut peer_queue: mpsc::UnboundedReceiver<ConnectionData>,
) -> Result<(), StreamSerializerError> {
    loop {
        match peer_queue.recv().await {
            Some(connection_data) => {
                peers
                    .lock()
                    .map_err(|e| StreamSerializerError::StrError(format!("{:?}", e)))?
                    .push(connection_data.into());
            }
            None => {
                break Ok(());
            }
        }
    }
}
