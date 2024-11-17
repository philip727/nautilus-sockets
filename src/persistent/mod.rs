use std::any::Any;

pub mod storage;

pub trait Persistent: Any + Send + Sync {}
