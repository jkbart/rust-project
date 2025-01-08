use rust_project::modules::protocol::*;
use std::io::Cursor;

#[test]
fn serialization_user_discovery() {
    let original = UserDiscovery {
        port: 121,
        user_id: 98989,
    };

    let data = original.to_packet().unwrap();

    let deserialized = UserDiscovery::from_packet(data).unwrap();

    assert_eq!(original, deserialized);
}

#[tokio::test]
async fn serialization_message_async_1() {
    let original = Message::User(UserMessage::Text("Dzień dobry".to_string()));

    let mut buf: Vec<u8> = Vec::new();
    let mut cursor = Cursor::new(&mut buf);

    original.send(&mut cursor).await.unwrap();

    cursor.set_position(0);

    let deserialized: Message = Message::read(&mut cursor).await.unwrap();

    assert_eq!(original, deserialized);

    // Check if serialization-deserialization is identity
    assert_eq!(cursor.position(), buf.len() as u64);
}

#[tokio::test]
async fn serialization_message_async_2() {
    let original = Message::Internal(InternalMessage::FileContent(
        24857234,
        0b10101,
        vec![42; 0b10101],
    ));

    let mut buf: Vec<u8> = Vec::new();
    let mut cursor = Cursor::new(&mut buf);

    original.send(&mut cursor).await.unwrap();

    cursor.set_position(0);

    let deserialized: Message = Message::read(&mut cursor).await.unwrap();

    assert_eq!(original, deserialized);

    // Check if serialization-deserialization is identity
    assert_eq!(cursor.position(), buf.len() as u64);
}
