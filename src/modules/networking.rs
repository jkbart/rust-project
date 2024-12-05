use log::*;
use std::net::{Ipv4Addr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::mpsc;
use tokio::time;
use tokio::time::Duration;

use socket2::{Domain, Protocol, Socket, Type};

use crate::config::{MULTICAST_IP, MULTICAST_PORT, USER_ID, USER_NAME};

use super::protocol::*;

pub struct ConnectionData {
    pub stream: TcpStream,
    pub peer_address: SocketAddr,
    // pub peer_id: u64,
    pub peer_name: String,
}

/// Converts unread TcpStream into ConnectionData.
async fn establish_connection(
    mut stream: TcpStream,
    addr: SocketAddr,
    conn_queue: mpsc::UnboundedSender<ConnectionData>,
) {
    match time::timeout(
        Duration::from_secs(2),
        (ConnectionInfo {
            user_name: (*USER_NAME).clone(),
        })
        .send(&mut stream),
    )
    .await
    {
        Ok(Err(e)) => {
            info!("Couldn't establish connection: {:?}", e); // Ensure e is boxed with Send if necessary
            return;
        }
        Err(e) => {
            error!("Couldn't establish connection: {:?}", e); // Ensure e is boxed with Send if necessary
            return;
        }
        _ => (),
    }

    info!("Sent connection Info");

    match time::timeout(Duration::from_secs(2), ConnectionInfo::read(&mut stream)).await {
        Ok(Ok(info)) => {
            let _ = conn_queue.send(ConnectionData {
                stream,
                peer_address: addr,
                peer_name: info.user_name,
            });
        }
        Ok(Err(e)) => {
            error!("Couldn't establish connection: {:?}", e); // Ensure e is boxed with Send if necessary
        }
        Err(e) => {
            error!("Timed out during connection establishment: {:?}", e); // Ensure e is boxed with Send if necessary
        }
    }
}

pub async fn search_for_users(
    connection_queue: mpsc::UnboundedSender<ConnectionData>,
) -> Result<(), StreamSerializerError> {
    trace!("Binding multicast socket");
    let socket = Arc::new(get_multicast_socket(&MULTICAST_IP, MULTICAST_PORT).await?);

    trace!("Binding tcplistener socket");
    let listener = TcpListener::bind("0.0.0.0:0").await?; // Port to listen for
    let used_port = listener.local_addr()?.port();

    trace!("Accepting tcp connections on  port {}", used_port);

    let invitation_packet = UserDiscovery {
        user_id: *USER_ID,
        port: used_port,
    }
    .to_packet()?;

    trace!("Sending invite on MULTICAST for port {}!", used_port);

    socket.0.send_to(&invitation_packet, socket.1).await?;

    // TODO: handle their JoinHandles.
    tokio::task::spawn(socket_listener(listener, connection_queue.clone()));
    tokio::task::spawn(detect_new_users(socket,  connection_queue.clone()));
    Ok(())
}


/// Detects new tcp connections on port indefinitly and annouces user presence on MULTICAST.
async fn socket_listener(
    listener: TcpListener,
    connection_queue: mpsc::UnboundedSender<ConnectionData>,
) -> Result<(), StreamSerializerError> {
    loop {
        match listener.accept().await {
            Ok((socket, addr)) => {
                info!("Accepted new tcp connection from {}", addr);
                tokio::task::spawn(establish_connection(socket, addr, connection_queue.clone()));
            }
            Err(e) => {
                return Err(StreamSerializerError::Io(e));
            }
        }
    }
}

/// Detects new users on MULTICAST.
async fn detect_new_users(
    socket: Arc<(UdpSocket, SocketAddr)>,
    connection_queue: mpsc::UnboundedSender<ConnectionData>,
) -> Result<(), StreamSerializerError> {
    loop {
        let mut buf = vec![0; 4096];
        let (len, mut addr) = socket.0.recv_from(&mut buf).await?;
        info!("Received some bytes on MULTICAST!");

        match UserDiscovery::from_packet(buf[0..len].to_vec()) {
            Ok(disc) => {
                if disc.user_id == *USER_ID {
                    continue;
                }

                addr.set_port(disc.port); // update addr to point to tcp socket.
                info!("Multicast Userdiscovery packet received from: {:?}", addr);

                match TcpStream::connect(addr).await {
                    Ok(stream) => {
                        info!("connected ot tcp {}", addr);
                        tokio::task::spawn(establish_connection(
                            stream,
                            addr,
                            connection_queue.clone(),
                        ));
                    }
                    Err(e) => {
                        error!("Couldnt connect to addr posted via MULTICAST {e}!");
                    }
                }
            }
            Err(e) => {
                error!("UserDiscovery parsing error: {:?}!", e);
            }
        }
    }
}

pub async fn get_multicast_socket(
    mc_ip: &str,
    mc_port: u16,
) -> Result<(UdpSocket, SocketAddr), StreamSerializerError> {
    // Parse the multicast address
    let multicast_addr = SocketAddr::from_str(&format!("{mc_ip}:{mc_port}"))?;

    // Create a `socket2` socket
    let socket = match multicast_addr {
        SocketAddr::V4(_) => Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?,
        SocketAddr::V6(_) => Socket::new(Domain::IPV6, Type::DGRAM, Some(Protocol::UDP))?,
    };

    socket.set_reuse_address(true)?;
    socket.set_nonblocking(true)?;

    socket.bind(&format!("0.0.0.0:{mc_port}").parse::<SocketAddr>()?.into())?;

    // Convert `socket2::Socket` to `tokio::net::UdpSocket`
    let udp_socket = UdpSocket::from_std(socket.into())?;

    // Join multicast group.
    match multicast_addr {
        SocketAddr::V4(addr) => {
            udp_socket.join_multicast_v4(*addr.ip(), Ipv4Addr::UNSPECIFIED)?;
        }
        SocketAddr::V6(addr) => {
            udp_socket.join_multicast_v6(addr.ip(), 0)?;
        }
    }

    Ok((udp_socket, multicast_addr))
}
