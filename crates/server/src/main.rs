//! Dedicated server (stub until v0.5).
//!
//! `server --selftest` sends a hello through the in-process transport and checks that it comes
//! out the other side, which proves the binary links the real transport seam.

use protocol::PROTOCOL_VERSION;
use protocol::transport::{Channel, LoopbackTransport, Transport, TransportEvent};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None => println!(
            "server {} (stub) protocol {}",
            env!("CARGO_PKG_VERSION"),
            PROTOCOL_VERSION
        ),
        Some("--selftest") => match selftest() {
            Ok(report) => println!("{report}"),
            Err(why) => {
                eprintln!("server selftest failed: {why}");
                std::process::exit(1);
            }
        },
        Some(other) => {
            eprintln!("usage: server [--selftest]\nunknown argument `{other}`");
            std::process::exit(2);
        }
    }
}

/// The stand-in handshake message: protocol version, then a greeting.
fn hello() -> Vec<u8> {
    let mut bytes = PROTOCOL_VERSION.to_le_bytes().to_vec();
    bytes.extend_from_slice(b"hello");
    bytes
}

/// A client sends a hello over a loopback link and the server receives it, byte for byte.
fn selftest() -> Result<String, String> {
    let (mut client, mut server) = LoopbackTransport::pair();
    let to_server = client.peer();
    client.send_reliable(to_server, &hello());

    let mut connected = false;
    while let Some(event) = server.poll() {
        match event {
            TransportEvent::Connected(_) => connected = true,
            TransportEvent::Message {
                channel: Channel::Reliable,
                bytes,
                ..
            } if connected => {
                return if bytes == hello() {
                    Ok(format!(
                        "server selftest ok: hello received ({} bytes, protocol {PROTOCOL_VERSION})",
                        bytes.len()
                    ))
                } else {
                    Err(format!("the hello arrived altered: {bytes:?}"))
                };
            }
            other => return Err(format!("unexpected event {other:?}")),
        }
    }
    Err("the hello never arrived".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hello_goes_through_the_loopback() {
        let report = selftest().unwrap();
        assert!(report.contains("hello received (9 bytes"), "{report}");
    }
}
