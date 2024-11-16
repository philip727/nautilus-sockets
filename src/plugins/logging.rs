use std::sync::Arc;

use crate::server::NautServer;

use super::SocketPlugin;

pub struct LoggingPlugin;

impl SocketPlugin<'_, NautServer> for LoggingPlugin {
    fn register(&self, socket: &mut crate::prelude::NautSocket<'_, NautServer>) {
        socket.on_poll(move |server| {
            while let Some(event) = server.inner.iter_server_events().next() {
                println!("[LOG] {event:?}");
            }
        });

        socket.on_poll(move |server| {
            for event in server.socket_events.iter() {

            }
        });
    }
}
