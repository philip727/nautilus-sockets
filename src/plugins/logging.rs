use std::io::{self, Write};

use crate::{client::NautClient, server::NautServer};

use super::SocketPlugin;

/// A Default plugin that provides logging of events and server events if a naut server
pub struct LoggingPlugin;

impl SocketPlugin<'_, NautServer> for LoggingPlugin {
    fn register(&self, socket: &mut crate::prelude::NautSocket<'_, NautServer>) {
        socket.on_poll(move |socket| {
            let stdout = io::stdout();
            let mut handle = stdout.lock();
            for event in socket.server().iter_server_events() {
                let _ = writeln!(handle, "[LOG][SERVER EVENT] {event:?}");
            }
        });

        socket.on_poll(move |socket| {
            let stdout = io::stdout();
            let mut handle = stdout.lock();
            for event in socket.socket_events.iter() {
                let _ = writeln!(handle, "[LOG][SOCKET EVENT] {event:?}");
            }
        });
    }
}

impl SocketPlugin<'_, NautClient> for LoggingPlugin {
    fn register(&self, socket: &mut crate::prelude::NautSocket<'_, NautClient>) {
        socket.on_poll(move |server| {
            let stdout = io::stdout();
            let mut handle = stdout.lock();
            for event in server.socket_events.iter() {
                let _ = writeln!(handle, "[LOG][SOCKET EVENT] {event:?}");
            }
        });
    }
}
