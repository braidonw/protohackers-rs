use tokio::sync::mpsc::{Receiver, Sender, UnboundedSender};

use super::protocol::{Packet, SessionId};
use std::collections::VecDeque;

pub struct LrcpClient {
    // Identifies the session
    id: SessionId,
    // A queue for sending responses to the client
    response_queue: VecDeque<Packet>,

    packet_rx: Receiver<Packet>,
    main_tx: UnboundedSender<Packet>,

    // The highest incoming position
    in_position: usize,
    // The highest outgoing position
    out_position: usize,
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
            in_position: 0,
            out_position: 0,
        }
    }

    pub async fn run(&mut self) {
        loop {
            tokio::select! {
                Some(packet) = self.packet_rx.recv() => {
                todo!()
                }
            }
        }
    }
}
