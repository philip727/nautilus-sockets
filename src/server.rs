use std::{
    collections::{HashMap, VecDeque},
    net::{SocketAddr, ToSocketAddrs, UdpSocket},
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};

use anyhow::anyhow;

use crate::{
    events::EventEmitter,
    packet::PacketDelivery,
    socket::{NautSocket, SocketType},
};

// Incremental Id
pub type ConnectionId = u16;
pub struct NautServer {
    pub max_connections: u8,
    pub connection_addr_to_id: HashMap<SocketAddr, ConnectionId>,
    pub connection_id_to_addr: HashMap<ConnectionId, SocketAddr>,

    time_outs: HashMap<ConnectionId, Instant>,

    pub next_id: ConnectionId,
    pub freed_ids: VecDeque<ConnectionId>,

    pub idle_connection_timeout: Duration,
}

impl NautServer {
    pub fn disconnect_and_free_clients(&mut self) {
        let time_outs = self.time_outs.clone();
        for (id, time) in time_outs {
            if Instant::now().duration_since(time) < self.idle_connection_timeout {
                continue;
            }

            self.freed_ids.push_back(id);
            let Some(addr) = self.connection_id_to_addr.remove(&id) else {
                println!("Failed to find address of idle'd client with id: {id}");
                continue;
            };

            self.connection_addr_to_id.remove(&addr);
            println!("Timed out client with id: {id}");
            self.time_outs.remove(&id);
        }
    }
}

impl Default for NautServer {
    fn default() -> Self {
        Self {
            max_connections: 128,
            connection_addr_to_id: Default::default(),
            connection_id_to_addr: Default::default(),
            time_outs: Default::default(),
            next_id: 0,
            freed_ids: VecDeque::new(),
            idle_connection_timeout: Duration::from_secs(20),
        }
    }
}

impl NautSocket<NautServer> {
    pub fn new<A>(addr: A) -> anyhow::Result<Self>
    where
        A: ToSocketAddrs,
    {
        let socket = UdpSocket::bind(addr)?;
        socket.set_nonblocking(true)?;

        let server = Arc::new(RwLock::new(NautServer::default()));
        let event_emitter = EventEmitter::new(Arc::clone(&server));
        Ok(Self {
            socket,
            packet_queue: VecDeque::new(),
            inner: server,
            event_emitter,
        })
    }

    pub fn run_events(&mut self) {
        {
            // Disconnects the idle'd clients each event loop
            if let Ok(mut inner) = self.inner.write() {
                inner.disconnect_and_free_clients();
            }
        }

        while let Some((addr, packet)) = self.get_last_received_packet() {
            let Ok(event) = Self::get_event_from_packet(&packet) else {
                continue;
            };

            let Ok(mut server) = self.inner.write() else {
                continue;
            };
            // Establishes a connection with a client if not already established
            #[allow(clippy::map_entry)]
            if !server.connection_addr_to_id.contains_key(&addr) {
                // Gets a new client id
                let client_id = {
                    if let Some(client_id) = server.freed_ids.pop_front() {
                        client_id
                    } else {
                        let client_id = server.next_id;
                        server.next_id += 1;
                        client_id
                    }
                };

                server.connection_addr_to_id.insert(addr, client_id);
                server.connection_id_to_addr.insert(client_id, addr);
            }

            let Some(client) = server.connection_addr_to_id.get(&addr) else {
                continue;
            };
            let client = *client;
            server.time_outs.insert(client, Instant::now());

            let bytes = Self::get_packet_bytes(&packet);
            self.event_emitter.emit_event(&event, (addr, &bytes));
        }
    }

    pub fn broadcast(
        &self,
        event: &str,
        buf: &[u8],
        delivery: PacketDelivery,
    ) -> anyhow::Result<()> {
        let Ok(server) = self.inner.read() else {
            return Err(anyhow!(
                "Failed to get RwLock read guard in broadcast with event {event}"
            ));
        };

        for id in server.connection_id_to_addr.keys() {
            let _ = self.send(event, buf, delivery, *id);
        }

        todo!()
    }

    pub fn send(
        &self,
        event: &str,
        buf: &[u8],
        delivery: PacketDelivery,
        client: ConnectionId,
    ) -> anyhow::Result<()> {
        let Ok(server) = self.inner.read() else {
            return Err(anyhow!("Failed to get inner rwlock guard"));
        };

        let addr = server.connection_id_to_addr.get(&client).ok_or(anyhow!(
            "There is no associated address with this client id"
        ))?;

        self.send_by_addr(event, buf, delivery, addr)
    }

}

impl SocketType for NautServer {}
