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

pub mod actuators;
pub mod aero;
pub mod aircraft;
pub mod airdata;
pub mod atmosphere;
pub mod body;
pub mod controls;
pub mod energy;
pub mod engine;
pub mod env;
pub mod error;
pub mod frames;
pub mod linearise;
pub mod mass;
pub mod physical;
pub mod table;
pub mod trim;

pub use error::ModelError;

#[cfg(test)]
pub(crate) mod test_content {
    //! The core content pack, baked once for the unit tests.

    use std::sync::OnceLock;

    use content::{AeroDef, AircraftDef, EngineDef, Registry};

    /// The Kestrel's three entries.
    pub(crate) struct Kestrel {
        pub(crate) aircraft: AircraftDef,
        pub(crate) engine: EngineDef,
        pub(crate) aero: AeroDef,
    }

    /// The baked `content/core` pack.
    pub(crate) fn registry() -> &'static Registry {
        static PACK: OnceLock<Registry> = OnceLock::new();
        PACK.get_or_init(|| {
            let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/core");
            let baked = content_bake::bake::bake_dir(&dir)
                .unwrap_or_else(|d| panic!("the core pack bakes: {d:?}"));
            Registry::from_cpack(&baked.cpack).expect("the baked pack reads back")
        })
    }

    /// The first aircraft of the core pack and what it refers to.
    pub(crate) fn kestrel() -> Kestrel {
        let r = registry();
        let h = r.aircraft_handles().next().expect("an aircraft");
        let aircraft = r.aircraft(h).expect("aircraft").clone();
        let engine = r
            .engine(aircraft.engines[0].engine)
            .expect("engine")
            .clone();
        let aero = r.aero(aircraft.aero.tables).expect("aero").clone();
        Kestrel {
            aircraft,
            engine,
            aero,
        }
    }
}
