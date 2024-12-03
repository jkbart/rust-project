use log::LevelFilter;

use env_logger;
use log::info;
use rust_project::config::USER_NAME;
use rust_project::modules::networking::*;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() {
    env_logger::Builder::new()
        .filter(None, LevelFilter::Trace) // Log all levels
        .target(env_logger::Target::Stderr) // Direct logs to stderr
        .init();

    info!("My name: {}", *USER_NAME);

    let (rx, mut tx) = mpsc::channel::<ConnectionData>(100);

    let rx_clone = rx.clone();

    tokio::task::spawn(async move {
        match detect_new_users(rx_clone).await {
            Ok(()) => {}
            Err(e) => {
                info!("socket_listener ended with error {:?}!", e);
            }
        }
    });

    while let Some(conn) = tx.recv().await {
        println!("New conn: {}", conn.peer_name);
    }
}
