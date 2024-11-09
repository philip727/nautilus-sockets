pub mod client;
pub mod events;
pub mod packet;
pub mod server;
pub mod socket;

#[cfg(test)]
mod tests {
    use std::{
        sync::{Arc, RwLock},
        time::Duration,
    };

    use crate::{client::NautClient, server::NautServer, socket::NautSocket};

    #[test]
    fn test_udp_socket() {
        let server = Arc::new(RwLock::new(
            NautSocket::<NautServer>::new("127.0.0.1:8008").unwrap(),
        ));

        std::thread::sleep(Duration::from_millis(100));

        let socket = NautSocket::<NautClient>::new("127.0.0.1:0").unwrap();
        socket.connect_to("127.0.0.1:8008").unwrap();

        let server_clone = Arc::clone(&server);
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_millis(1));

            let mut server = server_clone.write().unwrap();
            let _ = server.poll();
            server.run_events();
        });

        loop {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
