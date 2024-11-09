use std::{
    collections::VecDeque,
    net::{ToSocketAddrs, UdpSocket},
    sync::{Arc, RwLock},
};

use byteorder::{ByteOrder, LittleEndian};

use crate::{
    events::EventEmitter,
    packet::PacketDelivery,
    socket::{NautSocket, SocketType},
};

#[derive(Default)]
pub struct NautClient {}

impl SocketType for NautClient {}

impl NautSocket<NautClient> {
    pub fn new<A>(addr: A) -> anyhow::Result<Self>
    where
        A: ToSocketAddrs,
    {
        let socket = UdpSocket::bind(addr)?;
        socket.set_nonblocking(true)?;

        let client = Arc::new(RwLock::new(NautClient::default()));
        let mut naut_socket = Self {
            socket,
            packet_queue: VecDeque::new(),
            inner: Arc::clone(&client),
            event_emitter: EventEmitter::new(Arc::clone(&client)),
        };

        naut_socket.event_emitter = EventEmitter::new(Arc::clone(&client));

        Ok(naut_socket)
    }

    pub fn connect_to<A>(&self, addr: A) -> anyhow::Result<()>
    where
        A: ToSocketAddrs,
    {
        Ok(self.socket().connect(addr)?)
    }

    pub fn send(
        &self,
        event: &str,
        buf: &[u8],
        delivery: PacketDelivery,
    ) -> anyhow::Result<()> {
        // Stays consistent with memory layout
        let pad = (4 - (event.len() % 4)) % 4;
        let padded_event_len = event.len() + pad;
        let total_len = Self::DELIVERY_TYPE_BUF
            + Self::DELIVERY_SEQ_BUF
            + Self::EVENT_LEN_BUF
            + padded_event_len
            + buf.len();

        let mut packet = vec![0; total_len];
        // Inserts the packet delivery type into the packet
        LittleEndian::write_u16(
            &mut packet
                [Self::DELIVERY_TYPE_OFFSET..Self::DELIVERY_TYPE_OFFSET + Self::DELIVERY_TYPE_BUF],
            delivery as u16,
        );

        LittleEndian::write_u32(
            &mut packet
                [Self::DELIVERY_SEQ_OFFSET..Self::DELIVERY_SEQ_OFFSET + Self::DELIVERY_SEQ_BUF],
            0, // We will get the sequence number
        );

        // Inserts the length of the event string into the packet
        LittleEndian::write_u32(
            &mut packet[Self::EVENT_LEN_OFFSET..Self::EVENT_LEN_OFFSET + Self::EVENT_LEN_BUF],
            event.len() as u32,
        );

        let event_offset = Self::EVENT_LEN_OFFSET + Self::EVENT_LEN_BUF;
        // Copies the event into the packet
        packet[event_offset..event_offset + event.len()].copy_from_slice(event.as_bytes());

        // Inserts the byte buf into the packet buf
        let bytes_offset = event_offset + padded_event_len;
        packet[bytes_offset..].copy_from_slice(buf);

        self.socket().send(&packet)?;

        Ok(())
    }

    pub fn run_events(&mut self) {
        while let Some((addr, packet)) = self.get_last_received_packet() {
            let Ok(event) = Self::get_event_from_packet(&packet) else {
                continue;
            };

            let bytes = Self::get_packet_bytes(&packet);
            self.event_emitter.emit_event(&event, (addr, &bytes));
        }
    }
}
