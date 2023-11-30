#![allow(dead_code)]
use log::{error, info};
use protocol::Packet;
use std::collections::BTreeMap;
use std::net::SocketAddr;
use tokio::{
    net::UdpSocket,
    sync::mpsc::{channel, unbounded_channel, Sender, UnboundedSender},
};

use self::{
    lrcp::LrcpClient,
    protocol::{Payload, SessionId},
};

mod lrcp;
mod message;
mod protocol;

const BLOCK_SIZE: usize = 1024;
const CHANNEL_SIZE: usize = 100;

type Sessions = BTreeMap<SessionId, Session>;
pub struct Session {
    pub tx: Sender<Packet>,
    pub address: SocketAddr,
}

pub async fn run(port: &str) -> anyhow::Result<()> {
    let addr = format!("0.0.0.0:{}", port);
    let socket = UdpSocket::bind(&addr).await?;
    info!("Running Line Reversal server on {}...", &addr);

    let (tx, mut rx) = unbounded_channel::<Packet>();

    let mut sessions: BTreeMap<SessionId, Session> = BTreeMap::new();

    loop {
        tokio::select! {
            (packet, address) = read_packet(&socket) => {
                info!("Received packet from address: {}", address);
                handle_receive_client_packet(packet, address, &mut sessions, tx.clone()).await;
            },

            resp = rx.recv() => {
                info!("Received packet from main channel: {:?}", resp);
                let packet = resp.expect("Failed to receive packet from main channel");
                handle_receive_internal_packet(packet, &socket, &mut sessions).await;
            }
        }
    }
}

async fn read_packet(socket: &UdpSocket) -> (Packet, SocketAddr) {
    loop {
        let mut buf = [0u8; 1024];
        let (num_bytes, src) = socket
            .recv_from(&mut buf)
            .await
            .expect("Failed to receive packet");

        match Packet::try_from(&buf[..num_bytes]) {
            Ok(packet) => return (packet, src),
            Err(e) => {
                error!("Failed to parse packet: {}", e);
            }
        }
    }
}

async fn handle_receive_client_packet(
    packet: Packet,
    addr: SocketAddr,
    sessions: &mut Sessions,
    main_tx: UnboundedSender<Packet>,
) {
    info!("Handing client packet: {:?}", &packet);
    match packet.payload {
        Payload::Connect => {
            // If the session exists, ignore the message
            if let Some(_session) = sessions.get(&packet.session_id) {
                return;
            }

            // Create a new session
            info!("Creating a new session for {:?}", packet.session_id);
            let (packet_tx, packet_rx) = channel::<Packet>(CHANNEL_SIZE);
            let session = Session {
                tx: packet_tx,
                address: addr,
            };
            sessions.insert(packet.session_id.clone(), session);
            let main_tx = main_tx;

            // Spawn a new task to handle the session
            tokio::spawn(async move {
                let mut client = LrcpClient::new(packet.session_id, packet_rx, main_tx);
                client.run().await;
            });
        }

        Payload::Close => {
            // If the session doesn't exist, ignore the message
            if let Some(session) = sessions.get(&packet.session_id) {
                if let Err(e) = session.tx.send(packet.clone()).await {
                    error!("Failed to send packet to session: {}", e);
                }
                sessions.remove(&packet.session_id);
            } else {
                error!("Session doesn't exist: {:?}", packet.session_id);
            }
        }

        _ => {
            // If the session doesn't exist, ignore the message
            if let Some(session) = sessions.get(&packet.session_id) {
                if let Err(e) = session.tx.send(packet).await {
                    error!("Failed to send packet to session: {}", e);
                }
            } else {
                error!("Session doesn't exist: {:?}", packet.session_id);
            }
        }
    }
}

pub async fn handle_receive_internal_packet(
    packet: Packet,
    socket: &UdpSocket,
    sessions: &mut Sessions,
) {
    match packet.payload {
        Payload::Close => {
            // If the session doesn't exist, ignore the message
            if let Some(session) = sessions.remove(&packet.session_id) {
                respond(socket, packet, session.address).await;
            } else {
                error!("Session doesn't exist: {:?}", packet.session_id);
            }
        }

        _ => {
            // If the session doesn't exist, ignore the message
            if let Some(session) = sessions.get(&packet.session_id) {
                respond(socket, packet, session.address).await;
            } else {
                error!("Session doesn't exist: {:?}", packet.session_id);
            }
        }
    }
}

pub async fn respond(socket: &UdpSocket, packet: Packet, addr: SocketAddr) {
    let bytes = packet.to_bytes();
    match socket.send_to(&bytes, addr).await {
        Ok(_num_bytes) => {
            info!("Sent packet to {}", addr);
        }
        Err(e) => {
            error!("Failed to send packet: {}", e);
        }
    }
}
