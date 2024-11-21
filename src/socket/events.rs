/// The results of a socket running its events
pub enum SocketRunEventResult<V> {
    Ok,
    HadFailures(V)
}

/// The different type of events that can occur during a socket running its events
#[derive(Clone, Debug)]
pub enum SocketEvent {
    PacketDiscard(String),
    ReadPacketFail(String),
    SendPacketFail(String),
}
