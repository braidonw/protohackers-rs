use log::info;
use std::net::SocketAddr;
use tokio::sync::mpsc::Receiver;

use super::message::{Message, Payload, SessionId};
use std::sync::Arc;
use std::sync::RwLock;
use tokio::net::UdpSocket;

use std::time::Duration;

/// If we receive no data from a connection for this long, we will close it.
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(20);
/// If we don't receive an ack of a data packet after this amount of time,
/// we will send it again.
const RETRANSMISSION_TIMEOUT: Duration = Duration::from_secs(3);

pub struct LrcpSession {
    // Identifies the session
    id: SessionId,

    // The address of the client
    address: SocketAddr,
    socket: Arc<UdpSocket>,

    message_rx: Receiver<Message>,

    connected: bool,
    data: String,

    bytes_received: u32,
    bytes_sent: u32,
    bytes_acked: Arc<RwLock<u32>>,
}

impl LrcpSession {
    pub fn new(
        id: SessionId,
        socket: Arc<UdpSocket>,
        address: SocketAddr,
        message_rx: Receiver<Message>,
    ) -> Self {
        Self {
            id,
            address,
            socket,
            message_rx,
            connected: false,
            data: String::new(),
            bytes_received: 0,
            bytes_sent: 0,
            bytes_acked: Arc::new(RwLock::new(0)),
        }
    }

    pub async fn run(&mut self) {
        info!(
            "New session connected. session_id={:?}, peer_address={}",
            self.id, self.address
        );

        loop {
            tokio::select! {
                Some(message) = self.message_rx.recv() => {
                    if self.handle_message(message).await.is_err() {
                        info!("Session closed. session={:?}, address={}", self.id, self.address);
                    };
                }
            }
        }
    }

    async fn ack(&self, position: u32) -> anyhow::Result<()> {
        let response = Message::new_ack(self.id.clone(), position);
        info!("Acking message: {:?}", &response);
        match self
            .socket
            .send_to(&response.to_packet(), &self.address)
            .await
        {
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow::anyhow!("Failed to send packet: {}", e)),
        }
    }

    async fn close(&mut self) -> anyhow::Result<()> {
        let response = Message::new_close(self.id.clone());
        info!("Closing session: {:?}", &response);
        self.socket
            .send_to(&response.to_packet(), &self.address)
            .await
            .expect("Failed to send packet");

        self.message_rx.close();

        Err(anyhow::anyhow!("Session closed"))
    }

    async fn handle_message(&mut self, msg: Message) -> anyhow::Result<()> {
        info!("Handling new message: {:?}", &msg);
        match msg.payload {
            Payload::Connect => {
                self.connected = true;
                self.ack(0).await
            }

            Payload::Close => self.close().await,

            Payload::Ack { position } => {
                if !self.connected {
                    self.close().await?;
                }

                if position > self.bytes_sent {
                    info!(
                        "Unexpected Ack: {:?}. Current Bytes Sent: {}",
                        &msg, self.bytes_sent
                    );
                    self.close().await?;
                }

                let mut acked_bytes = self.bytes_acked.write().unwrap();
                *acked_bytes = position;

                Ok(())
            }

            Payload::Data { data, position } => {
                if !self.connected {
                    self.close().await?;
                }

                if position > self.bytes_received {
                    self.ack(self.bytes_received).await?;
                    return Ok(());
                }

                let data_position = self.bytes_received - position;
                if data_position as usize > data.len() {
                    info!(
                        "Message already seen. Current Bytes Received: {}",
                        self.bytes_received
                    );
                    return Ok(());
                }

                let new_data = &data[data_position as usize..];
                self.data.push_str(&String::from_utf8_lossy(new_data));
                self.bytes_received += new_data.len() as u32;
                self.ack(self.bytes_received).await?;

                if new_data.contains(&b'\n') {
                    for line in self
                        .data
                        .split_inclusive('\n')
                        .filter(|line| line.ends_with('\n'))
                    {
                        let reversed_line = reverse_line(line);
                        self.send_line(reversed_line).await;
                    }
                    if let Some(last_str) = self.data.split_inclusive('\n').last() {
                        if last_str.ends_with('\n') {
                            info!("Clearing buffer data. session_id={:?}", self.id);
                            self.data.clear();
                        } else {
                            info!(
                                "Dropping already sent buffer data. session_id={:?}",
                                self.id
                            );
                            self.data = last_str.to_owned();
                        }
                    }
                }
                Ok(())
            }
        }
    }

    async fn send_line(&self, line: String) {
        let messages = chunk_lines(line)
            .iter()
            .map(|line| {
                let position = self.bytes_sent;
                let message =
                    Message::new_data(self.id.clone(), line.as_bytes().to_vec(), position);
                message
            })
            .collect();

        // Timeout
        //
        tokio::spawn(send_messages(
            self.socket.clone(),
            self.address,
            messages,
            self.bytes_acked.clone(),
        ));
    }
}

fn chunk_lines(line: String) -> Vec<String> {
    line.as_bytes()
        .chunks(900)
        .map(|chunk| String::from_utf8_lossy(chunk).to_string())
        .collect::<Vec<String>>()
}

async fn send_messages(
    socket: Arc<UdpSocket>,
    addr: SocketAddr,
    messages: Vec<Message>,
    bytes_acked: Arc<RwLock<u32>>,
) {
    let mut retransmission_timeout = tokio::time::interval(Duration::from_secs(3));

    loop {
        tokio::select! {
            biased;

            _ = retransmission_timeout.tick() => {
                let most_recent_ack = { *bytes_acked.read().unwrap() };
                let mut all_messages_acked = true;

                for message in &messages {
                    if let Payload::Data { position, ..} = message.payload {
                        if position > most_recent_ack {
                            all_messages_acked = false;
                            socket.send_to(&message.to_packet(), &addr).await.unwrap();
                        }
                    }
                }
                if all_messages_acked {
                    break;
                }
            }
        }
    }
}

fn reverse_line(line: &str) -> String {
    let mut reversed_line: String = line.trim_end().chars().rev().collect();
    reversed_line.push('\n');
    reversed_line
}
