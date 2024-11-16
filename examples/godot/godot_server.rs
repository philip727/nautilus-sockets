use std::{thread::sleep, time::Duration};

use nautilus_sockets::prelude::*;

fn main() {
    let mut socket = NautSocket::<NautServer>::new("127.0.0.1:8008", ServerConfig::default())
        .unwrap()
        .register_plugin(LoggingPlugin);

    socket.on("join", move |socket, (addr, _packet)| {
        let client = socket.server().get_client_id(&addr);
        let _ = socket.send("hello", &[], PacketDelivery::Reliable, *client.unwrap());
    });

    loop {
        sleep(Duration::from_millis(1));
        socket.poll();
        socket.run_events();
    }
}
