use std::{
    collections::HashMap,
    net::ToSocketAddrs,
    time::{Duration, Instant},
};

use crate::packet::PacketDelivery;

pub type AckNumber = u32;
pub const ACK_RESET_LIMIT: AckNumber = 128_000;

pub struct AckPacket {
    pub bytes: Vec<u8>,
    pub time_created: Instant,
    pub target: String,
}

impl AckPacket {
    pub fn new<A>(bytes: Vec<u8>, created: Instant, target: A) -> Self
    where
        A: ToSocketAddrs + Into<String>,
    {
        Self {
            bytes,
            time_created: created,
            target: target.into(),
        }
    }
}

pub struct AcknowledgementManager {
    pub last_ack: AckNumber,
    pub packets_waiting_on_ack: HashMap<AckNumber, AckPacket>,
    pub ack_retry_time: Duration,
}

impl AcknowledgementManager {
    pub fn new() -> Self {
        Self {
            last_ack: 0,
            packets_waiting_on_ack: HashMap::new(),
            ack_retry_time: Duration::from_secs(2),
        }
    }

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
