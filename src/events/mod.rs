use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use crate::socket::{NautSocket, SocketType};

/// The arguments that are passed into a callback, the sending address and the packet itself
pub(crate) type EventCallbackArgs<'callback> = (SocketAddr, &'callback [u8]);
/// The structure of a callback, it passes in a reference to the socket type and the arguments
pub(crate) type EventCallback<T> = dyn Fn(&T, EventCallbackArgs) + Send + Sync;
/// The arguments that are passed into a polled callback
pub(crate) type PolledCallback<T> = dyn Fn(&T) + Send + Sync;

/// Listens to and emits events, running callbacks on events that have been emitted
pub(crate) struct EventEmitter<'socket, T> where T: SocketType<'socket> {
    pub event_callbacks: HashMap<String, Vec<Arc<EventCallback<T>>>>,
    pub polled_callbacks: Vec<Arc<PolledCallback<NautSocket<'socket, T>>>>,
}

impl<'socket, T> EventEmitter<'socket, T> where T: SocketType<'socket> {
    /// Creates a new event emitter
    pub(crate) fn new() -> Self {
        Self {
            event_callbacks: HashMap::new(),
            polled_callbacks: Vec::new(),
        }
    }

    /// Registers a callback to be run when an event is emitted
    pub(crate) fn register_event<F>(&mut self, event: &str, f: F)
    where
        F: Fn(&T, EventCallbackArgs) + Send + Sync + 'static,
    {
        let event = event.to_string();
        if let Some(callbacks) = self.event_callbacks.get_mut(&event) {
            callbacks.push(Arc::new(f));
            return;
        }

        self.event_callbacks.insert(event, vec![Arc::new(f)]);
    }

    /// Emits an event and fires all callbacks registered for that event
    pub(crate) fn emit_event(&self, event: &str, value: &T, args: EventCallbackArgs) {
        let Some(callbacks) = self.event_callbacks.get(event) else {
            return;
        };

        for callback in callbacks {
            callback(value, args)
        }
    }

    /// Registers a callback that is run everytime the socket is polled
    pub(crate) fn register_poll_event<F>(&mut self, f: F)
    where
        F: Fn(&NautSocket<T>) + Send + Sync + 'static,
    {
        self.polled_callbacks.push(Arc::new(f));
    }

    /// Runs all polled events
    pub(crate) fn emit_polled_events(&self, socket: &NautSocket<'socket, T>)
    where
        T: SocketType<'socket>,
    {
        for callback in self.polled_callbacks.iter() {
            callback(socket)
        }
    }
}

impl<'socket, T> Default for EventEmitter<'socket, T> where T: SocketType<'socket> {
    fn default() -> Self {
        Self::new()
    }
}
