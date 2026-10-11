//! WebAssembly ABI for the simulation core (GDD 61.4.4).
//!
//! A hand-written C ABI, with no `wasm-bindgen` on the hot path. Pointers and 64-bit integers are
//! avoided where possible: 64-bit values cross the boundary as two `u32` halves so JavaScript never
//! needs `BigInt`. The one `unsafe` module is [`abi`]; its invariants are in `UNSAFE.md`.

pub mod abi;
