use std::{
    collections::VecDeque,
    marker::PhantomData,
    net::{SocketAddr, ToSocketAddrs, UdpSocket},
    str::FromStr,
    sync::{Arc, RwLock},
    time::Instant,
    u16,
};

use anyhow::anyhow;
use byteorder::{ByteOrder, LittleEndian};

use crate::{
    acknowledgement::{AckPacket, AcknowledgementManager},
    events::{CallbackArgs, EventEmitter},
    packet::{PacketDelivery, PACKET_ACK_DELIVERY},
    sequence::SequenceNumber,
};

pub type ReceivedPacket = (SocketAddr, Vec<u8>);
pub struct NautSocket<'socket, S>
where
    S: SocketType<'socket>,
{
    pub(crate) socket: UdpSocket,
    pub(crate) packet_queue: VecDeque<ReceivedPacket>,
    pub(crate) inner: Arc<RwLock<S>>,

    pub(crate) event_emitter: EventEmitter<S>,
    pub(crate) ack_manager: AcknowledgementManager,
    pub(crate) phantom: PhantomData<&'socket S>,
}

impl<'socket, S> NautSocket<'socket, S>
where
    S: SocketType<'socket>,
{
    pub const DELIVERY_TYPE_OFFSET: usize = 0;
    /// The amount of space in each packet for the delivery type
    pub const DELIVERY_TYPE_BUF: usize = 2;

    pub const SEQ_NUM_OFFSET: usize = 2;
    /// The amount of space in each packet for the sequence order if its a sequenced packet
    pub const SEQ_NUM_BUF: usize = 4;

    pub const ACK_NUM_OFFSET: usize = 6;
    /// The amount of space in each packet for the ack number
    pub const ACK_NUM_BUF: usize = 4;

    pub const EVENT_LEN_OFFSET: usize = 10;
    /// The amount of space in each packet for the length of the packet event
    pub const EVENT_LEN_BUF: usize = 4;

    pub const PACKET_PADDING: usize =
        Self::DELIVERY_TYPE_BUF + Self::SEQ_NUM_BUF + Self::ACK_NUM_BUF + Self::EVENT_LEN_BUF;

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

    // Retry ack packets if needed
    pub(crate) fn retry_ack_packets(&self) {
        for AckPacket {
            bytes,
            time_created,
            target,
        } in self.ack_manager.packets_waiting_on_ack.values()
        {
            if Instant::now().duration_since(*time_created) < self.ack_manager.ack_retry_time {
                continue;
            }

            println!("Retrying ack packet");
            let _ = self.socket.send_to(bytes, target);
        }
    }

    pub(crate) fn get_last_received_packet(&mut self) -> Option<ReceivedPacket> {
        self.packet_queue.pop_front()
    }

    pub(crate) fn get_delivery_type_from_packet(buf: &[u8]) -> u16 {
        LittleEndian::read_u16(
            &buf[Self::DELIVERY_TYPE_OFFSET..Self::DELIVERY_TYPE_OFFSET + Self::DELIVERY_TYPE_BUF],
        )
    }

    pub(crate) fn get_ack_num_from_packet(buf: &[u8]) -> u32 {
        LittleEndian::read_u32(&buf[Self::ACK_NUM_OFFSET..Self::ACK_NUM_OFFSET + Self::ACK_NUM_BUF])
    }

    pub(crate) fn get_seq_from_packet(buf: &[u8]) -> u32 {
        LittleEndian::read_u32(&buf[Self::SEQ_NUM_OFFSET..Self::SEQ_NUM_OFFSET + Self::SEQ_NUM_BUF])
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

    pub(crate) fn send_by_addr<A, D>(
        &mut self,
        event: &str,
        buf: &[u8],
        delivery: D,
        addr: A,
    ) -> anyhow::Result<()>
    where
        A: ToSocketAddrs + Into<String> + Clone,
        D: Into<PacketDelivery> + std::cmp::PartialEq<u16> + Copy,
    {
        if delivery == PACKET_ACK_DELIVERY {
            println!("Sending ack of packet");
            self.socket.send_to(buf, addr)?;
            return Ok(());
        }

        // Stays consistent with memory layout
        let pad = (4 - (event.len() % 4)) % 4;
        let padded_event_len = event.len() + pad;
        let total_len = Self::PACKET_PADDING + padded_event_len + buf.len();

        let mut packet = vec![0; total_len];
        // Inserts the packet delivery type into the packet
        LittleEndian::write_u16(
            &mut packet
                [Self::DELIVERY_TYPE_OFFSET..Self::DELIVERY_TYPE_OFFSET + Self::DELIVERY_TYPE_BUF],
            delivery.into() as u16,
        );

        if delivery == PacketDelivery::ReliableSequenced as u16
            || delivery == PacketDelivery::UnreliableSequenced as u16
        {
            let Ok(mut inner) = self.inner.write() else {
                return Err(anyhow!(
                    "Failed to get inner rwlock write guard for send by addr"
                ));
            };

            let addr = Into::<String>::into(addr.clone());
            let addr = SocketAddr::from_str(&addr).unwrap();
            let seq_num = inner.update_current_send_seq_num_for_event(&addr, event);

            if let Some(seq_num) = seq_num {
                LittleEndian::write_u32(
                    &mut packet[Self::SEQ_NUM_OFFSET..Self::SEQ_NUM_OFFSET + Self::SEQ_NUM_BUF],
                    seq_num,
                );
            }
        }

        // If its a reliable packet then we must assign it an ack number and insert it into
        // the awaiting ack list
        let ack_number = self.ack_manager.add_awaiting_ack_packet_if_needed(
            delivery.into(),
            packet.to_vec(),
            addr.clone(),
        );

        // If its an ack packet then we insert it
        LittleEndian::write_u32(
            &mut packet[Self::ACK_NUM_OFFSET..Self::ACK_NUM_OFFSET + Self::ACK_NUM_BUF],
            ack_number,
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

    pub(crate) fn send_ack_packet<A>(&self, addr: A, packet: &[u8])
    where
        A: ToSocketAddrs,
    {
        let mut buf = vec![0; 6];

        // Write that its a ack response to the packet
        LittleEndian::write_u16(
            &mut buf
                [Self::DELIVERY_TYPE_OFFSET..Self::DELIVERY_TYPE_OFFSET + Self::DELIVERY_TYPE_BUF],
            PACKET_ACK_DELIVERY,
        );

        // Get the ack num from the original packet
        let ack_num = Self::get_ack_num_from_packet(packet);
        // Write ack num into ack delivery packet
        LittleEndian::write_u32(&mut buf[2..6], ack_num);

        let _ = self.socket.send_to(&buf, addr);
    }
}

pub trait SocketType<'socket>: Default {
    fn update_current_send_seq_num_for_event(
        &mut self,
        addr: &SocketAddr,
        event: &str,
    ) -> Option<SequenceNumber>;

    fn last_recv_seq_num_for_event(
        &'socket mut self,
        addr: &SocketAddr,
        event: &str,
    ) -> Option<&'socket mut SequenceNumber>;
}
