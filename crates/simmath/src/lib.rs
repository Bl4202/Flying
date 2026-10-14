//! Deterministic `f64` maths shared by the simulation crates (GDD 0.3.3, ADR-10, ADR-29).
//!
//! * [`math`]: `DVec3`, `DQuat` and `DMat3`, built only from `+ - * /`, `sqrt` and [`dmath`].
//! * [`dmath`]: every transcendental function, through the pure-Rust software `libm`.
//!
//! The crate has no dependency other than `libm` (default features off), so `sim-core`, `flight`
//! and the tools all compute bit-identical results on every target.
#![deny(missing_docs)]

pub mod dmath;
pub mod math;
