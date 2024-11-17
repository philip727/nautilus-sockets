use std::{
    collections::VecDeque,
    marker::PhantomData,
    net::{SocketAddr, ToSocketAddrs, UdpSocket},
    str::FromStr,
};

use byteorder::{ByteOrder, LittleEndian};

use crate::{
    acknowledgement::{manager::AcknowledgementManager, packet::AckNumber},
    connection::EstablishedConnection,
    events::EventEmitter,
    packet::{IntoPacketDelivery, PacketDelivery},
    persistent::storage::PersistentStorage,
    sequence::SequenceNumber,
    socket::{events::SocketEvent, NautSocket, SocketType},
};

pub type ConnectionId = u16;
#[derive(Default)]
pub struct NautClient {
    /// The [nautilus server](crate::server::NautServer) we are connected to
    server_connection: Option<EstablishedConnection>,
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
                .insert(event.to_owned(), SequenceNumber::new(0));
            return Some(SequenceNumber::new(0));
        };

        *seq += SequenceNumber::new(1);

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
                .insert(event.to_string(), SequenceNumber::new(0));
        }

        let seq = server_connection.last_seq_num_recv.get_mut(event)?;

        Some(seq)
    }
}

impl<'socket> NautSocket<'socket, NautClient> {
    /// Creates a new [event listening socket](crate::socket::NautSocket) with a [client](NautClient) type
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
            socket_events: Vec::new(),
            persistent: PersistentStorage::new(),
        };

        Ok(naut_socket)
    }

    /// Gets the [address](SocketAddr) of the (server)[crate::server::NautServer] we are connected
    /// to
    pub fn get_server_address(&self) -> Option<&SocketAddr> {
        Some(&self.inner.server_connection.as_ref()?.addr)
    }

    /// Establishes a connection to another [nautilus compatible socket](crate::socket::NautSocket)
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

    /// Sends an event message to the [server](crate::server::NautServer) we are connected to
    pub fn send(
        &mut self,
        event: &str,
        buf: &[u8],
        delivery: PacketDelivery,
    ) -> anyhow::Result<()> {
        let server_addr = { self.inner.server_connection.as_ref().unwrap().addr };

        self.send_by_addr(event, buf, delivery, server_addr.to_string())?;

        Ok(())
    }

    /// Gets the packets from the packet queue and will handle returning
    /// [ack packets](crate::acknowledgement::packet::AckPacket), resolving sequenced packets and emitting
    /// listening events
    pub fn run_events(&mut self) {
        let event_emitter = std::mem::take(&mut self.event_emitter);
        let event_emitter_ref = &event_emitter;
        while let Some((addr, packet)) = self.oldest_packet_in_queue() {
            let delivery_type = Self::get_delivery_type_from_packet(&packet);
            let Ok(delivery_type) =
                <PacketDelivery as IntoPacketDelivery<u16>>::into_packet_delivery(delivery_type)
            else {
                continue;
            };

            // We have received acknowledgement of a packet we have sent
            if delivery_type == PacketDelivery::ack_delivery() {
                let ack_num = AckNumber::new(LittleEndian::read_u32(&packet[2..6]));
                self.ack_manager.packets_waiting_on_ack.remove(&ack_num);

                continue;
            }

            // Check size here instead of in poll as ack packets do not fit into padding
            if packet.len() < Self::PACKET_PADDING {
                continue;
            }

            // Gets the event title from the packet
            let Ok(event) = Self::get_event_from_packet(&packet) else {
                continue;
            };

            // Send a packet  to acknowledge the sender we have recieved their packet
            if delivery_type.is_reliable() {
                if let Err(e) = self.send_ack_packet(addr, &packet) {
                    self.socket_events
                        .push(SocketEvent::SendPacketFail(e.to_string()))
                }
            }

            // If its a sequenced packet we must make sure its the latest packet in sequence
            if delivery_type.is_sequenced() {
                let seq_num = Self::get_seq_from_packet(&packet);
                if let Some(last_recv_seq_num) =
                    self.inner.last_recv_seq_num_for_event(&addr, &event)
                {
                    // Discard packet
                    if seq_num < *last_recv_seq_num {
                        println!(
                            "Discarding {event} packet, last recv: {:?} recv: {:?}",
                            *last_recv_seq_num, seq_num
                        );
                        continue;
                    }

                    *last_recv_seq_num = seq_num;
                };
            }

            let bytes = Self::get_packet_bytes(&packet);

            // Emits the event to the event listeners
            event_emitter_ref.emit_event(&event, self, (addr, &bytes));
        }

        event_emitter_ref.emit_polled_events(self);

        // Retry ack packets
        self.socket_events.clear();
        self.retry_ack_packets();

        self.event_emitter = event_emitter;
    }
}
