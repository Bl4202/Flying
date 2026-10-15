//! Read-only services the systems consult (GDD 61.4.2). Each is a seam with a replaceable
//! implementation (GDD 61.5); the defaults are the simple, real first implementations.

use content::Registry;
use flight::aircraft::AircraftModel;
use flight::atmosphere::NonStandardDay;
use flight::env::FlightEnv;

use crate::math::DVec3;
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
    /// The player's aircraft type, built from the registry when content is loaded: the aircraft
    /// entry with the lowest handle (no ids in code, `AC-12.4`). `None` without aircraft content,
    /// and then the player is a spectator.
    pub aircraft: Option<AircraftModel>,
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
            aircraft: None,
        }
    }

    /// The world as the flight model sees it.
    #[must_use]
    pub fn flight_env(&self) -> ServicesEnv<'_> {
        ServicesEnv {
            geo: &*self.geo,
            weather: &*self.weather,
        }
    }
}

/// The geo and weather seams as the flight model's [`FlightEnv`] (ADR-29).
#[derive(Clone, Copy)]
pub struct ServicesEnv<'a> {
    geo: &'a dyn GeoService,
    weather: &'a dyn WeatherProvider,
}

impl<'a> ServicesEnv<'a> {
    /// An environment over the given seams.
    #[must_use]
    pub fn new(geo: &'a dyn GeoService, weather: &'a dyn WeatherProvider) -> Self {
        Self { geo, weather }
    }
}

impl FlightEnv for ServicesEnv<'_> {
    fn height_msl_m(&self, pos_bf: DVec3) -> f64 {
        self.geo.height_msl_m(pos_bf)
    }

    fn up_bf(&self, pos_bf: DVec3) -> DVec3 {
        self.geo.up_bf(pos_bf)
    }

    fn earth_radius_m(&self) -> f64 {
        self.geo.frame().radius_m
    }

    fn gravity_bf(&self, pos_bf: DVec3) -> DVec3 {
        self.geo.gravity_bf(pos_bf)
    }

    fn ground_height_m(&self, pos_bf: DVec3) -> f64 {
        self.geo.ground_height_msl_m(pos_bf)
    }

    fn wind_bf(&self, pos_bf: DVec3, t_s: f64) -> DVec3 {
        self.weather.wind_bf(pos_bf, t_s)
    }

    fn day(&self) -> NonStandardDay {
        self.weather.day()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::seams::geo::FlatGeo;
    use crate::seams::weather::UniformWind;

    #[test]
    fn the_environment_is_the_seams() {
        let geo = ProceduralGeo::default_frame();
        let wind = UniformWind {
            wind_bf: DVec3::new(3.0, -1.0, 0.0),
        };
        let env = ServicesEnv::new(&geo, &wind);
        let p = DVec3::new(40_000.0, -25_000.0, 3_000.0);
        assert_eq!(env.height_msl_m(p), geo.height_msl_m(p));
        assert_eq!(env.up_bf(p), geo.up_bf(p));
        assert_eq!(env.gravity_bf(p), geo.gravity_bf(p));
        assert_eq!(env.ground_height_m(p), geo.ground_height_msl_m(p));
        assert_eq!(env.wind_bf(p, 1.0), wind.wind_bf);
        assert_eq!(env.day(), NonStandardDay::ISA);
        assert_eq!(env.earth_radius_m(), 6_371_000.0);
        let flat = FlatGeo::new();
        let env = ServicesEnv::new(&flat, &StillAir);
        assert_eq!(env.height_msl_m(DVec3::new(5.0, 6.0, 700.0)), 700.0);
        assert_eq!(env.ground_height_m(DVec3::new(5.0, 6.0, 700.0)), 0.0);
    }
}
