use std::{
    any::TypeId,
    collections::HashMap,
    sync::{Arc, RwLock},
};

use super::Persistent;

pub(crate) struct PersistentStorage {
    pub data: HashMap<TypeId, Arc<RwLock<dyn Persistent>>>,
}

impl PersistentStorage {
    pub(crate) fn new() -> PersistentStorage {
        PersistentStorage {
            data: HashMap::new(),
        }
    }

    pub(crate) fn get_persistent<P>(&self) -> Option<Arc<RwLock<P>>>
    where
        P: Persistent + 'static,
    {
        let type_id = TypeId::of::<P>();
        let persistent = Arc::clone(self.data.get(&type_id)?);
        let raw: *const RwLock<dyn Persistent> = Arc::into_raw(persistent);
        let raw: *const RwLock<P> = raw.cast();

        unsafe {
            let persistent = Arc::from_raw(raw);
            Some(persistent)
        }
    }

    pub(crate) fn insert_persistent<P>(&mut self, persistent: P)
    where
        P: Persistent,
    {
        let type_id = TypeId::of::<P>();
        self.data.insert(type_id, Arc::new(RwLock::new(persistent)));
    }

    pub(crate) fn init_persistent<P>(&mut self)
    where
        P: Persistent + Default,
    {
        let type_id = TypeId::of::<P>();
        self.data
            .insert(type_id, Arc::new(RwLock::new(P::default())));
    }
}
