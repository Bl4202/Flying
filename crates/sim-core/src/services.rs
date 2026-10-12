//! Read-only services the systems consult (GDD 61.4.2). Each is a seam with a replaceable
//! implementation (GDD 61.5); the defaults are the simple, real first implementations.

use content::Registry;

use crate::seams::geo::{GeoService, ProceduralGeo};
use crate::seams::los::{LosService, OpenSky};
use crate::seams::strategic::{NoStrategicContext, StrategicContext};
use crate::seams::weather::{StillAir, WeatherProvider};

/// The services available to systems during a tick.
pub struct Services {
    /// Terrain and geodesy.
    pub geo: Box<dyn GeoService>,
    /// Line of sight between points.
    pub los: Box<dyn LosService>,
    /// Wind and atmosphere.
    pub weather: Box<dyn WeatherProvider>,
    /// The campaign the mission belongs to.
    pub strategic: Box<dyn StrategicContext>,
    /// Baked content, immutable once the core starts.
    pub registry: Registry,
}

impl Services {
    /// The first implementations: procedural spherical Earth, open sky, still air, no campaign, and
    /// no content until a pack is loaded.
    #[must_use]
    pub fn defaults() -> Self {
        Self {
            geo: Box::new(ProceduralGeo::default_frame()),
            los: Box::new(OpenSky),
            weather: Box::new(StillAir),
            strategic: Box::new(NoStrategicContext),
            registry: Registry::empty(),
        }
    }
}
