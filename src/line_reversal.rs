#![allow(dead_code)]
use log::{error, info};
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::{
    net::UdpSocket,
    sync::mpsc::{channel, unbounded_channel, Sender},
};

use self::{
    lrcp::LrcpSession,
    message::{Message, Payload, SessionId},
};

mod lrcp;
mod message;

const BLOCK_SIZE: usize = 1024;
const CHANNEL_SIZE: usize = 100;

type Sessions = BTreeMap<SessionId, Session>;
pub struct Session {
    pub tx: Sender<Message>,
    pub address: SocketAddr,
}

pub async fn run(port: &str) -> anyhow::Result<()> {
    let addr = format!("0.0.0.0:{}", port);
    let socket = Arc::new(UdpSocket::bind(&addr).await?);
    info!("Running Line Reversal server on {}...", &addr);

    let (_tx, mut rx) = unbounded_channel::<Message>();

    let mut sessions: BTreeMap<SessionId, Session> = BTreeMap::new();

    loop {
        tokio::select! {
            (message, address) = read_message(&socket) => {
                info!("Received packet from address: {}", address);
                handle_client_message(message, address, socket.clone(), &mut sessions).await;
            },

            resp = rx.recv() => {
                info!("Received packet from main channel: {:?}", resp);
                let message = resp.expect("Failed to receive packet from main channel");
                handle_response(message, &socket, &mut sessions).await;
            }
        }
    }
}

async fn read_message(socket: &UdpSocket) -> (Message, SocketAddr) {
    loop {
        let mut buf = [0u8; 1024];
        let (num_bytes, src) = socket
            .recv_from(&mut buf)
            .await
            .expect("Failed to receive packet");

        match Message::parse(&buf[..num_bytes]) {
            Ok(packet) => return (packet, src),
            Err(e) => {
                error!("Failed to parse packet: {}", e);
            }
        }
    }
}

async fn handle_client_message(
    message: Message,
    addr: SocketAddr,
    socket: Arc<UdpSocket>,
    sessions: &mut Sessions,
) {
    info!("Handing client message: {:?}", &message);
    match message.payload {
        Payload::Connect => {
            // If the session exists, ignore the message
            if let Some(_session) = sessions.get(&message.session) {
                return;
            }

            // Create a new session
            info!("Creating a new session for {:?}", message.session);
            let (packet_tx, packet_rx) = channel::<Message>(CHANNEL_SIZE);
            let session = Session {
                tx: packet_tx,
                address: addr,
            };
            sessions.insert(message.session.clone(), session);

            // Spawn a new task to handle the session
            tokio::spawn(async move {
                let mut client = LrcpSession::new(message.session, socket.clone(), addr, packet_rx);
                client.run().await;
            });
        }

        _ => {
            // If the session doesn't exist, ignore the message
            if let Some(session) = sessions.get(&message.session) {
                if let Err(e) = session.tx.send(message).await {
                    error!("Failed to send packet to session: {}", e);
                }
            } else {
                error!("Session doesn't exist: {:?}", message.session);
            }
        }
    }
}

pub async fn handle_response(message: Message, socket: &UdpSocket, sessions: &mut Sessions) {
    match message.payload {
        Payload::Close => {
            // If the session doesn't exist, ignore the message
            if let Some(session) = sessions.remove(&message.session) {
                respond(socket, message, session.address).await;
            } else {
                error!("Session doesn't exist: {:?}", message.session);
            }
        }

        _ => {
            // If the session doesn't exist, ignore the message
            if let Some(session) = sessions.get(&message.session) {
                respond(socket, message, session.address).await;
            } else {
                error!("Session doesn't exist: {:?}", message.session);
            }
        }
    }
}

pub async fn respond(socket: &UdpSocket, message: Message, addr: SocketAddr) {
    let bytes = message.to_packet();
    match socket.send_to(&bytes, addr).await {
        Ok(_num_bytes) => {
            info!("Sent packet to {}", addr);
        }
        Err(e) => {
            error!("Failed to send packet: {}", e);
        }
    }
}
