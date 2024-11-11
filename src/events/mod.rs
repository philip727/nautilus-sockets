use std::{collections::HashMap, net::SocketAddr, sync::Arc};

/// The arguments that are passed into a callback, the sending address and the packet itself
pub(crate) type CallbackArgs<'callback> = (SocketAddr, &'callback [u8]);
/// The structure of a callback, it passes in a reference to the socket type and the arguments
pub(crate) type Callback<T> = dyn Fn(&T, CallbackArgs) + Send + Sync;

/// Listens to and emits events, running callbacks on events that have been emitted
pub(crate) struct EventEmitter<T> {
    pub event_callbacks: HashMap<String, Vec<Arc<Callback<T>>>>,
}

impl<T> EventEmitter<T> {
    /// Creates a new event emitter
    pub(crate) fn new() -> Self {
        Self {
            event_callbacks: HashMap::new(),
        }
    }

    /// Registers a callback to be run when an event is emitted
    pub(crate) fn register_event<F>(&mut self, event: &str, f: F)
    where
        F: Fn(&T, CallbackArgs) + Send + Sync + 'static,
    {
        let event = event.to_string();
        if let Some(callbacks) = self.event_callbacks.get_mut(&event) {
            callbacks.push(Arc::new(f));
            return;
        }

        self.event_callbacks.insert(event, vec![Arc::new(f)]);
    }

    /// Emits an event and fires all callbacks registered for that event
    pub(crate) fn emit_event(&self, event: &str, value: &T, args: CallbackArgs) {
        let Some(callbacks) = self.event_callbacks.get(event) else {
            return;
        };

        for callback in callbacks {
            callback(value, args)
        }
    }
}

impl<T> Default for EventEmitter<T> {
    fn default() -> Self {
        Self::new()
    }
}
