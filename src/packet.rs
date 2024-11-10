#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PacketDelivery {
    Unreliable = 0,
    UnreliableSequenced = 1,
    Reliable = 2,
    ReliableSequenced = 3,
}

impl From<u16> for PacketDelivery {
    fn from(value: u16) -> Self {
        match value {
            0 => PacketDelivery::Unreliable,
            1 => PacketDelivery::UnreliableSequenced,
            2 => PacketDelivery::Reliable,
            3 => PacketDelivery::ReliableSequenced,
            // Just default to unreliable on unknown types of delivery
            _ => PacketDelivery::Unreliable,
        }
    }
}

impl PartialEq<u16> for PacketDelivery {
    fn eq(&self, other: &u16) -> bool {
        *self as u16 == *other
    }
}

pub(crate) const PACKET_ACK_DELIVERY: u16 = 10;
