pub mod logging;

use crate::socket::{NautSocket, SocketType};

/// Allows you to isolate specific functionality into different plugins
pub trait SocketPlugin<'socket, S: SocketType<'socket>> {
    fn register(&self, socket: &mut NautSocket<'socket, S>);
}
