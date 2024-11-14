use std::{net::ToSocketAddrs, ops::{Add, AddAssign}, time::Instant};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub(crate) struct AckNumber(u32);

impl AckNumber {
    pub fn new(value: u32) -> Self {
        Self(value)
    }

    pub fn raw(&self) -> u32 {
        self.0
    }
}

impl Add for AckNumber {
    type Output = AckNumber;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl AddAssign for AckNumber {
    fn add_assign(&mut self, rhs: Self) {
        *self = Self(self.0 + rhs.0)
    }
}

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
