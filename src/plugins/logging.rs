use crate::{client::NautClient, server::NautServer};

use super::SocketPlugin;

pub struct LoggingPlugin;

impl SocketPlugin<'_, NautServer> for LoggingPlugin {
    fn register(&self, socket: &mut crate::prelude::NautSocket<'_, NautServer>) {
        socket.on_poll(move |socket| {
            for event in socket.server().iter_server_events() {
                println!("[LOG][SERVER EVENT] {event:?}");
            }
        });

        socket.on_poll(move |socket| {
            for event in socket.socket_events.iter() {
                println!("[LOG][SOCKET EVENT] {event:?}");
            }
        });
    }
}

impl SocketPlugin<'_, NautClient> for LoggingPlugin {
    fn register(&self, socket: &mut crate::prelude::NautSocket<'_, NautClient>) {
        socket.on_poll(move |server| {
            for event in server.socket_events.iter() {
                println!("[LOG][SOCKET EVENT] {event:?}");
            }
        });
    }
}
