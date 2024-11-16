use anyhow::{anyhow, Ok};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct SocketDelivery;

/// Describes how a packet will reach its target
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
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

    #[allow(private_interfaces)]
    /// The packet delivery type for an acknowledgement packet
    AckDelivery(SocketDelivery) = 10,

    /// In place packet delivery type to request the details of the server without establishing a
    /// connection
    #[allow(private_interfaces)]
    DetailRequest(SocketDelivery) = 11,
}

impl PacketDelivery {
    /// Creates a packet delivery type for ack since it's a private interface
    pub(crate) fn ack_delivery() -> Self {
        Self::AckDelivery(SocketDelivery)
    }

    /// Creates a packet delivery type for detail request since it's a private interface
    pub(crate) fn detail_request() -> Self {
        Self::AckDelivery(SocketDelivery)
    }

    /// Is a reliable delivery type
    pub fn is_reliable(&self) -> bool {
        *self == Self::Reliable || *self == Self::ReliableSequenced
    }

    /// Is an unreliable delivery type
    pub fn is_unreliable(&self) -> bool {
        *self == Self::UnreliableSequenced || *self == Self::Unreliable
    }

    /// Is a sequenced delivery type
    pub fn is_sequenced(&self) -> bool {
        *self == Self::ReliableSequenced || *self == Self::UnreliableSequenced
    }
}

impl IntoPacketDelivery<u16> for PacketDelivery {
    fn into_packet_delivery(value: u16) -> anyhow::Result<Self> {
        match value {
            0 => Ok(PacketDelivery::Unreliable),
            1 => Ok(PacketDelivery::UnreliableSequenced),
            2 => Ok(PacketDelivery::Reliable),
            3 => Ok(PacketDelivery::ReliableSequenced),
            10 => Ok(PacketDelivery::ack_delivery()),
            11 => Ok(PacketDelivery::detail_request()),
            _ => Err(anyhow!(
                "Cannot turn value {value} into type of PacketDelivery"
            )),
        }
    }

    fn packet_delivery_as(&self) -> anyhow::Result<u16> {
        match self {
            PacketDelivery::Unreliable => Ok(0),
            PacketDelivery::UnreliableSequenced => Ok(1),
            PacketDelivery::Reliable => Ok(2),
            PacketDelivery::ReliableSequenced => Ok(3),
            PacketDelivery::AckDelivery(SocketDelivery) => Ok(10),
            PacketDelivery::DetailRequest(SocketDelivery) => Ok(11),
        }
    }
}

pub trait IntoPacketDelivery<T> {
    fn into_packet_delivery(value: T) -> anyhow::Result<Self>
    where
        Self: Sized;

    fn packet_delivery_as(&self) -> anyhow::Result<T>;
}
