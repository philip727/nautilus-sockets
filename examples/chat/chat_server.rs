use std::{collections::HashMap, net::SocketAddr};

use nautilus_sockets::prelude::*;

fn main() {
    let mut socket =
        NautSocket::<NautServer>::new("127.0.0.1:8008", ServerConfig::default()).unwrap();
    socket.register_plugin(LoggingPlugin);
    socket.register_plugin(ChattersPlugin);

    socket.on("hi", move |_, _| {});

    loop {
        // Must be run for every socket
        socket.poll();
        socket.run_events();
    }
}

#[derive(Default)]
pub struct Chatters {
    names: HashMap<ConnectionId, String>,
}

impl Persistent for Chatters {}

pub struct ChattersPlugin;

impl SocketPlugin<'_, NautServer> for ChattersPlugin {
    fn register(&self, socket: &mut NautSocket<'_, NautServer>) {
        socket.init_persistent::<Chatters>();

        socket.on("new_messenger", create_new_chatter);
        socket.on("send_message", on_send_message);

        socket.on_poll(remove_chatters_on_disconnect);
    }
}

fn remove_chatters_on_disconnect(socket: &mut NautSocket<'_, NautServer>) {
    for event in socket.server().iter_server_events() {
        match event {
            ServerEvent::OnClientTimeout(id) | ServerEvent::OnClientDisconnected(id) => {
                let Some(chatters) = socket.get_persistent::<Chatters>() else {
                    return;
                };

                let Ok(mut chatters) = chatters.write() else {
                    return;
                };

                chatters.names.remove(id);
            }
            _ => {}
        }
    }
}

fn on_send_message(socket: &mut NautSocket<'_, NautServer>, (addr, packet): (SocketAddr, &[u8])) {
    let Some(id) = socket.server().get_client_id(&addr) else {
        return;
    };

    let name = {
        let Some(chatters) = socket.get_persistent::<Chatters>() else {
            return;
        };
        let Ok(chatters) = chatters.read() else {
            return;
        };

        chatters.names.get(id).cloned()
    };

    let Some(name) = name else {
        return;
    };

    let msg = String::from_utf8(packet.to_vec()).unwrap();
    let string = format!("{}: {}", name, msg);
    let _ = socket.broadcast("recv_message", string.as_bytes(), PacketDelivery::Reliable);
}

fn create_new_chatter(
    socket: &mut NautSocket<'_, NautServer>,
    (addr, packet): (SocketAddr, &[u8]),
) {
    let Some(id) = socket.server().get_client_id(&addr) else {
        return;
    };

    let Some(chatters) = socket.get_persistent::<Chatters>() else {
        return;
    };

    let name = String::from_utf8(packet.to_vec()).unwrap();
    {
        let Ok(mut chatters) = chatters.write() else {
            return;
        };

        chatters.names.insert(*id, name.clone());
    }

    let join_msg = format!("Welcome {name}");

    let _ = socket.broadcast(
        "recv_message",
        join_msg.as_bytes(),
        PacketDelivery::Reliable,
    );
}
