//! Dedicated server (stub until v0.5).

fn main() {
    println!(
        "server {} (stub) protocol {}",
        env!("CARGO_PKG_VERSION"),
        protocol::PROTOCOL_VERSION
    );
}
