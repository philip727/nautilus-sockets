use std::{
    collections::VecDeque,
    net::{SocketAddr, ToSocketAddrs, UdpSocket},
    sync::{Arc, RwLock},
};

use byteorder::{ByteOrder, LittleEndian};

use crate::{
    events::{Callback, CallbackArgs, EventEmitter},
    packet::PacketDelivery,
};

pub type ReceivedPacket = (SocketAddr, Vec<u8>);
pub struct NautSocket<S>
where
    S: SocketType,
{
    pub(crate) socket: UdpSocket,
    pub(crate) packet_queue: VecDeque<ReceivedPacket>,
    pub(crate) inner: Arc<RwLock<S>>,

    pub(crate) event_emitter: EventEmitter<S>,
}

impl<S> NautSocket<S>
where
    S: SocketType,
{
    pub const DELIVERY_TYPE_OFFSET: usize = 0;
    /// The amount of space in each packet for the delivery type
    pub const DELIVERY_TYPE_BUF: usize = 2;

    pub const DELIVERY_SEQ_OFFSET: usize = 2;
    /// The amount of space in each packet for the sequence order if its a sequenced packet
    pub const DELIVERY_SEQ_BUF: usize = 4;

    pub const EVENT_LEN_OFFSET: usize = 6;
    /// The amount of space in each packet for the length of the packet event
    pub const EVENT_LEN_BUF: usize = 4;

    pub fn socket(&self) -> &UdpSocket {
        &self.socket
    }

    pub fn socket_mut(&mut self) -> &mut UdpSocket {
        &mut self.socket
    }

    pub fn poll(&mut self) -> anyhow::Result<()> {
        let mut buf = vec![0; 1024];
        while let Ok((size, addr)) = self.socket.recv_from(&mut buf) {
            let buf = buf[0..size].to_vec();
            self.packet_queue.push_back((addr, buf));
        }

        Ok(())
    }

    pub(crate) fn get_last_received_packet(&mut self) -> Option<ReceivedPacket> {
        self.packet_queue.pop_front()
    }

    pub(crate) fn get_event_from_packet(buf: &[u8]) -> anyhow::Result<String> {
        let length = LittleEndian::read_u32(
            &buf[Self::EVENT_LEN_OFFSET..Self::EVENT_LEN_OFFSET + Self::EVENT_LEN_BUF],
        ) as usize;

        let event_offset = Self::EVENT_LEN_OFFSET + Self::EVENT_LEN_BUF;

        Ok(String::from_utf8(
            buf[event_offset..event_offset + length].to_vec(),
        )?)
    }

    pub(crate) fn get_packet_bytes(buf: &[u8]) -> Vec<u8> {
        let length = LittleEndian::read_u32(
            &buf[Self::EVENT_LEN_OFFSET..Self::EVENT_LEN_OFFSET + Self::EVENT_LEN_BUF],
        ) as usize;
        let pad = (4 - (length % 4)) % 4;

        let event_offset = Self::EVENT_LEN_OFFSET + Self::EVENT_LEN_BUF;
        let bytes_offset = event_offset + length + pad;

        buf[bytes_offset..].to_vec()
    }

    pub(crate) fn send_by_addr<A>(
        &self,
        event: &str,
        buf: &[u8],
        delivery: PacketDelivery,
        addr: A,
    ) -> anyhow::Result<()>
    where
        A: ToSocketAddrs,
    {
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

        self.socket.send_to(&packet, addr)?;

        Ok(())
    }

    pub fn on<F>(&mut self, event: &str, cb: F)
    where
        F: Fn(&Arc<RwLock<S>>, CallbackArgs) + Send + Sync + 'static,
    {
        self.event_emitter.register_event(event, cb);
    }
}

pub trait SocketType: Default {}
