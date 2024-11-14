use std::{
    collections::HashMap,
    net::ToSocketAddrs,
    time::{Duration, Instant},
};

use super::packet::{AckNumber, AckPacket};

pub(crate) const ACK_RESET_LIMIT: u32 = 128_000;

/// Handles packet acknowledgements
pub(crate) struct AcknowledgementManager {
    /// The last ack number that was sent out
    pub last_ack: AckNumber,
    /// Packets we are waiting on being acknowledged
    pub packets_waiting_on_ack: HashMap<AckNumber, AckPacket>,
    /// How long to retry sending a packet after not being acknowledged
    pub ack_retry_time: Duration,
}

impl AcknowledgementManager {
    /// Creates a new acknowledgement manager
    pub(crate) fn new() -> Self {
        Self {
            last_ack: AckNumber::new(0),
            packets_waiting_on_ack: HashMap::new(),
            ack_retry_time: Duration::from_secs(2),
        }
    }

    pub(crate) fn get_new_ack_num(&mut self) -> AckNumber {
        self.last_ack += AckNumber::new(1);

        if self.last_ack.raw() >= ACK_RESET_LIMIT {
            self.last_ack = AckNumber::new(0);
        }

        self.last_ack
    }

    pub(crate) fn insert_packet_into_ack_waiting_list<A>(
        &mut self,
        ack_num: AckNumber,
        buf: Vec<u8>,
        addr: A,
    ) where
        A: ToSocketAddrs + Into<String>,
    {
        self.packets_waiting_on_ack
            .insert(ack_num, AckPacket::new(buf, Instant::now(), addr));
    }
}

impl Default for AcknowledgementManager {
    fn default() -> Self {
        Self::new()
    }
}
