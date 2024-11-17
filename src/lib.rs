mod acknowledgement;
pub mod client;
mod connection;
mod events;
pub mod packet;
mod sequence;
pub mod server;
pub mod socket;
pub mod plugins;
pub mod persistent;

/// Gives you access to everything you need to create an event listening socket
pub mod prelude {
    pub use crate::server::*;
    pub use crate::client::*;
    pub use crate::socket::*;
    pub use crate::packet::*;
    pub use crate::server::config::*;
    pub use crate::plugins::*;
    pub use crate::plugins::logging::*;
    pub use crate::persistent::*;
}
