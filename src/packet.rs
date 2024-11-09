#[derive(Clone, Copy)]
pub enum PacketDelivery {
    Unreliable = 0,
    UnreliableSequenced = 1,
    Reliable = 2,
    ReliableSequenced = 3,
}
