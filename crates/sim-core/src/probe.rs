//! Determinism probe world.
//!
//! There is no flight model yet, so the 10^6-tick WASM-vs-native hash test needs a stand-in that
//! exercises everything ADR-10 worries about: `f64` division and `sqrt`, every transcendental
//! (`exp`, `pow`, `sin`, `cos`, `atan2`), integer RNG draws, quaternion integration and
//! renormalisation, and fixed iteration order. Bodies loosely fly over a curved Earth in the
//! battlefield frame (BF): Earth's centre is at `(0, 0, -R)`, so "up" is the local radial.
//! Physical plausibility is not the goal; reproducibility is.

use crate::dmath;
use crate::hash::StateHasher;
use crate::rng::Xoshiro256StarStar;
use crate::time::{SIM_DT, sim_time};

/// Number of probe bodies.
pub const BODY_COUNT: usize = 32;
/// A body is re-seeded from the RNG every this many ticks (round-robin), so state never settles.
pub const RESEED_PERIOD_TICKS: u64 = 600;

const EARTH_RADIUS_M: f64 = 6_371_000.0;
const STD_GRAVITY: f64 = 9.806_65;
const SEA_LEVEL_DENSITY: f64 = 1.225;
const SCALE_HEIGHT_M: f64 = 8_500.0;
const HOLD_ALTITUDE_M: f64 = 8_000.0;
const THRUST_ACCEL: f64 = 3.0;
const TAU: f64 = core::f64::consts::TAU;

type V3 = [f64; 3];

/// A small world of independent bodies.
#[derive(Clone, Debug)]
pub struct ProbeWorld {
    pos_bf: [V3; BODY_COUNT],
    vel_bf: [V3; BODY_COUNT],
    quat: [[f64; 4]; BODY_COUNT],
    omega_body: [V3; BODY_COUNT],
    drag_k: [f64; BODY_COUNT],
    gust: Xoshiro256StarStar,
    reseed: Xoshiro256StarStar,
}

impl ProbeWorld {
    /// Builds the world, drawing initial conditions from `init`, and keeping `gust` and `reseed`
    /// as the per-tick random streams.
    #[must_use]
    pub fn new(
        mut init: Xoshiro256StarStar,
        gust: Xoshiro256StarStar,
        reseed: Xoshiro256StarStar,
    ) -> Self {
        let mut w = Self {
            pos_bf: [[0.0; 3]; BODY_COUNT],
            vel_bf: [[0.0; 3]; BODY_COUNT],
            quat: [[1.0, 0.0, 0.0, 0.0]; BODY_COUNT],
            omega_body: [[0.0; 3]; BODY_COUNT],
            drag_k: [0.0; BODY_COUNT],
            gust,
            reseed,
        };
        for i in 0..BODY_COUNT {
            w.spawn(i, &mut init);
        }
        w
    }

    fn spawn(&mut self, i: usize, rng: &mut Xoshiro256StarStar) {
        let x = rng.range_f64(-50_000.0, 50_000.0);
        let y = rng.range_f64(-50_000.0, 50_000.0);
        let z = rng.range_f64(2_000.0, 12_000.0);
        let heading = rng.range_f64(0.0, TAU);
        let speed = rng.range_f64(150.0, 300.0);
        self.pos_bf[i] = [x, y, z];
        self.vel_bf[i] = [
            speed * dmath::cos(heading),
            speed * dmath::sin(heading),
            0.0,
        ];
        self.quat[i] = [1.0, 0.0, 0.0, 0.0];
        self.omega_body[i] = [0.0; 3];
        self.drag_k[i] = rng.range_f64(1.5e-4, 4.0e-4);
    }

    /// Advances every body by one tick. `tick` is the index of the tick being executed.
    pub fn step(&mut self, tick: u64) {
        if tick > 0 && tick.is_multiple_of(RESEED_PERIOD_TICKS) {
            let i = ((tick / RESEED_PERIOD_TICKS) % BODY_COUNT as u64) as usize;
            let mut rng = self.reseed.clone();
            self.spawn(i, &mut rng);
            self.reseed = rng;
        }
        let t = sim_time(tick);
        for i in 0..BODY_COUNT {
            self.step_body(i, t);
        }
    }

    fn step_body(&mut self, i: usize, t: f64) {
        let [px, py, pz] = self.pos_bf[i];
        let [vx, vy, vz] = self.vel_bf[i];

        // Radial geometry on the curved Earth (centre at (0, 0, -R)).
        let (rx, ry, rz) = (px, py, pz + EARTH_RADIUS_M);
        let rn2 = rx * rx + ry * ry + rz * rz;
        let rn = dmath::sqrt(rn2);
        let altitude = rn - EARTH_RADIUS_M;
        let mu_over_rn3 = STD_GRAVITY * EARTH_RADIUS_M * EARTH_RADIUS_M / (rn2 * rn);
        let vr = (vx * rx + vy * ry + vz * rz) / rn;

        // Atmosphere and drag.
        let rho = SEA_LEVEL_DENSITY * dmath::exp(-altitude / SCALE_HEIGHT_M);
        let speed = dmath::sqrt(vx * vx + vy * vy + vz * vz);
        let drag = self.drag_k[i] * rho * dmath::pow(speed, 0.8);

        // Weaving thrust.
        let phase = i as f64 * 0.37;
        let heading = dmath::atan2(vy, vx) + 0.3 * dmath::sin(0.2 * t + phase);
        let thrust_x = THRUST_ACCEL * dmath::cos(heading);
        let thrust_y = THRUST_ACCEL * dmath::sin(heading);

        // Altitude hold along the local radial, and random gusts.
        let hold = (-0.01 * (altitude - HOLD_ALTITUDE_M) - 0.05 * vr) / rn;
        let gx = self.gust.next_f64() - 0.5;
        let gy = self.gust.next_f64() - 0.5;
        let gz = self.gust.next_f64() - 0.5;

        let ax = -mu_over_rn3 * rx - drag * vx + thrust_x + hold * rx + gx;
        let ay = -mu_over_rn3 * ry - drag * vy + thrust_y + hold * ry + gy;
        let az = -mu_over_rn3 * rz - drag * vz + hold * rz + gz;

        // Semi-implicit Euler: velocity first, then position with the new velocity.
        let nvx = vx + ax * SIM_DT;
        let nvy = vy + ay * SIM_DT;
        let nvz = vz + az * SIM_DT;
        self.vel_bf[i] = [nvx, nvy, nvz];
        self.pos_bf[i] = [px + nvx * SIM_DT, py + nvy * SIM_DT, pz + nvz * SIM_DT];

        // Attitude: damped random angular rate, quaternion integrated and renormalised.
        let damp = 1.0 - 0.5 * SIM_DT;
        let mut w = self.omega_body[i];
        for wk in &mut w {
            *wk = *wk * damp + (self.gust.next_f64() - 0.5) * 0.2;
        }
        self.omega_body[i] = w;
        let [qw, qx, qy, qz] = self.quat[i];
        let h = 0.5 * SIM_DT;
        let dw = -qx * w[0] - qy * w[1] - qz * w[2];
        let dx = qw * w[0] + qy * w[2] - qz * w[1];
        let dy = qw * w[1] + qz * w[0] - qx * w[2];
        let dz = qw * w[2] + qx * w[1] - qy * w[0];
        let (nw, nx, ny, nz) = (qw + h * dw, qx + h * dx, qy + h * dy, qz + h * dz);
        let inv = 1.0 / dmath::sqrt(nw * nw + nx * nx + ny * ny + nz * nz);
        self.quat[i] = [nw * inv, nx * inv, ny * inv, nz * inv];
    }

    /// True if every state value is finite.
    #[must_use]
    pub fn is_finite(&self) -> bool {
        let flat = |vs: &[V3]| vs.iter().flatten().all(|x| x.is_finite());
        flat(&self.pos_bf)
            && flat(&self.vel_bf)
            && flat(&self.omega_body)
            && self.quat.iter().flatten().all(|x| x.is_finite())
            && self.drag_k.iter().all(|x| x.is_finite())
    }

    /// Largest `| |q| - 1 |` over all bodies (GDD 0.3.3: must stay below 1e-12).
    #[must_use]
    pub fn max_quat_norm_error(&self) -> f64 {
        self.quat
            .iter()
            .map(|q| {
                (dmath::sqrt(q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]) - 1.0).abs()
            })
            .fold(0.0, f64::max)
    }

    /// Position of body `i` in BF, metres.
    #[must_use]
    pub fn pos_bf(&self, i: usize) -> V3 {
        self.pos_bf[i]
    }

    /// Attitude of body `i` as `[w, x, y, z]` (unit quaternion).
    #[must_use]
    pub fn quat(&self, i: usize) -> [f64; 4] {
        self.quat[i]
    }

    /// Hash of all body state, in index order.
    #[must_use]
    pub fn hash_bodies(&self) -> u64 {
        let mut h = StateHasher::new();
        for i in 0..BODY_COUNT {
            h.write_f64s(&self.pos_bf[i]);
            h.write_f64s(&self.vel_bf[i]);
            h.write_f64s(&self.quat[i]);
            h.write_f64s(&self.omega_body[i]);
            h.write_f64(self.drag_k[i]);
        }
        h.finish()
    }

    /// Hash of the random streams owned by the probe.
    #[must_use]
    pub fn hash_rng(&self) -> u64 {
        let mut h = StateHasher::new();
        for w in self.gust.state().into_iter().chain(self.reseed.state()) {
            h.write_u64(w);
        }
        h.finish()
    }
}
