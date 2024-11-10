pub mod acknowledgement;
pub mod client;
pub mod connection;
pub mod events;
pub mod packet;
pub mod sequence;
pub mod server;
pub mod socket;

#[cfg(test)]
mod tests {
    use std::{
        sync::{Arc, RwLock},
        time::Duration,
    };

    use crate::{
        client::NautClient, packet::PacketDelivery, server::NautServer, socket::NautSocket,
    };

    #[test]
    fn test_udp_socket() {
        let server = Arc::new(RwLock::new(
            NautSocket::<NautServer>::new("127.0.0.1:8008").unwrap(),
        ));

        std::thread::sleep(Duration::from_millis(100));

        let mut client = NautSocket::<NautClient>::new("127.0.0.1:0").unwrap();
        client.connect_to("127.0.0.1:8008").unwrap();
        let _ = client.send("ack_client", &[], PacketDelivery::Reliable);

        let server_clone = Arc::clone(&server);
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_millis(1));

            let mut server = server_clone.write().unwrap();
            let _ = server.poll();
            let _ = server.broadcast("meow", &[0, 53, 52, 89], PacketDelivery::ReliableSequenced);
            server.run_events();
        });

        std::thread::sleep(Duration::from_millis(10));

        client.on("meow", |_client, (_addr, packet)| {
            println!("{packet:?}");
        });

        loop {
            let _ = client.poll();
            client.run_events();
        }
    }
}
