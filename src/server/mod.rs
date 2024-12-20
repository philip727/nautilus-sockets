pub mod config;
use std::{
    collections::{HashMap, VecDeque},
    marker::PhantomData,
    net::{SocketAddr, ToSocketAddrs, UdpSocket},
    time::{Duration, Instant},
};

use anyhow::anyhow;
use byteorder::{ByteOrder, LittleEndian};
use config::ServerConfig;

use crate::{
    acknowledgement::{manager::AcknowledgementManager, packet::AckNumber},
    client::ConnectionId,
    connection::EstablishedConnection,
    events::EventEmitter,
    packet::{IntoPacketDelivery, PacketDelivery},
    persistent::storage::PersistentStorage,
    sequence::SequenceNumber,
    socket::{events::SocketEvent, NautSocket, SocketType},
};

// Incremental Id
pub struct NautServer {
    max_connections: u8,

    connection_addr_to_id: HashMap<SocketAddr, ConnectionId>,
    connection_id_to_addr: HashMap<ConnectionId, SocketAddr>,
    connections: HashMap<ConnectionId, EstablishedConnection>,

    time_outs: HashMap<ConnectionId, Instant>,

    next_id: ConnectionId,
    freed_ids: VecDeque<ConnectionId>,

    idle_connection_timeout: Duration,

    server_events: VecDeque<ServerEvent>,
}

impl NautServer {
    pub fn new(config: ServerConfig) -> Self {
        Self {
            max_connections: config.max_connections,
            idle_connection_timeout: config.idle_connection_time,
            ..Default::default()
        }
    }

    /// Clones a vector of all connection ids
    pub fn clients(&self) -> Vec<ConnectionId> {
        self.connections.keys().cloned().collect::<Vec<ConnectionId>>()
    }

    /// Gets the [client's id](ConnectionId) from an [address](SocketAddr)
    pub fn get_client_addr(&self, id: &ConnectionId) -> Option<&SocketAddr> {
        self.connection_id_to_addr.get(id)
    }

    /// Gets the [client's address](SocketAddr) from an [id](ConnectionId)
    pub fn get_client_id(&self, addr: &SocketAddr) -> Option<&ConnectionId> {
        self.connection_addr_to_id.get(addr)
    }

    /// Gets an iterator to all [server events](ServerEvent) in the queue, this will not remove any from queue
    pub fn iter_server_events(&self) -> std::collections::vec_deque::Iter<'_, ServerEvent> {
        self.server_events.iter()
    }

    /// Gets the max amount of connections the server can handle
    pub fn get_max_connections(&self) -> u8 {
        self.max_connections
    }

    /// Gets the current amount of established connections
    pub fn get_current_connections(&self) -> u8 {
        self.connections.len() as u8
    }

    /// Checks if a client has not sent a packet for the (idle time)[Self::idle_connection_timeout]
    pub(crate) fn any_client_needs_freeing(&self) -> Option<Vec<ConnectionId>> {
        let mut ids = Vec::new();
        for (id, time) in self.time_outs.iter() {
            if Instant::now().duration_since(*time) < self.idle_connection_timeout {
                continue;
            }
            ids.push(*id);
        }

        if ids.is_empty() {
            return None;
        }

        Some(ids)
    }

    /// Frees a client up to the server
    pub(crate) fn free_client(&mut self, id: ConnectionId) {
        self.freed_ids.push_back(id);
        let Some(addr) = self.connection_id_to_addr.remove(&id) else {
            println!("Failed to find address of idle'd client with id: {id}");
            return;
        };

        self.connection_addr_to_id.remove(&addr);
        self.time_outs.remove(&id);
        self.connections.remove(&id);
    }

    /// Closes a connection with a client and pushes a [client disconnected event](ServerEvent::OnClientDisconnected)
    /// to the server events queue
    pub fn close_connection_with_client(&mut self, id: ConnectionId) {
        self.free_client(id);
        self.server_events
            .push_back(ServerEvent::OnClientDisconnected(id));
    }

    /// Establishes a new connection to a new [socket address](SocketAddr) and pushes a
    /// [client connected event](ServerEvent::OnClientConnected) to the server events queue
    pub(crate) fn establish_new_connection(&mut self, addr: SocketAddr) {
        // Gets a new client id
        let client_id = {
            if let Some(client_id) = self.freed_ids.pop_front() {
                client_id
            } else {
                let client_id = self.next_id;
                self.next_id += 1;
                client_id
            }
        };

        self.connection_addr_to_id.insert(addr, client_id);
        self.connection_id_to_addr.insert(client_id, addr);
        self.connections
            .insert(client_id, EstablishedConnection::new(addr));

        self.server_events
            .push_back(ServerEvent::OnClientConnected(client_id));
    }
}

impl Default for NautServer {
    fn default() -> Self {
        Self {
            max_connections: 128,
            connections: Default::default(),
            connection_addr_to_id: Default::default(),
            connection_id_to_addr: Default::default(),
            time_outs: Default::default(),
            next_id: Default::default(),
            freed_ids: VecDeque::new(),
            idle_connection_timeout: Duration::from_secs(20),
            server_events: VecDeque::new(),
        }
    }
}

impl<'socket> NautSocket<'socket, NautServer> {
    /// Creates a new [event listening socket](crate::socket::NautSocket) with a
    /// [server](NautServer) type
    pub fn new<A>(addr: A, config: ServerConfig) -> anyhow::Result<Self>
    where
        A: ToSocketAddrs,
    {
        let socket = UdpSocket::bind(addr)?;
        socket.set_nonblocking(true)?;

        let server = NautServer::new(config);
        let event_emitter = EventEmitter::new();
        Ok(Self {
            socket,
            packet_queue: VecDeque::new(),
            inner: server,
            event_emitter,
            ack_manager: AcknowledgementManager::new(),
            phantom: PhantomData,
            socket_events: Vec::new(),
            persistent: PersistentStorage::new(),
        })
    }

    /// Gets a reference to the [server](NautServer)
    pub fn server(&self) -> &NautServer {
        &self.inner
    }

    /// Gets a mutable reference to the [server](NautServer)
    pub fn server_mut(&mut self) -> &mut NautServer {
        &mut self.inner
    }

    /// Gets the packets from the packet queue and will handle returning
    /// [ack packets](crate::acknowledgement::packet::AckPacket), resolving sequenced packets, emitting
    /// listening events, establishing new connections and disconnecting idling clients
    pub fn run_events(&mut self) {
        // Disconnect idle clients
        if let Some(ids_to_free) = self.inner.any_client_needs_freeing() {
            for id in ids_to_free.iter() {
                self.inner.free_client(*id);

                self.inner
                    .server_events
                    .push_back(ServerEvent::OnClientTimeout(*id));
            }
        }

        let event_emitter = std::mem::take(&mut self.event_emitter);
        let event_emitter_ref = &event_emitter;
        while let Some((addr, packet)) = self.oldest_packet_in_queue() {
            let Some(delivery_type) = Self::get_delivery_type_from_packet(&packet) else {
                self.socket_events
                    .push(SocketEvent::ReadPacketFail("No delivery type".to_string()));
                continue;
            };

            let Ok(delivery_type) =
                <PacketDelivery as IntoPacketDelivery<u16>>::into_packet_delivery(delivery_type)
            else {
                self.socket_events
                    .push(SocketEvent::ReadPacketFail(String::from(
                        "Failed to read packet due to invalid delivery type",
                    )));
                continue;
            };

            // We must check if the packet is of ack delivery first because ack packets do not have
            // the same byte size as a normal packet
            if delivery_type == PacketDelivery::ack_delivery() {
                let ack_num = AckNumber::new(LittleEndian::read_u32(&packet[2..6]));
                self.ack_manager.packets_waiting_on_ack.remove(&ack_num);

                continue;
            }

            // Check size here instead of in poll as ack packets do not fit into padding
            if packet.len() < Self::PACKET_PADDING {
                continue;
            }

            // Send a packet  to acknowledge the sender we have recieved their packet
            if delivery_type.is_reliable() {
                if let Err(e) = self.send_ack_packet(addr, &packet) {
                    self.socket_events
                        .push(SocketEvent::SendPacketFail(e.to_string()))
                }
            }

            let Ok(event) = Self::get_event_from_packet(&packet) else {
                continue;
            };

            if delivery_type.is_sequenced() {
                let Some(seq_num) = Self::get_seq_from_packet(&packet) else {
                    self.socket_events.push(SocketEvent::ReadPacketFail(
                        "No sequence number in sequenced packet".to_string(),
                    ));
                    continue;
                };

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

            // Just ignore the packet and dont establish connection as its maxed out
            if self.inner.connections.len() >= self.inner.max_connections as usize {
                continue;
            }

            // Establishes a connection with a client if not already established
            if !self.inner.connection_addr_to_id.contains_key(&addr) {
                self.inner.establish_new_connection(addr);
            }

            let Some(client) = self.inner.connection_addr_to_id.get(&addr) else {
                continue;
            };

            let client = *client;
            self.inner.time_outs.insert(client, Instant::now());

            let bytes = Self::get_packet_bytes(&packet).unwrap_or(Default::default());
            event_emitter_ref.emit_event(&event, self, (addr, &bytes));
        }

        // Emit all polled events
        event_emitter.emit_polled_events(self);

        // Clear server events this time around
        self.inner.server_events.clear();
        self.socket_events.clear();

        // Retry ack packets
        self.retry_ack_packets();

        self.event_emitter = event_emitter;
    }

    /// Sends an event message to all [established connections](EstablishedConnection)
    pub fn broadcast(&mut self, event: &str, buf: &[u8], delivery: PacketDelivery) {
        let connection_ids: Vec<ConnectionId> =
            { self.inner.connection_id_to_addr.keys().cloned().collect() };

        for id in connection_ids {
            let _ = self.send(event, buf, delivery, id);
        }
    }

    pub fn broadcast_except(
        &mut self,
        event: &str,
        buf: &[u8],
        delivery: PacketDelivery,
        excluded: Vec<ConnectionId>,
    ) {
        let connection_ids: Vec<ConnectionId> =
            { self.inner.connection_id_to_addr.keys().cloned().collect() };

        for id in connection_ids {
            if excluded.contains(&id) {
                continue;
            }

            let _ = self.send(event, buf, delivery, id);
        }
    }

    /// Sends an event message to the [server](crate::server::NautServer) we are connected to
    pub fn send(
        &mut self,
        event: &str,
        buf: &[u8],
        delivery: PacketDelivery,
        client: ConnectionId,
    ) -> anyhow::Result<()> {
        let addr = {
            *self
                .inner
                .connection_id_to_addr
                .get(&client)
                .ok_or(anyhow!(
                    "There is no associated address with this client id"
                ))?
        };

        let _ = self.send_by_addr(event, buf, delivery, addr.to_string());

        Ok(())
    }
}

impl<'socket> SocketType<'socket> for NautServer {
    fn last_recv_seq_num_for_event(
        &'socket mut self,
        addr: &std::net::SocketAddr,
        event: &str,
    ) -> Option<&'socket mut SequenceNumber> {
        let client_id = self.connection_addr_to_id.get(addr)?;
        let connection = self.connections.get_mut(client_id)?;

        if !connection.last_seq_num_recv.contains_key(event) {
            connection
                .last_seq_num_recv
                .insert(event.to_string(), SequenceNumber::new(0));
        }

        let seq = connection.last_seq_num_recv.get_mut(event)?;

        Some(seq)
    }

    fn update_current_send_seq_num_for_event(
        &mut self,
        addr: &SocketAddr,
        event: &str,
    ) -> Option<SequenceNumber> {
        let client_id = self.connection_addr_to_id.get(addr)?;

        let connection = self.connections.get_mut(client_id)?;

        let Some(seq) = connection.current_send_seq_num.get_mut(event) else {
            connection
                .current_send_seq_num
                .insert(event.to_owned(), SequenceNumber::new(0));
            return Some(SequenceNumber::new(0));
        };

        *seq += SequenceNumber::new(1);

        Some(*seq)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum ServerEvent {
    /// Pushed to the server event queue when a client connects
    OnClientConnected(ConnectionId),
    /// Pushed to the server event queue when a client times out
    OnClientTimeout(ConnectionId),
    /// Pushes to the server event queue when a client is disconnected
    OnClientDisconnected(ConnectionId),
}
