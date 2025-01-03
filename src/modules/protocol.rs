use async_trait::async_trait;
use bincode::{deserialize, serialize};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::net::AddrParseError;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::config::UNIQUE_BYTES;


/// Struct with content of user message.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum UserMessage {
    Text(String),
    FileHeader(String, u64, u64),      // Filename, filesize, file-id
}

/// Struct with content of internal message.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum InternalMessage {
    FileRequest(u64),                  // File-id
    FileContent(u64, u64, Vec<u8>),    // File-id, first byte idx, bytes
}

/// Main message structure.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Message {
    User(UserMessage),
    Internal(InternalMessage),
}


/// Struct that by being broadcasted annouces user presence.
/// Should not be sent as plain serialization in order to avoid misdetections.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UserDiscovery {
    pub port: u16,
    pub user_id: u64,
}

/// Struct that is being is send once at the begining of connection.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConnectionInfo {
    pub user_name: String,
    // Add id, currently under some conditions, user can have auto 2 conversations with same person.
}

impl UserDiscovery {
    // Packet format: UNIQUE_BYTES, 8 bytes of msg len, msg
    pub fn to_packet(&self) -> Result<Vec<u8>, StreamSerializerError> {
        let msg_data = serialize(self)?;
        let msg_len = (msg_data.len() as u64).to_be_bytes();

        let mut packet = Vec::new();

        packet.extend_from_slice(UNIQUE_BYTES);
        packet.extend_from_slice(&msg_len);
        packet.extend(msg_data);

        assert!(packet.len() < 4048); // Make sure it fits in one packet.
        Ok(packet)
    }

    pub fn from_packet(packet: Vec<u8>) -> Result<Self, StreamSerializerError> {
        if packet.len() < UNIQUE_BYTES.len() + 8 {
            return Err("Discovery packet too short!".into());
        }

        let mut buff_idx = UNIQUE_BYTES.len();

        // Compare the UNIQUE_BYTES.
        if &packet[..buff_idx] != UNIQUE_BYTES {
            return Err("UNIQUE_BYTES don't match!".into());
        }

        // Read length of UserDiscovery struct.
        let msg_len =
            u64::from_be_bytes(packet[buff_idx..buff_idx + 8].try_into().unwrap()) as usize; // This unwrap will never fail.

        buff_idx += 8;

        if packet.len() - buff_idx != msg_len {
            return Err("Discovery packet incorrect length!".into());
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
    ) -> Result<(), StreamSerializerError> {
        let msg_data = serialize(self)?;
        let msg_len = (msg_data.len() as u64).to_be_bytes();

        stream.write_all(&msg_len).await?;
        stream.write_all(&msg_data).await?;

        Ok(())
    }

    async fn read<S: AsyncReadExt + Unpin + Send>(
        stream: &mut S,
    ) -> Result<Self, StreamSerializerError> {
        let mut msg_len_buff: [u8; 8] = [0; 8];
        stream.read_exact(&mut msg_len_buff).await?;
        let msg_len = u64::from_be_bytes(msg_len_buff);

        let mut msg_data_buff: Vec<u8> = vec![0; msg_len as usize];
        stream.read_exact(&mut msg_data_buff).await?;
        let msg = deserialize::<Self>(&msg_data_buff)?;

        Ok(msg)
    }
}

#[derive(Debug)]
pub enum StreamSerializerError {
    Io(std::io::Error),
    Bincode(bincode::Error),
    StrError(String),
    AddrParse(AddrParseError), // Possible only when parsing multicast addr. Left here for convinience.
}

// Implement `From` trait to automatically convert `std::io::Error` to `StreamSerializerError`
impl From<std::io::Error> for StreamSerializerError {
    fn from(err: std::io::Error) -> Self {
        StreamSerializerError::Io(err)
    }
}

// Implement `From` trait to automatically convert `bincode::Error` to `StreamSerializerError`
impl From<bincode::Error> for StreamSerializerError {
    fn from(err: bincode::Error) -> Self {
        StreamSerializerError::Bincode(err)
    }
}

// Implementing `From<&str>` for automatic conversion
impl From<&str> for StreamSerializerError {
    fn from(err: &str) -> Self {
        StreamSerializerError::StrError(err.to_string())
    }
}

// Implementing `From<AddrParseError>` for automatic conversion
impl From<AddrParseError> for StreamSerializerError {
    fn from(err: AddrParseError) -> Self {
        StreamSerializerError::AddrParse(err)
    }
}
