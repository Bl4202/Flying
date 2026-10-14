//! The observer: an input-driven kinematic body that the camera rides in v0.0b.
//!
//! It is the stand-in for the player's aircraft until the flight model arrives in v0.1a. Stick
//! axes command body rates, the throttle lever commands speed, and the body flies where its nose
//! points. It is deliberately not an aircraft: no lift, drag or stall, so there is nothing to
//! tune and nothing to mistake for flight physics.
//!
//! State is `f64` in the battlefield frame (ADR-03); attitude is a unit quaternion body (FRD) to
//! BF, renormalised every step.

use crate::hash::StateHasher;
use crate::input::InputState;
use crate::layout::{axis, button};
use crate::math::{DQuat, DVec3};
use crate::seams::geo::GeoService;
use crate::seams::weather::WeatherProvider;
use crate::world::Counters;

/// Every number that shapes the observer's behaviour, in one place.
#[derive(Clone, Copy, Debug)]
pub struct ObserverTuning {
    /// Pitch rate at full stick, rad/s (positive = nose up).
    pub pitch_rate: f64,
    /// Roll rate at full stick, rad/s (positive = right wing down).
    pub roll_rate: f64,
    /// Yaw rate at full pedal, rad/s (positive = nose right).
    pub yaw_rate: f64,
    /// Speed at idle throttle, m/s.
    pub speed_min: f64,
    /// Added speed at full throttle, m/s.
    pub speed_span: f64,
    /// Time constant of the speed response, seconds.
    pub speed_tau: f64,
    /// The observer is held at least this far above the ground, metres.
    pub min_agl: f64,
    /// Spawn height above the ground, metres.
    pub spawn_agl: f64,
    /// Spawn distance south of the terrain patch centre, metres.
    pub spawn_south: f64,
    /// Half-width of the playable square around the patch centre, metres.
    pub boundary: f64,
}

/// The tuning in force.
pub const TUNING: ObserverTuning = ObserverTuning {
    pitch_rate: 1.0,
    roll_rate: 2.0,
    yaw_rate: 0.5,
    speed_min: 60.0,
    speed_span: 240.0,
    speed_tau: 1.0,
    min_agl: 50.0,
    spawn_agl: 600.0,
    spawn_south: 6_000.0,
    boundary: 15_000.0,
};

/// Body (FRD) to BF rotation for "nose north, wings level, right wing east".
///
/// A half turn about the axis `(1, 1, 0) / sqrt(2)` sends body x to north (+y), body y to east
/// (+x) and body z (down) to BF -z.
const HEADING_NORTH: DQuat = DQuat::new(
    0.0,
    core::f64::consts::FRAC_1_SQRT_2,
    core::f64::consts::FRAC_1_SQRT_2,
    0.0,
);

/// The observer's state.
#[derive(Clone, Debug)]
pub struct Observer {
    pos_bf: DVec3,
    vel_bf: DVec3,
    q_body_to_bf: DQuat,
    speed_mps: f64,
    throttle: f64,
}

impl Observer {
    /// Spawned 6 km south of the patch centre, level, heading north.
    #[must_use]
    pub fn spawn(geo: &dyn GeoService) -> Self {
        let mut o = Self {
            pos_bf: DVec3::ZERO,
            vel_bf: DVec3::ZERO,
            q_body_to_bf: HEADING_NORTH,
            speed_mps: 0.0,
            throttle: f64::from(InputState::DEFAULT_THROTTLE),
        };
        o.reset(geo);
        o
    }

    /// Back to the spawn point and attitude.
    pub fn reset(&mut self, geo: &dyn GeoService) {
        let (x, y) = (0.0, -TUNING.spawn_south);
        self.pos_bf = DVec3::new(x, y, geo.ground_z_bf(x, y) + TUNING.spawn_agl);
        self.q_body_to_bf = HEADING_NORTH;
        self.throttle = f64::from(InputState::DEFAULT_THROTTLE);
        self.speed_mps = TUNING.speed_min + TUNING.speed_span * self.throttle;
        self.vel_bf = self.q_body_to_bf.rotate(DVec3::X) * self.speed_mps;
    }

    /// Stage 5 (dynamics): one explicit step. `t_s` is simulation time at the start of the tick.
    pub fn integrate(
        &mut self,
        input: &InputState,
        weather: &dyn WeatherProvider,
        geo: &dyn GeoService,
        t_s: f64,
        dt: f64,
    ) {
        if input.pressed(button::RESET) {
            self.reset(geo);
        }
        let rates_body = DVec3::new(
            input.axis(axis::ROLL) * TUNING.roll_rate,
            input.axis(axis::PITCH) * TUNING.pitch_rate,
            input.axis(axis::YAW) * TUNING.yaw_rate,
        );
        self.q_body_to_bf = self.q_body_to_bf.integrate_body_rate(rates_body, dt);

        self.throttle = input.throttle_cmd();
        let target = TUNING.speed_min + TUNING.speed_span * self.throttle;
        self.speed_mps += (target - self.speed_mps) * (dt / (TUNING.speed_tau + dt));

        let forward = self.q_body_to_bf.rotate(DVec3::X);
        self.vel_bf = forward * self.speed_mps + weather.wind_bf(self.pos_bf, t_s);
        self.pos_bf += self.vel_bf * dt;
    }

    /// Stage 8 (collision): keep the body inside the playable square and above the ground, and
    /// recover from non-finite state (GDD 61.9.2: restore, count, never propagate).
    pub fn constrain(&mut self, geo: &dyn GeoService, counters: &mut Counters) {
        if !(self.pos_bf.is_finite()
            && self.vel_bf.is_finite()
            && self.q_body_to_bf.is_finite()
            && self.speed_mps.is_finite())
        {
            counters.observer_resets = counters.observer_resets.wrapping_add(1);
            self.reset(geo);
            return;
        }
        let b = TUNING.boundary;
        let (cx, cy) = (self.pos_bf.x.clamp(-b, b), self.pos_bf.y.clamp(-b, b));
        if cx.to_bits() != self.pos_bf.x.to_bits() || cy.to_bits() != self.pos_bf.y.to_bits() {
            self.pos_bf.x = cx;
            self.pos_bf.y = cy;
            counters.boundary_clamps = counters.boundary_clamps.wrapping_add(1);
        }
        let floor = geo.ground_z_bf(self.pos_bf.x, self.pos_bf.y) + TUNING.min_agl;
        if self.pos_bf.z < floor {
            self.pos_bf.z = floor;
            counters.ground_clamps = counters.ground_clamps.wrapping_add(1);
        }
    }

    /// Position in BF, metres.
    #[must_use]
    pub fn pos_bf(&self) -> DVec3 {
        self.pos_bf
    }

    /// Velocity in BF, m/s.
    #[must_use]
    pub fn vel_bf(&self) -> DVec3 {
        self.vel_bf
    }

    /// Attitude, body (FRD) to BF.
    #[must_use]
    pub fn attitude(&self) -> DQuat {
        self.q_body_to_bf
    }

    /// Airspeed along the nose, m/s.
    #[must_use]
    pub fn speed_mps(&self) -> f64 {
        self.speed_mps
    }

    /// Throttle setting in force, 0 to 1.
    #[must_use]
    pub fn throttle(&self) -> f64 {
        self.throttle
    }

    /// Feeds the state into `h`.
    pub fn hash_into(&self, h: &mut StateHasher) {
        h.write_f64s(&self.pos_bf.to_array());
        h.write_f64s(&self.vel_bf.to_array());
        h.write_f64s(&self.q_body_to_bf.to_array());
        h.write_f64s(&[self.speed_mps, self.throttle]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::InputFrame;
    use crate::seams::geo::{FlatGeo, ProceduralGeo};
    use crate::seams::weather::{StillAir, UniformWind};
    use crate::time::SIM_DT;

    fn fly(o: &mut Observer, input: &InputState, geo: &dyn GeoService, ticks: u32) -> Counters {
        let mut c = Counters::default();
        for i in 0..ticks {
            o.integrate(input, &StillAir, geo, f64::from(i) * SIM_DT, SIM_DT);
            o.constrain(geo, &mut c);
        }
        c
    }

    fn input_with(f: impl FnOnce(&mut InputFrame)) -> InputState {
        let mut s = InputState::new();
        let mut fr = InputFrame::neutral(1);
        fr.axes[axis::THROTTLE] = 0.5;
        f(&mut fr);
        s.push(fr);
        s
    }

    #[test]
    fn spawns_level_heading_north_above_the_ground() {
        let geo = ProceduralGeo::default_frame();
        let o = Observer::spawn(&geo);
        let p = o.pos_bf();
        assert_eq!((p.x, p.y), (0.0, -6_000.0));
        assert!((p.z - geo.ground_z_bf(0.0, -6_000.0) - 600.0).abs() < 1e-9);
        let fwd = o.attitude().rotate(DVec3::X);
        let right = o.attitude().rotate(DVec3::Y);
        let down = o.attitude().rotate(DVec3::Z);
        let close = |a: DVec3, b: DVec3| (a - b).length() < 1e-12;
        assert!(close(fwd, DVec3::Y), "nose north: {fwd:?}");
        assert!(close(right, DVec3::X), "right wing east: {right:?}");
        assert!(close(down, -DVec3::Z), "belly down: {down:?}");
    }

    #[test]
    fn flies_straight_at_the_commanded_speed() {
        let geo = FlatGeo::new();
        let mut o = Observer::spawn(&geo);
        let input = input_with(|f| f.axes[axis::THROTTLE] = 0.5);
        let start = o.pos_bf();
        fly(&mut o, &input, &geo, 60);
        let moved = o.pos_bf() - start;
        // 60 + 240 * 0.5 = 180 m/s north for one second.
        assert!(moved.x.abs() < 1e-9 && moved.z.abs() < 1e-9, "{moved:?}");
        assert!((moved.y - 180.0).abs() < 0.5, "moved {moved:?}");
    }

    #[test]
    fn throttle_changes_speed_with_a_one_second_lag() {
        let geo = FlatGeo::new();
        let mut o = Observer::spawn(&geo);
        let full = input_with(|f| f.axes[axis::THROTTLE] = 1.0);
        fly(&mut o, &full, &geo, 60);
        // After one time constant: 63 % of the way from 180 to 300 m/s.
        let expected = 180.0 + 120.0 * (1.0 - crate::dmath::exp(-1.0));
        assert!(
            (o.speed_mps() - expected).abs() < 2.0,
            "speed {}",
            o.speed_mps()
        );
        fly(&mut o, &full, &geo, 600);
        assert!((o.speed_mps() - 300.0).abs() < 0.01);
    }

    #[test]
    fn stick_commands_the_documented_rates() {
        let geo = FlatGeo::new();
        // Roll right at full stick: 2 rad/s about the nose for a quarter second is 0.5 rad.
        let mut o = Observer::spawn(&geo);
        let roll = input_with(|f| f.axes[axis::ROLL] = 1.0);
        fly(&mut o, &roll, &geo, 15);
        let right_after = o.attitude().rotate(DVec3::Y);
        let angle = crate::dmath::acos(right_after.dot(DVec3::X));
        assert!((angle - 0.5).abs() < 0.01, "rolled {angle} rad");
        // Pull back: the nose rises.
        let mut o = Observer::spawn(&geo);
        let pull = input_with(|f| f.axes[axis::PITCH] = 1.0);
        fly(&mut o, &pull, &geo, 30);
        assert!(o.attitude().rotate(DVec3::X).z > 0.3, "nose up");
        // Right pedal: the nose swings from north toward east.
        let mut o = Observer::spawn(&geo);
        let yaw = input_with(|f| f.axes[axis::YAW] = 1.0);
        fly(&mut o, &yaw, &geo, 30);
        assert!(o.attitude().rotate(DVec3::X).x > 0.2, "nose to the right");
    }

    #[test]
    fn the_ground_stops_a_dive() {
        let geo = ProceduralGeo::default_frame();
        let mut o = Observer::spawn(&geo);
        // A held pull is a loop (radius = speed / rate = 180 m), so tip the nose over by about
        // 76 degrees and let go: it then flies straight at the ground.
        let push = input_with(|f| f.axes[axis::PITCH] = -1.0);
        let released = input_with(|_| {});
        let mut c = fly(&mut o, &push, &geo, 80);
        assert_eq!(c.ground_clamps, 0, "still in the air after the push");
        let after = fly(&mut o, &released, &geo, 600);
        c.ground_clamps += after.ground_clamps;
        assert!(c.ground_clamps > 0, "the dive reached the ground");
        let p = o.pos_bf();
        assert!(p.z >= geo.ground_z_bf(p.x, p.y) + TUNING.min_agl - 1e-9);
    }

    #[test]
    fn the_boundary_keeps_it_over_the_patch() {
        let geo = FlatGeo::new();
        let mut o = Observer::spawn(&geo);
        let input = input_with(|f| f.axes[axis::THROTTLE] = 1.0);
        // 120 s at up to 300 m/s from 6 km south is well past the 15 km edge.
        let c = fly(&mut o, &input, &geo, 7_200);
        assert!(c.boundary_clamps > 0);
        assert!(o.pos_bf().y <= TUNING.boundary + 1e-9);
        assert!(o.pos_bf().x.abs() <= TUNING.boundary + 1e-9);
    }

    #[test]
    fn wind_carries_the_body() {
        let geo = FlatGeo::new();
        let mut o = Observer::spawn(&geo);
        let input = input_with(|f| f.axes[axis::THROTTLE] = 0.5);
        let wind = UniformWind {
            wind_bf: DVec3::new(10.0, 0.0, 0.0),
        };
        let start = o.pos_bf();
        for i in 0..60 {
            o.integrate(&input, &wind, &geo, f64::from(i) * SIM_DT, SIM_DT);
        }
        assert!((o.pos_bf().x - start.x - 10.0).abs() < 0.05);
    }

    #[test]
    fn the_reset_button_returns_to_spawn() {
        let geo = FlatGeo::new();
        let mut o = Observer::spawn(&geo);
        let spawn = o.pos_bf();
        let mut input = input_with(|f| f.axes[axis::THROTTLE] = 1.0);
        fly(&mut o, &input, &geo, 120);
        assert!((o.pos_bf() - spawn).length() > 100.0);
        let mut press = InputFrame::neutral(2);
        press.buttons = 1 << button::RESET;
        input.enqueue(press);
        input.ingest(0);
        o.integrate(&input, &StillAir, &geo, 0.0, SIM_DT);
        // One step after reset: within one tick of travel of the spawn point.
        assert!((o.pos_bf() - spawn).length() < 6.0);
    }

    #[test]
    fn non_finite_state_is_recovered_and_counted() {
        let geo = FlatGeo::new();
        let mut o = Observer::spawn(&geo);
        o.pos_bf.x = f64::NAN;
        let mut c = Counters::default();
        o.constrain(&geo, &mut c);
        assert_eq!(c.observer_resets, 1);
        assert!(o.pos_bf().is_finite());
        assert_eq!(o.pos_bf().y, -6_000.0);
    }

    #[test]
    fn the_attitude_stays_normalised_over_a_long_erratic_flight() {
        let geo = ProceduralGeo::default_frame();
        let mut o = Observer::spawn(&geo);
        let mut input = InputState::new();
        input.set_script(Some(crate::input::ErraticPilot::new(
            crate::rng::RngStreams::new(3).stream("input/erratic"),
        )));
        let mut c = Counters::default();
        for tick in 0..20_000_u64 {
            input.ingest(tick);
            o.integrate(&input, &StillAir, &geo, crate::time::sim_time(tick), SIM_DT);
            o.constrain(&geo, &mut c);
            assert!((o.attitude().norm_squared() - 1.0).abs() < 1e-12);
        }
        assert!(o.pos_bf().is_finite());
    }
}
