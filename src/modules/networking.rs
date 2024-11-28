use std::net::Ipv4Addr;
use std::net::SocketAddrV4;
use tokio::net::UdpSocket;
use tokio::time;
use tokio::io::AsyncReadExt;
use tokio::time::Duration;
use std::io;
use tokio::net::TcpListener;
use std::net::SocketAddr;
use tokio::sync::mpsc;
use tokio::net::TcpStream;
use log::info;

use crate::config::{MULTICAST_IP, MULTICAST_PORT, UNIQUE_BYTES, USER_ID};

use super::protocol::UserDiscovery;

pub struct ConnectionData {
	pub tcp_stream: TcpStream,
	pub end_address: SocketAddr,
}

// Validates connection by comparing first n bytes to UNIQUE_BYTES, and crates new ConnectionData at the end of mpsc.
async fn validate_connection(mut socket: TcpStream, addr: SocketAddr, conn_queue: mpsc::Sender<ConnectionData>) {
	let mut buffor = vec![0; UNIQUE_BYTES.len()];
	match time::timeout(Duration::from_secs(2), socket.read_exact(&mut buffor)).await {
	    Ok(_) => {
	    	if buffor[..] != *UNIQUE_BYTES {
	    		info!("Attempting connection with wrong UNIQUE_BYTES!")
	    	} else {
	    		if let Err(e) = conn_queue.send(ConnectionData { tcp_stream: socket, end_address: addr }).await {
	    			info!("Failed to add connection to queue: {e}");
	    		}
	    	}
	    },
	    Err(e) => info!("Connection from {addr} timedout at start! ({e})"),
	}
}

// Detects new tcp connections on port indefinitly and annouces user presence on MULTICAST.
async fn socket_listener(connection_queue: mpsc::Sender<ConnectionData>) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let used_port = listener.local_addr()?.port();

    let multicast_addr = SocketAddr::new(MULTICAST_IP.parse()?, MULTICAST_PORT);
    let invitation_packet = UserDiscovery{user_id: USER_ID, port: used_port}.to_packet()?;

    // Invite other users to our port.
    UdpSocket::bind("0.0.0.0:0").await?.send_to(&invitation_packet, multicast_addr).await?;

    loop {
	    match listener.accept().await {
	        Ok((socket, addr)) => {
	        	let q_clone = connection_queue.clone();

	        	tokio::task::spawn(validate_connection(socket, addr, q_clone));
	        },
	        Err(e) => {
	        	return Err(Box::new(e));
	        }
	    }
    }
}

// Detects new users on MULTICAST indefinitly.
async fn detect_new_users(conn_queue: mpsc::Sender<ConnectionData>) -> Result<(), Box<dyn std::error::Error>> {
    let socket = UdpSocket::bind("127.0.0.1:0").await?;
    let used_port = socket.local_addr()?.port();

    let multicast_addr = SocketAddrV4::new(MULTICAST_IP.parse()?, MULTICAST_PORT);
      
    socket.join_multicast_v4(*multicast_addr.ip(), Ipv4Addr::UNSPECIFIED)?;

    loop {
    	let mut buf = vec![0; 4048];
        let (len, mut addr2) = socket.recv_from(&mut buf).await?;

        match UserDiscovery::from_packet(buf[0..len].to_vec()) {
        	Ok(disc) => {
        		let q_clone = conn_queue.clone();
        		addr2.set_port(disc.port); // update addr to point to tcp socket.

        		tokio::task::spawn(async move {
        			match TcpStream::connect(addr2).await {
        				Ok(stream) => {
							if let Err(e) = q_clone.send(ConnectionData { tcp_stream: stream, end_address: addr2 }).await {
	    						info!("Failed to add connection to queue: {e}");
	    					}
						},

						Err(e) => {
							info!("Couldnt connect to addr posted via MULTICAST!");
						}

        			}
        		});
        	},
        	Err(e) => {
        		info!("UserDiscovery parsing error: {e}!");
        	}
        }
    }
}