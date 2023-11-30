use log::info;
use tokio::sync::mpsc::{Receiver, UnboundedSender};
use tokio::time::Sleep;

use super::protocol::{Packet, Payload, SessionId};
use std::collections::VecDeque;
use std::task::{Context, Poll};
use std::time::Duration;
use std::{future::Future, pin::Pin};

/// If we receive no data from a connection for this long, we will close it.
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(20);
/// If we don't receive an ack of a data packet after this amount of time,
/// we will send it again.
const RETRANSMISSION_TIMEOUT: Duration = Duration::from_secs(3);

pub struct LrcpClient {
    // Identifies the session
    id: SessionId,
    // A queue for sending responses to the client
    response_queue: VecDeque<Packet>,

    packet_rx: Receiver<Packet>,
    main_tx: UnboundedSender<Packet>,

    // The highest incoming position
    in_position: i32,
    // The highest outgoing position
    out_position: i32,
    total_data_length: i32,
}

impl LrcpClient {
    pub fn new(
        id: SessionId,
        packet_rx: Receiver<Packet>,
        main_tx: UnboundedSender<Packet>,
    ) -> Self {
        Self {
            id,
            response_queue: VecDeque::new(),
            packet_rx,
            main_tx,
            total_data_length: 0,
            in_position: 0,
            out_position: 0,
        }
    }

    fn update_in_position(&mut self, pos: i32) {
        if self.in_position < pos {
            self.in_position = pos;
        }
    }

    pub async fn run(&mut self) {
        loop {
            while let Some(packet) = self.response_queue.pop_front() {
                info!("Sending packet: {:?}", &packet);
                self.handle_respond(packet)
                    .await
                    .expect("Failed to send packet");
            }

            tokio::select! {
                Some(packet) = self.packet_rx.recv() => {
                    self.handle_packet(packet).await;
                },
            }
        }
    }

    async fn handle_respond(&self, packet: Packet) -> anyhow::Result<()> {
        self.main_tx
            .send(packet)
            .map_err(|_| anyhow::anyhow!("Failed to send packet"))
    }

    async fn handle_packet(&mut self, packet: Packet) {
        info!("Handing client packet: {:?}", &packet);
        match packet.payload {
            // If they connect, we ack
            Payload::Connect => {
                let response = Packet::new(
                    self.id.clone(),
                    Payload::Ack {
                        pos: self.in_position,
                    },
                );
                self.response_queue.push_back(response);
            }

            // If they send data, we reverse it and ack
            Payload::Data { data, pos } => {
                self.update_in_position(pos);
                self.total_data_length += data.as_ref().len() as i32;

                let response = Packet::new(
                    self.id.clone(),
                    Payload::Ack {
                        pos: self.in_position,
                    },
                );
                self.response_queue.push_back(response);
            }

            // If they ack, we update our out position
            Payload::Ack { pos } => {
                if self.out_position < pos {
                    self.out_position = pos;
                }
            }

            // If they close, we close
            Payload::Close => {
                let response = Packet::new(self.id.clone(), Payload::Close);
                self.response_queue.push_back(response);
            }
        }
    }
}

/// A sleeping future that will return a specific number when it resolves.
///
/// These are used to track whether retransmission is necessary.
struct SleepTimer {
    sleeper: Pin<Box<Sleep>>,
    count: usize,
}

impl SleepTimer {
    fn new(count: usize) -> SleepTimer {
        SleepTimer {
            sleeper: Box::pin(tokio::time::sleep(RETRANSMISSION_TIMEOUT)),
            count,
        }
    }
}

impl Future for SleepTimer {
    type Output = usize;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<usize> {
        match (self.sleeper).as_mut().poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(()) => Poll::Ready(self.count),
        }
    }
}
