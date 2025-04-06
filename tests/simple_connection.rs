use rust_project::config::*;
use rust_project::modules::{networking::*, peer_state::PeerState, protocol::*};
use std::net::SocketAddr;
use tokio::io::AsyncWriteExt;

use std::time::Duration;

use tokio::fs;
use tokio::net::*;

use ntest::timeout;
use tempfile::tempdir;
// User detection is not possible to be easily tested, since connections from user with same id are ignored.

async fn connect_to_port(addr: SocketAddr) -> TcpStream {
    TcpStream::connect(addr).await.unwrap()
}

async fn get_2_connections() -> (ConnectionData, ConnectionData) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let handle = tokio::task::spawn(connect_to_port(addr.clone()));

    let (stream, peer_address) = listener.accept().await.unwrap();
    drop(listener);

    let cd1 = ConnectionData {
        stream,
        peer_address,
        peer_name: "USER_A".to_string(),
    };

    let cd2 = ConnectionData {
        stream: handle.await.unwrap(),
        peer_address: addr,
        peer_name: "USER_B".to_string(),
    };

    (cd1, cd2)
}

async fn get_2_peers() -> (PeerState<'static>, PeerState<'static>) {
    let (cd1, cd2) = get_2_connections().await;

    (PeerState::from(cd1), PeerState::from(cd2))
}

#[tokio::test]
#[timeout(500)]
async fn exchange_text() {
    let (peer1, mut peer2) = get_2_peers().await;

    let example_user_msg = UserMessage::Text("IQVIBOABCHO".to_string());

    peer1.send(Message::User(example_user_msg.clone()));

    tokio::time::sleep(Duration::from_millis(100)).await;

    peer2.update();

    match &peer2.messages.list[..] {
        [msg] => {
            assert_eq!(msg.message, example_user_msg);
        }
        _ => panic!("Unexpected msg list length!"),
    }
}

#[tokio::test]
#[timeout(500)]
async fn exchange_text_both_ways() {
    let (mut peer1, mut peer2) = get_2_peers().await;

    let example_user_msg1 = UserMessage::Text("IQVIBOABCHO".to_string());
    let example_user_msg2 = UserMessage::Text("!@#$%^&&*()".to_string());

    peer1.send(Message::User(example_user_msg1.clone()));

    tokio::time::sleep(Duration::from_millis(100)).await;

    peer2.update();

    match &peer2.messages.list[..] {
        [msg] => {
            assert_eq!(msg.message, example_user_msg1);
        }
        list => panic!("Unexpected msg list length! {:#?}", list),
    }

    peer2.send(Message::User(example_user_msg2.clone()));

    tokio::time::sleep(Duration::from_millis(100)).await;

    peer1.update();

    match &peer1.messages.list[..] {
        [msg_self, msg_other] => {
            assert_eq!(msg_self.message, example_user_msg1);
            assert_eq!(msg_other.message, example_user_msg2);
        }
        list => panic!("Unexpected msg list length! {:#?}", list),
    }
}

#[tokio::test]
async fn file_transfer_successful() {
    let (mut peer1, mut peer2) = get_2_peers().await;

    let random_file_name = "rust-project-test-file-j87Rv8Mb19XNlX".to_string();

    let mut download_path = DOWNLOAD_PATH.clone();
    download_path.push(&random_file_name);

    let tmp_dir = tempdir().unwrap();

    let mut file_path = tmp_dir.path().to_path_buf();
    file_path.push(&random_file_name);

    // Make space for downloaded file.
    if std::fs::metadata(&download_path)
        .map(|metadata| metadata.is_file())
        .unwrap_or(false)
    {
        fs::remove_file(&download_path).await.unwrap();
    }

    let mut file = fs::OpenOptions::new()
        .create_new(true) // Ensures the file doesn't already exist
        .write(true)
        .open(&file_path)
        .await
        .unwrap();

    let file_content = "THIS IS TEST FILE!!".as_bytes();

    file.write_all(file_content).await.unwrap();
    drop(file);

    peer1.upload_file(file_path);

    tokio::time::sleep(Duration::from_millis(100)).await;

    peer2.update();

    peer2.messages.select(0);
    peer2.handle_action_on_msg();

    tokio::time::sleep(Duration::from_millis(100)).await;

    let downloaded_content = fs::read(&download_path).await.unwrap();

    let result = downloaded_content == file_content;

    if std::fs::metadata(&download_path)
        .map(|metadata| metadata.is_file())
        .unwrap_or(false)
    {
        fs::remove_file(&download_path).await.unwrap();
    }

    assert!(result);
}
