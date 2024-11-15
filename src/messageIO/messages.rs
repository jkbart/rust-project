use tokio::io::{AsyncReadExt, AsyncWriteExt};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use bincode::{serialize, deserialize};
use std::error::Error;

/// Struct with content of message.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum MessageContent {
    Text(String),
    // TODO: add file exchange fields.
}

/// Message wrapper for meta data.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Message {
    // pub sender: String,
    // pub receiver: String,
    pub content: MessageContent,
    pub msg_id: u64,
}

/// Struct that by being broadcasted annouces user presence.
/// Should not be sent as plain serialization in order to avoid misdetections.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UserDiscovery {
    pub name: String,
    pub port: u16,
    pub user_id: u64,
}

impl UserDiscovery {
    // String added to decrease possibility of accidental discovery.
    const UNIQUE_SUFFIX: &'static str = "CHATapp>4RxPOv@1Gy8SZ8syH7$MlVAA2>0y]D`%KTIN\"Y[Lk9Z}\"k{p)";

    // TODO: own serializer to avoid possiblity of error when serializing.
    fn to_packet(&self) -> Result<Vec<u8>, Box<dyn Error>> {
        let msg_data = serialize(self)?;
        let msg_len = (msg_data.len() as u64).to_be_bytes();

        let mut packet = Vec::new();

        packet.extend_from_slice(&msg_len);
        packet.extend(msg_data);
        packet.extend_from_slice(&UserDiscovery::UNIQUE_SUFFIX.as_bytes());

        Ok(packet)
    }


    pub fn from_packet(packet: Vec<u8>) -> Result<Self, Box<dyn Error>> {
        let msg_len = u64::from_be_bytes(packet[0..8].try_into()?) as usize;

        if packet.len() != 8 + msg_len + UserDiscovery::UNIQUE_SUFFIX.len() {
            return Err("Invalid packet length.".into());
        }

        let data: UserDiscovery = bincode::deserialize(&packet[8..(8 + msg_len)])?;

        if &packet[(8 + msg_len)..] != UserDiscovery::UNIQUE_SUFFIX.as_bytes() {
            return Err("Invalid suffix.".into());
        }

        // Return the deserialized struct
        Ok(data)
    }
}

// Auto implement stream serialization for all possible structs.
impl<T> StreamSerialization for T where T: Serialize + DeserializeOwned {}

pub trait StreamSerialization: Serialize + DeserializeOwned {
    async fn send<S: AsyncWriteExt + Unpin>(&self, stream: &mut S) -> Result<(), Box<dyn Error>> {
        let msg_data = serialize(self)?;
        let msg_len = (msg_data.len() as u64).to_be_bytes();

        stream.write_all(&msg_len).await?;
        stream.write_all(&msg_data).await?;

        Ok(())
    }

    async fn read<S: AsyncReadExt + Unpin>(stream: &mut S) -> Result<Self, Box<dyn Error>> {
        let mut msg_len_buff: [u8; 8] = [0; 8];
        stream.read_exact(&mut msg_len_buff).await?;
        let msg_len = u64::from_be_bytes(msg_len_buff);

        let mut msg_data_buff: Vec<u8> = vec![0; msg_len as usize];
        stream.read_exact(&mut msg_data_buff).await?;
        let msg = deserialize::<Self>(&msg_data_buff)?;

        Ok(msg)
    }

}