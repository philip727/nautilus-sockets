use std::{
    collections::HashMap,
    sync::{mpsc, Arc, RwLock},
    time::Duration,
};

use nautilus_sockets::prelude::*;

fn main() {
    let mut socket = NautSocket::<NautServer>::new("127.0.0.1:8008").unwrap();

    let names: Arc<RwLock<HashMap<ConnectionId, String>>> = Arc::new(RwLock::new(HashMap::new()));
    let (sender, receiver) = mpsc::channel();
    let sender = Arc::new(sender);

    let names_clone = Arc::clone(&names);
    let sender_clone = Arc::clone(&sender);
    socket.on("new_messenger", move |server, (addr, packet)| {
        let Some(id) = server.connection_addr_to_id.get(&addr) else {
            return;
        };

        let Ok(mut names) = names_clone.write() else {
            return;
        };

        let name = String::from_utf8(packet.to_vec()).unwrap();

        names.insert(*id, name.clone());

        let join_msg = format!("Welcome {name}");
        let _ = sender_clone.send(join_msg);
    });

    socket.on("send_message", move |server, (addr, packet)| {
        let Some(id) = server.connection_addr_to_id.get(&addr) else {
            return;
        };

        let Ok(names) = names.read() else {
            return;
        };

        let Some(name) = names.get(id) else {
            return;
        };

        let msg = String::from_utf8(packet.to_vec()).unwrap();
        let string = format!("{}: {}", name, msg);
        println!("{string}");
        let _ = sender.send(string);
    });

    loop {
        std::thread::sleep(Duration::from_millis(1));
        // Must be run for every socket
        socket.poll();
        socket.run_events();

        while let Ok(msg) = receiver.try_recv() {
            let _ = socket.broadcast("recv_message", msg.as_bytes(), PacketDelivery::Reliable);
        }
    }
}
