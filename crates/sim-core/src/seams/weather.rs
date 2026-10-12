//! `WeatherProvider`: atmosphere the aircraft fly in (GDD 61.5). First implementation: still air
//! or one uniform wind (M1); dynamic fronts and turbulence replace it in M4.

use crate::math::DVec3;

/// Wind and (later) atmosphere queries.
pub trait WeatherProvider: Send {
    /// Wind velocity at a BF position and simulation time, metres per second, in BF axes.
    fn wind_bf(&self, pos_bf: DVec3, t_s: f64) -> DVec3;
}

/// No wind anywhere.
#[derive(Clone, Copy, Debug, Default)]
pub struct StillAir;

impl WeatherProvider for StillAir {
    fn wind_bf(&self, _pos_bf: DVec3, _t_s: f64) -> DVec3 {
        DVec3::ZERO
    }
}

/// The same wind everywhere, all the time.
#[derive(Clone, Copy, Debug)]
pub struct UniformWind {
    /// Wind velocity in BF axes, metres per second.
    pub wind_bf: DVec3,
}

impl WeatherProvider for UniformWind {
    fn wind_bf(&self, _pos_bf: DVec3, _t_s: f64) -> DVec3 {
        self.wind_bf
    }
}

/// Behaviour every `WeatherProvider` must have (`AC-61.5`).
///
/// # Panics
/// Panics, with a message naming the property, if the implementation violates the contract.
pub fn contract(w: &dyn WeatherProvider) {
    for (p, t) in [
        (DVec3::ZERO, 0.0),
        (DVec3::new(10_000.0, -20_000.0, 3_000.0), 12.5),
        (DVec3::new(-1.0e6, 1.0e6, 30_000.0), 86_400.0),
    ] {
        let a = w.wind_bf(p, t);
        assert!(a.is_finite(), "wind is finite at {p:?}, t={t}");
        assert_eq!(
            a,
            w.wind_bf(p, t),
            "wind is a pure function of position and time"
        );
        assert!(
            a.length() < 200.0,
            "wind is below any plausible jet-stream speed: {a:?}"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn still_air_is_still() {
        assert_eq!(
            StillAir.wind_bf(DVec3::new(1.0, 2.0, 3.0), 4.0),
            DVec3::ZERO
        );
    }

    #[test]
    fn uniform_wind_is_uniform() {
        let w = UniformWind {
            wind_bf: DVec3::new(5.0, -2.0, 0.0),
        };
        assert_eq!(
            w.wind_bf(DVec3::ZERO, 0.0),
            w.wind_bf(DVec3::new(9e4, 1e3, 5e3), 99.0)
        );
    }

    #[test]
    fn implementations_pass_the_contract() {
        contract(&StillAir);
        contract(&UniformWind {
            wind_bf: DVec3::new(12.0, 0.0, -1.0),
        });
    }
}
