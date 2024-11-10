use std::{
    collections::HashMap,
    net::{SocketAddr, ToSocketAddrs}, str::FromStr,
};

use crate::sequence::SequenceNumber;

pub struct EstablishedConnection {
    // Sequenced by packet event
    pub current_send_seq_num: HashMap<String, SequenceNumber>,
    pub last_seq_num_recv: HashMap<String, SequenceNumber>,
    pub addr: SocketAddr,
}

impl EstablishedConnection {
    pub fn new(addr: SocketAddr) -> Self {
        Self {
            current_send_seq_num: HashMap::new(),
            last_seq_num_recv: HashMap::new(),
            addr 
        }
    }
}
