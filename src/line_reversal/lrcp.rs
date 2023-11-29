use log::error;
use tokio::sync::mpsc::{Receiver, Sender, UnboundedSender};

use super::protocol::{Packet, Payload, SessionId};
use std::collections::VecDeque;

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
