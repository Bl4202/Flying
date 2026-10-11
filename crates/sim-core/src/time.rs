//! Simulation time. Time is a 64-bit tick counter times `SIM_DT`, never accumulated floats
//! (GDD 0.3.1).

/// Master tick rate in hertz (ADR-07).
pub const SIM_HZ: u32 = 60;

/// Seconds per master tick.
pub const SIM_DT: f64 = 1.0 / 60.0;

/// Simulation time in seconds at the start of `tick`.
#[must_use]
pub fn sim_time(tick: u64) -> f64 {
    tick as f64 * SIM_DT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_second_is_sixty_ticks() {
        assert_eq!(sim_time(0), 0.0);
        assert_eq!(sim_time(u64::from(SIM_HZ)), 1.0);
        assert_eq!(sim_time(600), 10.0);
    }
}
