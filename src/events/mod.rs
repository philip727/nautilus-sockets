use std::{collections::HashMap, net::SocketAddr, sync::Arc};

pub type CallbackArgs<'callback> = (SocketAddr, &'callback [u8]);
pub type Callback<T> = dyn Fn(&T, CallbackArgs) + Send + Sync;

pub struct EventEmitter<T> {
    pub event_callbacks: HashMap<String, Vec<Arc<Callback<T>>>>,
}

impl<T> EventEmitter<T> {
    pub fn new() -> Self {
        Self {
            event_callbacks: HashMap::new(),
        }
    }

    pub fn register_event<F>(&mut self, event: &str, f: F)
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

    pub fn emit_event(&self, event: &str, value: &T, args: CallbackArgs) {
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
