use tokio::io::{AsyncReadExt, AsyncWriteExt};
use serde::{Serialize, Deserialize};
use bincode::{serialize, deserialize};
use std::error::Error;


#[derive(Serialize, Deserialize, Debug)]
pub enum MessageContent {
    Text(String),
    // TODO: add file exchange fields.
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Message {
    // pub sender: String,
    // pub receiver: String,
    pub content: MessageContent,
    pub id: u64,
}

impl Message {
    async fn send<S: AsyncWriteExt + Unpin>(&self, stream: &mut S) -> Result<(), Box<dyn Error>> {
        let msg_data = serialize(self)?;
        let msg_len = (msg_data.len() as u64).to_be_bytes();

        stream.write_all(&msg_len).await?;
        stream.write_all(&msg_data).await?;

        Ok(())
    }

    async fn read<S: AsyncReadExt + Unpin>(stream: &mut S) -> Result<Message, Box<dyn Error>> {
        let mut msg_len_buff: [u8; 8] = [0; 8];
        stream.read_exact(&mut msg_len_buff).await?;
        let msg_len = u64::from_be_bytes(msg_len_buff);

        let mut msg_data_buff: Vec<u8> = vec![0; msg_len as usize];
        stream.read_exact(&mut msg_data_buff).await?;
        let message = deserialize::<Message>(&msg_data_buff)?;

        Ok(message)
    }
}