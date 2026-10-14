//! The flight model (GDD 11): one solver flies every aircraft from its data (GDD 11.1, 12.1).
//!
//! Layering (ADR-29): this crate depends only on `simmath` and `content`. It knows nothing about
//! the tick schedule, the WASM ABI or the browser; `sim-core` adapts its services into
//! [`env::FlightEnv`] and calls the model, and `flight-test` drives the same model headlessly for
//! the Performance Conformance Test (GDD 11.19, ADR-31).
//!
//! Conventions (GDD 0.3.2): SI units; radians internally; every vector is named with its frame
//! (`pos_bf`, `vel_body`). BF is East-North-Up and curved; BODY is forward-right-down.
#![deny(missing_docs)]

pub mod airdata;
pub mod atmosphere;
pub mod energy;
pub mod env;
pub mod physical;
pub mod table;
