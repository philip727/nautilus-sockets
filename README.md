# Nautilus Sockets
A simple event based UDP socket listener

## What is a nautilus socket (event based socket listener)?
The socket waits on specific events to be recieved and will emit an event to call all callbacks
registered to that event. For example, if we were to recieve a <b>"send_position"</b> event, everything
callback registered to that event would be called.

## Examples

```rust
// Creates a socket using the NautServer implementation
let mut socket = NautSocket::<NautServer>::new("127.0.0.1:8080", ServerConfig::default())?;
// A hashmap containing all the positions
let positions: Arc<Mutex<HashMap<ConnectionId, Vector3>>> = Arc::new(Mutex::new(HashMap::new()));

let position_clone = Arc::clone(&positions);
// Everytime we recieve a packet with this event, it will perform this callback
socket.on("recieve_position", move |server, (addr, packet)| {
    // Gets a lockguard for the positons
    let Ok(mut positions) = position_clone.lock() else {
        return;
    };

    // Gets the client id based on the address that sent the packet
    let client = server.get_client_id(&addr);
    /// Assuming we have some form of deserialization method
    let vec3 = Vector3::from_bytes(&packet);
    positions.insert(client, vec3);
});

```
