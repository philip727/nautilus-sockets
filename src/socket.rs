use std::{
    collections::VecDeque,
    marker::PhantomData,
    net::{SocketAddr, ToSocketAddrs, UdpSocket},
    str::FromStr,
    time::Instant,
};

use byteorder::{ByteOrder, LittleEndian};

use crate::{
    acknowledgement::{manager::AcknowledgementManager, packet::AckPacket},
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
    pub(crate) inner: S,

    pub(crate) event_emitter: EventEmitter<S>,
    pub(crate) ack_manager: AcknowledgementManager,
    pub(crate) phantom: PhantomData<&'socket S>,
}

impl<'socket, S> NautSocket<'socket, S>
where
    S: SocketType<'socket>,
{
    /// The offset in the packet of the delivery type
    pub const DELIVERY_TYPE_OFFSET: usize = 0;
    /// The amount of space in each packet for the delivery type
    pub const DELIVERY_TYPE_BUF: usize = 2;

    /// The offset in the packet of the sequence number if its got a sequenced delivery type
    pub const SEQ_NUM_OFFSET: usize = 2;
    /// The amount of space in each packet for the sequence order if its a sequenced packet
    pub const SEQ_NUM_BUF: usize = 4;

    /// The offset in the packet of the acknowldegement number
    pub const ACK_NUM_OFFSET: usize = 6;
    /// The amount of space in each packet for the ack number
    pub const ACK_NUM_BUF: usize = 4;

    /// The offset in the packet for the length of the event title
    pub const EVENT_LEN_OFFSET: usize = 10;
    /// The amount of space in each packet for the length of the event title
    pub const EVENT_LEN_BUF: usize = 4;

    pub const PACKET_PADDING: usize =
        Self::DELIVERY_TYPE_BUF + Self::SEQ_NUM_BUF + Self::ACK_NUM_BUF + Self::EVENT_LEN_BUF;

    /// Reference to the [raw socket](Self::socket)
    pub fn socket(&self) -> &UdpSocket {
        &self.socket
    }

    /// Mutable reference to the [raw socket](Self::socket)
    pub fn socket_mut(&mut self) -> &mut UdpSocket {
        &mut self.socket
    }

    /// Polls the received packets and pushes them to the [packet queue](Self::packet_queue)
    pub fn poll(&mut self) {
        let mut buf = vec![0; 1024];
        while let Ok((size, addr)) = self.socket.recv_from(&mut buf) {
            let buf = buf[0..size].to_vec();
            self.packet_queue.push_back((addr, buf));
        }
    }

    /// Retries a packet after [retry time](AcknowledgementManager::ack_retry_time)
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

            let _ = self.socket.send_to(bytes, target);
        }
    }

    /// Pops a packet from the front of the [packet queue](Self::packet_queue)
    pub(crate) fn get_last_received_packet(&mut self) -> Option<ReceivedPacket> {
        self.packet_queue.pop_front()
    }

    /// Gets the delivery type of the packet
    pub(crate) fn get_delivery_type_from_packet(buf: &[u8]) -> u16 {
        LittleEndian::read_u16(
            &buf[Self::DELIVERY_TYPE_OFFSET..Self::DELIVERY_TYPE_OFFSET + Self::DELIVERY_TYPE_BUF],
        )
    }

    /// Gets the acknowledgement number from the packet
    pub(crate) fn get_ack_num_from_packet(buf: &[u8]) -> u32 {
        LittleEndian::read_u32(&buf[Self::ACK_NUM_OFFSET..Self::ACK_NUM_OFFSET + Self::ACK_NUM_BUF])
    }

    /// Gets the sequence number from the packet
    pub(crate) fn get_seq_from_packet(buf: &[u8]) -> u32 {
        LittleEndian::read_u32(&buf[Self::SEQ_NUM_OFFSET..Self::SEQ_NUM_OFFSET + Self::SEQ_NUM_BUF])
    }

    /// Get the event title from the packet
    pub(crate) fn get_event_from_packet(buf: &[u8]) -> anyhow::Result<String> {
        let length = LittleEndian::read_u32(
            &buf[Self::EVENT_LEN_OFFSET..Self::EVENT_LEN_OFFSET + Self::EVENT_LEN_BUF],
        ) as usize;

        let event_offset = Self::EVENT_LEN_OFFSET + Self::EVENT_LEN_BUF;

        Ok(String::from_utf8(
            buf[event_offset..event_offset + length].to_vec(),
        )?)
    }

    /// Gets the remaining packet bytes
    pub(crate) fn get_packet_bytes(buf: &[u8]) -> Vec<u8> {
        let length = LittleEndian::read_u32(
            &buf[Self::EVENT_LEN_OFFSET..Self::EVENT_LEN_OFFSET + Self::EVENT_LEN_BUF],
        ) as usize;
        let pad = (4 - (length % 4)) % 4;

        let event_offset = Self::EVENT_LEN_OFFSET + Self::EVENT_LEN_BUF;
        let bytes_offset = event_offset + length + pad;

        buf[bytes_offset..].to_vec()
    }

    /// Sends a packet to a [socket address](SocketAddr) and inserts the [packet delivery type](PacketDelivery), [AckNumber] and [SequenceNumber] and the
    /// remaining bytes sent for the actual event
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
            let addr = Into::<String>::into(addr.clone());
            let addr = SocketAddr::from_str(&addr).unwrap();
            let seq_num = self
                .inner
                .update_current_send_seq_num_for_event(&addr, event);

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

    /// Run a function as a callback when a certain event is sent
    ///
    /// # Examples
    ///
    /// ```
    /// // When the client recieves a "hello" event it will print the bytes received
    /// client.on("hello", |_client, (_addr, packet)| {
    ///     println!("hello bytes {:?}", packet);
    /// });
    /// ```
    pub fn on<F>(&mut self, event: &str, cb: F)
    where
        F: Fn(&S, CallbackArgs) + Send + Sync + 'static,
    {
        self.event_emitter.register_event(event, cb);
    }

    /// Sends an [acknowledgement packet](AckPacket) to the [address](SocketAddr)
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

/// Used for the [nautlis socket](NautSocket)
/// A socket must have a way to handle sequenced packets for clients. Otherwise, it will manage
/// everything the same way
pub trait SocketType<'socket>: Default {
    /// Updates the current [sequence number](SequenceNumber) for that specific event and should be
    /// handled on a per client basis
    fn update_current_send_seq_num_for_event(
        &mut self,
        addr: &SocketAddr,
        event: &str,
    ) -> Option<SequenceNumber>;

    /// Returns a mutable reference to the last received sequence number, it should be changed to
    /// the newest sequence number if the last received sequence number is lower
    fn last_recv_seq_num_for_event(
        &'socket mut self,
        addr: &SocketAddr,
        event: &str,
    ) -> Option<&'socket mut SequenceNumber>;
}
