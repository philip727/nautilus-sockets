/// Describes how a packet will reach its target
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PacketDelivery {
    /// A packet which has no guarantee of reaching its target, and if it doesn't it will be
    /// forgotten
    Unreliable = 0,
    /// A packet which has no guarantee of reaching its target, and will be discarded if not the
    /// latest packet in sequence or will be forgotten if it does not reach the target
    UnreliableSequenced = 1,
    /// A packet which will require an acknowledgement. If the package receives no ack it will be
    /// resent to the target
    Reliable = 2,
    /// A packet which will require an acknowledgement. If the package receives no ack it will be
    /// resent to the target, however the packet may be discared if not the latest packet
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

/// The packet delivery type for an acknowledgement packet
pub(crate) const PACKET_ACK_DELIVERY: u16 = 10;
