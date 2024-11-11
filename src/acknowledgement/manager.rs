use std::{collections::HashMap, net::ToSocketAddrs, time::{Duration, Instant}};

use crate::packet::PacketDelivery;

use super::packet::{AckNumber, AckPacket};

pub(crate) const ACK_RESET_LIMIT: AckNumber = 128_000;

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
            last_ack: 0,
            packets_waiting_on_ack: HashMap::new(),
            ack_retry_time: Duration::from_secs(2),
        }
    }

    /// Adds a packet that requires an ack to the waiting list, if the packet does not receive an
    /// ack it will be resent to its target
    pub(crate) fn add_awaiting_ack_packet_if_needed<A>(
        &mut self,
        packet_delivery: PacketDelivery,
        buf: Vec<u8>,
        addr: A,
    ) -> AckNumber
    where
        A: ToSocketAddrs + Into<String>,
    {
        if matches!(packet_delivery, PacketDelivery::Reliable)
            || matches!(packet_delivery, PacketDelivery::ReliableSequenced)
        {
            if self.last_ack >= ACK_RESET_LIMIT {
                self.last_ack = 0;
            }

            self.last_ack += 1;

            self.packets_waiting_on_ack
                .insert(self.last_ack, AckPacket::new(buf, Instant::now(), addr));

            self.last_ack
        } else {
            0
        }
    }
}

impl Default for AcknowledgementManager {
    fn default() -> Self {
        Self::new()
    }
}
