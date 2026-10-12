//! Read-only services the systems consult (GDD 61.4.2). Each is a seam with a replaceable
//! implementation (GDD 61.5); the defaults are the simple, real first implementations.

use crate::seams::geo::{GeoService, ProceduralGeo};
use crate::seams::weather::{StillAir, WeatherProvider};

/// The services available to systems during a tick.
pub struct Services {
    /// Terrain, geodesy, line of sight.
    pub geo: Box<dyn GeoService>,
    /// Wind and atmosphere.
    pub weather: Box<dyn WeatherProvider>,
}

impl Services {
    /// The first implementations: procedural spherical Earth, still air.
    #[must_use]
    pub fn defaults() -> Self {
        Self {
            geo: Box::new(ProceduralGeo::default_frame()),
            weather: Box::new(StillAir),
        }
    }
}
