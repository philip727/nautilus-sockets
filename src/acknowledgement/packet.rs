use std::{net::ToSocketAddrs, time::Instant};

pub(crate) type AckNumber = u32;

/// A packet awaiting acknowledgement
pub(crate) struct AckPacket {
    /// The original bytes of the packet
    pub bytes: Vec<u8>,
    /// The time the packet was sent
    pub time_created: Instant,
    /// The target of the packet
    pub target: String,
}

impl AckPacket {
    /// Creates a new packet awaiting acknowledgement
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
