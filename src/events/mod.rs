use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, RwLock},
};

pub type CallbackArgs<'callback> = (SocketAddr, &'callback [u8]);
pub type Callback<T> = dyn Fn(&Arc<RwLock<T>>, CallbackArgs) + Send + Sync;

pub struct EventEmitter<T> {
    pub event_callbacks: HashMap<String, Vec<Arc<Callback<T>>>>,
    pub value: Arc<RwLock<T>>,
}

impl<T> EventEmitter<T> {
    pub fn new(value: Arc<RwLock<T>>) -> Self {
        Self {
            event_callbacks: HashMap::new(),
            value,
        }
    }

    pub fn register_event<F>(&mut self, event: &str, f: F)
    where
        F: Fn(&Arc<RwLock<T>>, CallbackArgs) + Send + Sync + 'static,
    {
        let event = event.to_string();
        if let Some(callbacks) = self.event_callbacks.get_mut(&event) {
            callbacks.push(Arc::new(f));
            return;
        }

        self.event_callbacks.insert(event, vec![Arc::new(f)]);
    }

    pub fn emit_event(&self, event: &str, args: CallbackArgs) {
        let Some(callbacks) = self.event_callbacks.get(event) else {
            return;
        };

        for callback in callbacks {
            callback(&self.value, args)
        }
    }
}
