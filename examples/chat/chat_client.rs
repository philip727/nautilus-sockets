use std::{
    io::stdin,
    sync::{Arc, RwLock},
    time::Duration,
};

use nautilus_sockets::{client::NautClient, packet::PacketDelivery, socket::NautSocket};

fn main() {
    let client = Arc::new(RwLock::new(
        NautSocket::<NautClient>::new("127.0.0.1:0").unwrap(),
    ));

    let mut name: Option<String> = None;
    {
        let Ok(mut client) = client.write() else {
            return;
        };

        client.connect_to("127.0.0.1:8008").unwrap();

        client.on("recv_message", |_inner, (_addr, bytes)| {
            let msg = String::from_utf8(bytes.to_vec()).unwrap();
            println!("{}", msg);
        });
    }

    let client_clone = Arc::clone(&client);
    // Separate thread as input blocks
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(1));

        let Ok(mut client) = client_clone.write() else {
            continue;
        };

        // Must be run for every socket
        client.poll();
        client.run_events();
    });

    loop {
        if name.as_ref().is_none() {
            let mut s = String::new();
            println!("Enter your name: ");
            stdin().read_line(&mut s).expect("Invalid string");

            if let Some('\n') = s.chars().next_back() {
                s.pop();
            }

            if let Some('\r') = s.chars().next_back() {
                s.pop();
            }

            name = Some(s.clone());

            // Get write lock after getting input, shortest time for it in scope
            let Ok(mut client) = client.write() else {
                continue;
            };
            let _ = client.send("new_messenger", s.as_bytes(), PacketDelivery::Reliable);
            continue;
        }

        let mut s = String::new();
        stdin().read_line(&mut s).expect("Invalid string");

        if let Some('\n') = s.chars().next_back() {
            s.pop();
        }

        if let Some('\r') = s.chars().next_back() {
            s.pop();
        }

        // Get write lock after getting input, shortest time for it in scope
        let Ok(mut client) = client.write() else {
            continue;
        };
        let _ = client.send("send_message", s.as_bytes(), PacketDelivery::Reliable);
    }
}
