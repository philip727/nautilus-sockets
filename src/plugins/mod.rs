pub mod logging;

use crate::socket::{NautSocket, SocketType};

pub trait SocketPlugin<'socket, S: SocketType<'socket>> {
    fn register(&self, socket: &mut NautSocket<'socket, S>);
}
