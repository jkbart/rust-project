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

    assert_eq!(original.port, deserialized.port);
    assert_eq!(original.user_id, deserialized.user_id);
}

#[tokio::test]
async fn serialization_message_async() {
    let original = Message {
        content: MessageContent::Text("Dzień dobry".to_string()),
        msg_id: 123,
    };

    let mut buf: Vec<u8> = Vec::new();
    let mut cursor = Cursor::new(&mut buf);

    original.send(&mut cursor).await.unwrap();

    cursor.set_position(0);

    let deserialized: Message = Message::read(&mut cursor).await.unwrap();

    if let MessageContent::Text(ref original_text) = original.content {
        if let MessageContent::Text(ref deserialized_text) = deserialized.content {
            assert_eq!(original_text, deserialized_text);
        } else {
            panic!("Deserialized content is not Text");
        }
    } else {
        panic!("Original content is not Text");
    }

    assert_eq!(original.msg_id, deserialized.msg_id);

    assert_eq!(cursor.position(), buf.len() as u64);
}
