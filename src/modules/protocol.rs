use async_trait::async_trait;
use bincode::{deserialize, serialize};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::error::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::config::UNIQUE_BYTES;

/// Struct with content of message.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum MessageContent {
    Text(String),
    // TODO: add file exchange fields.
}

/// Message wrapper for meta data.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Message {
    // pub sender: String,      // No need to include it in already established tcp connection.
    // pub receiver: String,    // No need to include it in already established tcp connection.
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
    // TODO: own serializer to avoid possiblity of error when serializing.
    fn to_packet(&self) -> Result<Vec<u8>, Box<dyn Error>> {
        let msg_data = serialize(self)?;
        let msg_len = (msg_data.len() as u64).to_be_bytes();

        let mut packet = Vec::new();

        packet.extend_from_slice(UNIQUE_BYTES);
        packet.extend_from_slice(&msg_len);
        packet.extend(msg_data);

        Ok(packet)
    }

    pub fn from_packet(packet: Vec<u8>) -> Result<Self, Box<dyn Error>> {
        if packet.len() < UNIQUE_BYTES.len() + 8 {
            return Err("Discovery packet too short!".into());
        }

        let mut buff_idx = UNIQUE_BYTES.len();

        // Compare the UNIQUE_BYTES.
        if &packet[..buff_idx] != UNIQUE_BYTES {
            return Err("UNIQUE_BYTES don't match!".into());
        }

        // Read length of UserDiscovery struct.
        let msg_len = u64::from_be_bytes(packet[buff_idx..buff_idx + 8].try_into()?) as usize;

        buff_idx += 8;

        if packet.len() - buff_idx != msg_len {
            return Err("Discovery packet too short!".into());
        }

        // Read UserDiscovery struct.
        let data: UserDiscovery = bincode::deserialize(&packet[buff_idx..(buff_idx + msg_len)])?;

        Ok(data)
    }
}

// Auto implement stream serialization for all possible structs.
impl<T> StreamSerialization for T where T: Serialize + DeserializeOwned {}

#[async_trait]
pub trait StreamSerialization: Serialize + DeserializeOwned {
    async fn send<S: AsyncWriteExt + Unpin + Send>(
        &self,
        stream: &mut S,
    ) -> Result<(), Box<dyn Error>> {
        let msg_data = serialize(self)?;
        let msg_len = (msg_data.len() as u64).to_be_bytes();

        stream.write_all(&msg_len).await?;
        stream.write_all(&msg_data).await?;

        Ok(())
    }

    async fn read<S: AsyncReadExt + Unpin + Send>(stream: &mut S) -> Result<Self, Box<dyn Error>> {
        let mut msg_len_buff: [u8; 8] = [0; 8];
        stream.read_exact(&mut msg_len_buff).await?;
        let msg_len = u64::from_be_bytes(msg_len_buff);

        let mut msg_data_buff: Vec<u8> = vec![0; msg_len as usize];
        stream.read_exact(&mut msg_data_buff).await?;
        let msg = deserialize::<Self>(&msg_data_buff)?;

        Ok(msg)
    }
}
