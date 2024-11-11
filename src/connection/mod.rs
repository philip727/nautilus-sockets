use std::{collections::HashMap, net::SocketAddr};

use crate::sequence::SequenceNumber;

pub(crate) struct EstablishedConnection {
    /// Each individual event has its own [seq number](crate::sequence::SequenceNumber)
    pub current_send_seq_num: HashMap<String, SequenceNumber>,
    /// The last seq number we received for that event
    pub last_seq_num_recv: HashMap<String, SequenceNumber>,
    /// The established [connection address](SocketAddr)
    pub addr: SocketAddr,
}

impl EstablishedConnection {
    /// Creates a new established connection to an address
    pub fn new(addr: SocketAddr) -> Self {
        Self {
            current_send_seq_num: HashMap::new(),
            last_seq_num_recv: HashMap::new(),
            addr,
        }
    }
}
