use geometry::{Sockets, Solid};
use patina_sdk::node;

/// Moves the solid to the socket with the name `socket`. The origin of the solid goes to the
/// socket, and the axes of the solid turn with the socket. Write sockets with no wire as
/// "name x y z", or "name x y z" and 3 angles. Put a ";" between the sockets.
#[node]
pub fn attach(solid: &Solid, sockets: &Sockets, socket: &str) -> Result<Solid, String> {
    let frame = sockets.get(socket).ok_or_else(|| format!("no socket {socket:?} in {sockets:?}"))?;
    Ok(solid.place(frame))
}
