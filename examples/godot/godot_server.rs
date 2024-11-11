use std::{
    sync::{mpsc, Arc},
    thread::sleep,
    time::Duration,
};

use nautilus_sockets::prelude::*;

fn main() {
    let mut socket =
        NautSocket::<NautServer>::new("127.0.0.1:8008", ServerConfig::default()).unwrap();
    let (sender, reciever) = mpsc::channel();
    let sender = Arc::new(sender);

    let sender_clone = Arc::clone(&sender);
    socket.on("join", move |server, (addr, _packet)| {
        let client = server.get_client_id(&addr);

        let _ = sender_clone.send(*client.unwrap());
    });

    loop {
        sleep(Duration::from_millis(1));
        socket.poll();
        socket.run_events();

        while let Ok(client) = reciever.try_recv() {
            println!("sending hello");
            let _ = socket.send("hello", &[], PacketDelivery::Reliable, client);
        }
    }
}
