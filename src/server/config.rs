use std::time::Duration;

/// The config of how the [server](crate::server::NautServer) should be structured
pub struct ServerConfig {
    /// The max amount of connections the server will process
    pub max_connections: u8,
    /// The max amount of events the event buffer can reach before clearing itself
    pub max_server_events: u8,
    /// How long it takes for the server to free an idling client
    pub idle_connection_time: Duration,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            max_connections: 128,
            max_server_events: 64,
            idle_connection_time: Duration::from_secs(20),
        }
    }
}
