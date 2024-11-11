use std::{
    collections::VecDeque,
    marker::PhantomData,
    net::{SocketAddr, ToSocketAddrs, UdpSocket},
    str::FromStr,
    sync::{Arc, RwLock},
};

use anyhow::anyhow;
use byteorder::{ByteOrder, LittleEndian};

use crate::{
    acknowledgement::AcknowledgementManager,
    connection::EstablishedConnection,
    events::EventEmitter,
    packet::{PacketDelivery, PACKET_ACK_DELIVERY},
    sequence::SequenceNumber,
    socket::{NautSocket, SocketType},
};

pub type ConnectionId = u16;
#[derive(Default)]
pub struct NautClient {
    pub(crate) server_connection: Option<EstablishedConnection>,
}

impl<'socket> SocketType<'socket> for NautClient {
    fn update_current_send_seq_num_for_event(
        &mut self,
        _addr: &std::net::SocketAddr,
        event: &str,
    ) -> Option<SequenceNumber> {
        let server_connection = self.server_connection.as_mut()?;

        let Some(seq) = server_connection.current_send_seq_num.get_mut(event) else {
            server_connection
                .current_send_seq_num
                .insert(event.to_owned(), 0);
            return Some(0);
        };

        *seq += 1;

        Some(*seq)
    }

    fn last_recv_seq_num_for_event(
        &'socket mut self,
        _addr: &std::net::SocketAddr,
        event: &str,
    ) -> Option<&'socket mut SequenceNumber> {
        let server_connection = self.server_connection.as_mut()?;

        if !server_connection.last_seq_num_recv.contains_key(event) {
            server_connection
                .last_seq_num_recv
                .insert(event.to_string(), 0);
        }

        let seq = server_connection.last_seq_num_recv.get_mut(event)?;

        Some(seq)
    }
}

impl<'socket> NautSocket<'socket, NautClient> {
    pub fn new<A>(addr: A) -> anyhow::Result<Self>
    where
        A: ToSocketAddrs,
    {
        let socket = UdpSocket::bind(addr)?;
        socket.set_nonblocking(true)?;

        let client = NautClient::default();
        let naut_socket = Self {
            socket,
            packet_queue: VecDeque::new(),
            inner: client,
            event_emitter: EventEmitter::new(),
            ack_manager: AcknowledgementManager::new(),
            phantom: PhantomData,
        };

        Ok(naut_socket)
    }

    pub fn connect_to<A>(&mut self, addr: A) -> anyhow::Result<()>
    where
        A: ToSocketAddrs + Into<String> + Clone,
    {
        let addr_str = Into::<String>::into(addr.clone());
        self.inner.server_connection = Some(EstablishedConnection::new(
            SocketAddr::from_str(addr_str.as_str()).unwrap(),
        ));

        Ok(self.socket().connect(addr)?)
    }

    pub fn send<D>(&mut self, event: &str, buf: &[u8], delivery: D) -> anyhow::Result<()>
    where
        D: Into<PacketDelivery> + std::cmp::PartialEq<u16> + Copy,
    {
        let server_addr = {
            self.inner.server_connection.as_ref().unwrap().addr
        };

        self.send_by_addr(event, buf, delivery, server_addr.to_string())?;

        Ok(())
    }

    pub fn run_events(&mut self) {
        while let Some((addr, packet)) = self.get_last_received_packet() {
            let delivery_type = Self::get_delivery_type_from_packet(&packet);

            if delivery_type == PACKET_ACK_DELIVERY {
                let ack_num = LittleEndian::read_u32(&packet[2..6]);
                self.ack_manager.packets_waiting_on_ack.remove(&ack_num);

                continue;
            }

            // Check size here instead of in poll as ack packets do not fit into padding
            if packet.len() < Self::PACKET_PADDING {
                continue;
            }

            let Ok(event) = Self::get_event_from_packet(&packet) else {
                continue;
            };

            let delivery_type = Into::<PacketDelivery>::into(delivery_type);

            // We must send an ack packet with the packet num back to the sender
            if delivery_type == PacketDelivery::Reliable
                || delivery_type == PacketDelivery::ReliableSequenced
            {
                self.send_ack_packet(addr, &packet);
            }

            // If its a sequenced packet we must make sure its the latest packet in sequence
            if delivery_type == PacketDelivery::ReliableSequenced
                || delivery_type == PacketDelivery::UnreliableSequenced
            {
                let seq_num = Self::get_seq_from_packet(&packet);
                if let Some(last_recv_seq_num) = self.inner.last_recv_seq_num_for_event(&addr, &event) {
                    // Discard packet
                    if seq_num < *last_recv_seq_num {
                        println!(
                            "Discarding {event} packet, last recv: {} recv: {}",
                            *last_recv_seq_num, seq_num
                        );
                        continue;
                    }

                    *last_recv_seq_num = seq_num;
                };
            }

            let bytes = Self::get_packet_bytes(&packet);
            self.event_emitter.emit_event(&event, &self.inner, (addr, &bytes));
        }

        // Retry ack packets
        self.retry_ack_packets();
    }
}
