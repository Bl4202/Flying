//! The rear-aspect infrared seeker (GDD 15.7, `AC-15.6`, ADR-46).
//!
//! Signal `s = (I / I_ref) · (R_ref / R)²`, so the lock range against intensity `I` is
//! `R_ref √(I / I_ref)`. Searching, the seeker is caged at its axis (the aircraft's nose on the
//! rail, the missile's velocity in flight) with a narrow field of view; it acquires (TONE) when the
//! target is inside the field of view, within the rear-aspect cut-off, above the acquisition
//! signal and in terrain line of sight, and LOCKS after a run of consecutive TONE ticks. Locked, it
//! tracks within its gimbal limit and keeps the lock with some hysteresis on the signal and aspect.

use content::WeaponDef;
use simmath::dmath;
use simmath::math::{DQuat, DVec3};

use crate::ir::aspect_angle_rad;

/// Signal needed to acquire. // ALGO: ADR-46, the reference intensity at the reference range
pub const ACQUIRE_SIGNAL: f64 = 1.0;
/// Signal below which a lock is lost. // ALGO: ADR-46, hysteresis against flicker at the edge
pub const KEEP_SIGNAL: f64 = 0.8;
/// How far past the acquisition aspect a lock is kept, rad. // ALGO: ADR-46, 5° of hysteresis
pub const KEEP_ASPECT_MARGIN_RAD: f64 = 0.087_266_462_599_716_48;
/// Shortest range used in the signal, m (the signal is capped there). // ALGO: no 1/0
const MIN_SIGNAL_RANGE_M: f64 = 1.0;

/// A seeker's parameters, in SI units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SeekerParams {
    /// Half the caged field of view, rad.
    pub half_fov_rad: f64,
    /// Gimbal limit when locked, rad.
    pub gimbal_rad: f64,
    /// Range at which a target of `ref_intensity` gives signal 1, m.
    pub ref_range_m: f64,
    /// The intensity `ref_range_m` is quoted at.
    pub ref_intensity: f64,
    /// Largest aspect angle (from dead astern) at which the seeker acquires, rad.
    pub max_aspect_rad: f64,
    /// Largest aspect angle at which it keeps a lock, rad.
    pub keep_aspect_rad: f64,
    /// Standard deviation of the line-of-sight noise, rad.
    pub noise_rad: f64,
    /// Consecutive TONE ticks before it locks.
    pub lock_ticks: u32,
}

impl SeekerParams {
    /// The seeker of a missile entry, for a tick of `dt_s` seconds; `None` for a gun.
    #[must_use]
    pub fn from_def(def: &WeaponDef, dt_s: f64) -> Option<Self> {
        let s = &def.missile.as_ref()?.seeker;
        let max_aspect_rad = f64::from(s.max_aspect_deg).to_radians();
        Some(Self {
            half_fov_rad: 0.5 * f64::from(s.fov_deg).to_radians(),
            gimbal_rad: f64::from(s.gimbal_deg).to_radians(),
            ref_range_m: f64::from(s.lock_ref_range_m),
            ref_intensity: f64::from(s.ref_intensity),
            max_aspect_rad,
            keep_aspect_rad: max_aspect_rad + KEEP_ASPECT_MARGIN_RAD,
            noise_rad: f64::from(s.noise_deg).to_radians(),
            lock_ticks: ((f64::from(s.lock_time_s) / dt_s).round() as u32).max(1),
        })
    }
}

/// What the seeker is doing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum SeekerMode {
    /// Not powered (no missile on the rails, or weapons safe).
    #[default]
    Off = 0,
    /// Caged at its axis, looking.
    Search = 1,
    /// Sees a target (the growl); not yet locked.
    Tone = 2,
    /// Tracking a target.
    Locked = 3,
}

impl SeekerMode {
    /// The mode's code (the snapshot's `seeker` field).
    #[must_use]
    pub const fn code(self) -> u8 {
        self as u8
    }
}

/// A seeker's state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SeekerState {
    /// The mode.
    pub mode: SeekerMode,
    /// Consecutive ticks it has seen the target while not locked.
    pub tone_ticks: u32,
    /// Terrain line of sight to the target at the last check.
    pub los_clear: bool,
}

impl SeekerState {
    /// A powered seeker, searching.
    #[must_use]
    pub const fn searching() -> Self {
        Self {
            mode: SeekerMode::Search,
            tone_ticks: 0,
            los_clear: false,
        }
    }
}

/// The target as a seeker (or a missile) sees it, BF.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TargetView {
    /// Reference point, m.
    pub pos_bf: DVec3,
    /// Velocity, m/s.
    pub vel_bf: DVec3,
    /// Attitude, body to BF.
    pub att: DQuat,
    /// Infrared intensity towards the seeker ([`crate::ir::intensity`]).
    pub intensity: f64,
}

impl TargetView {
    /// The target's nose direction, BF.
    #[must_use]
    pub fn nose_bf(&self) -> DVec3 {
        self.att.rotate(DVec3::X)
    }
}

/// The seeker's signal from a target of `intensity` at `range_m`: `(I / I_ref) (R_ref / R)²`.
#[must_use]
pub fn seeker_signal(params: &SeekerParams, range_m: f64, intensity: f64) -> f64 {
    let r = range_m.max(MIN_SIGNAL_RANGE_M);
    let k = params.ref_range_m / r;
    intensity / params.ref_intensity * k * k
}

/// The angle between two directions, rad; 0 when either has zero length.
fn angle_between(a: DVec3, b: DVec3) -> f64 {
    let la = a.length();
    let lb = b.length();
    if !(la > 0.0 && lb > 0.0) {
        return 0.0;
    }
    dmath::acos((a.dot(b) / (la * lb)).clamp(-1.0, 1.0))
}

/// Advances the seeker by one tick and returns its signal (0 when off or not finite).
///
/// `axis_bf` is the direction it is caged at or measures its gimbal from (the launcher's nose on
/// the rail, the missile's velocity in flight); `los_clear` the cached terrain line of sight.
/// SEARCH and TONE: it sees the target when the line of sight is clear, the target is within
/// `half_fov_rad` of the axis, its aspect is at most `max_aspect_rad` and the signal at least
/// [`ACQUIRE_SIGNAL`]; each tick it sees, `tone_ticks` grows, and at `lock_ticks` it LOCKS; a tick
/// it does not see returns it to SEARCH. LOCKED: it keeps the lock while the line of sight is
/// clear, the target is within `gimbal_rad` of the axis, its aspect is at most
/// `keep_aspect_rad` and the signal at least [`KEEP_SIGNAL`]; otherwise it returns to SEARCH.
pub fn seeker_step(
    state: &mut SeekerState,
    params: &SeekerParams,
    axis_bf: DVec3,
    seeker_pos_bf: DVec3,
    target: &TargetView,
    los_clear: bool,
) -> f64 {
    state.los_clear = los_clear;
    if state.mode == SeekerMode::Off {
        state.tone_ticks = 0;
        return 0.0;
    }
    let to_target = target.pos_bf - seeker_pos_bf;
    let signal = seeker_signal(params, to_target.length(), target.intensity);
    let off_axis = angle_between(axis_bf, to_target);
    let aa = aspect_angle_rad(target.nose_bf(), target.pos_bf, seeker_pos_bf);
    let finite = to_target.is_finite()
        && axis_bf.is_finite()
        && target.att.is_finite()
        && signal.is_finite()
        && off_axis.is_finite()
        && aa.is_finite();
    if state.mode == SeekerMode::Locked {
        let keeps = finite
            && los_clear
            && off_axis <= params.gimbal_rad
            && aa <= params.keep_aspect_rad
            && signal >= KEEP_SIGNAL;
        if !keeps {
            *state = SeekerState {
                los_clear,
                ..SeekerState::searching()
            };
        }
    } else {
        let sees = finite
            && los_clear
            && off_axis <= params.half_fov_rad
            && aa <= params.max_aspect_rad
            && signal >= ACQUIRE_SIGNAL;
        if sees {
            state.tone_ticks = state.tone_ticks.saturating_add(1);
            state.mode = if state.tone_ticks >= params.lock_ticks {
                SeekerMode::Locked
            } else {
                SeekerMode::Tone
            };
        } else {
            state.mode = SeekerMode::Search;
            state.tone_ticks = 0;
        }
    }
    if finite { signal } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_content;

    const DT: f64 = 1.0 / 60.0;

    fn dart1() -> SeekerParams {
        SeekerParams::from_def(test_content::missile(), DT).expect("a seeker")
    }

    /// A target 4 000 m north at 5 000 m, flying north (seen from dead astern by a shooter at the
    /// origin pointing north), intensity 1.
    fn north_target(range: f64) -> TargetView {
        TargetView {
            pos_bf: DVec3::new(0.0, range, 5_000.0),
            vel_bf: DVec3::new(0.0, 200.0, 0.0),
            att: DQuat::from_axis_angle(DVec3::Z, core::f64::consts::FRAC_PI_2)
                * DQuat::from_axis_angle(DVec3::X, core::f64::consts::PI),
            intensity: 1.0,
        }
    }

    const SHOOTER: DVec3 = DVec3::new(0.0, 0.0, 5_000.0);
    const NORTH: DVec3 = DVec3::Y;

    #[test]
    fn the_dart1_seeker_from_its_entry() {
        let p = dart1();
        assert!((p.half_fov_rad - 2f64.to_radians()).abs() < 1e-12);
        assert!((p.gimbal_rad - 30f64.to_radians()).abs() < 1e-12);
        assert_eq!(
            (p.ref_range_m, p.ref_intensity, p.lock_ticks),
            (6_000.0, 1.0, 30)
        );
        assert!((p.max_aspect_rad - 35f64.to_radians()).abs() < 1e-12);
        assert!((p.keep_aspect_rad - 40f64.to_radians()).abs() < 1e-12);
        assert!((p.noise_rad - f64::from(0.1f32).to_radians()).abs() < 1e-12);
        assert!(SeekerParams::from_def(test_content::gun(), DT).is_none());
        // The target view's nose points north.
        assert!((north_target(1.0).nose_bf() - NORTH).length() < 1e-12);
    }

    /// Does a fresh seeker get TONE on the first tick against `t`?
    fn tone(p: &SeekerParams, t: &TargetView) -> bool {
        let mut s = SeekerState::searching();
        seeker_step(&mut s, p, NORTH, SHOOTER, t, true);
        s.mode == SeekerMode::Tone
    }

    #[test]
    fn ms2_the_lock_range_scales_with_the_root_of_the_intensity() {
        let p = dart1();
        for i in [0.1, 0.5, 1.0, 2.0] {
            // The longest range with tone, found by bisection.
            let (mut lo, mut hi) = (10.0, 50_000.0);
            for _ in 0..60 {
                let mid = f64::midpoint(lo, hi);
                let t = TargetView {
                    intensity: i,
                    ..north_target(mid)
                };
                if tone(&p, &t) { lo = mid } else { hi = mid }
            }
            let expected = 6_000.0 * dmath::sqrt(i);
            assert!(
                ((lo - expected) / expected).abs() < 0.01,
                "I {i}: {lo} m vs {expected} m"
            );
        }
        // Worked example: intensity 0.5 at 3 000 m gives signal 0.5 · 4 = 2.
        assert!((seeker_signal(&p, 3_000.0, 0.5) - 2.0).abs() < 1e-12);
    }

    #[test]
    fn ms2_no_tone_beyond_the_rear_aspect_cutoff() {
        let p = dart1();
        // The target 2 km ahead, turned so the seeker sees it from 34° and 36° off its tail (the
        // target stays on the shooter's boresight; only its heading changes).
        for (deg, expect) in [(34.0, true), (36.0, false), (90.0, false), (180.0, false)] {
            let mut t = north_target(2_000.0);
            let yaw = DQuat::from_axis_angle(DVec3::Z, f64::to_radians(deg));
            t.att = yaw * t.att;
            assert_eq!(tone(&p, &t), expect, "{deg}°");
        }
    }

    #[test]
    fn ms2_tone_comes_first_then_the_lock_after_half_a_second() {
        let p = dart1();
        let t = north_target(3_000.0);
        let mut s = SeekerState::searching();
        let mut locked_at = None;
        for tick in 1..=60 {
            seeker_step(&mut s, &p, NORTH, SHOOTER, &t, true);
            if tick < 30 {
                assert_eq!(s.mode, SeekerMode::Tone, "tick {tick}");
            }
            if s.mode == SeekerMode::Locked && locked_at.is_none() {
                locked_at = Some(tick);
            }
        }
        let locked_at = f64::from(locked_at.expect("a lock"));
        assert!((locked_at * DT - 0.5).abs() <= DT + 1e-12, "{locked_at}");
        // A tick without the target (line of sight blocked) resets the run.
        let mut s = SeekerState::searching();
        for _ in 0..20 {
            seeker_step(&mut s, &p, NORTH, SHOOTER, &t, true);
        }
        seeker_step(&mut s, &p, NORTH, SHOOTER, &t, false);
        assert_eq!(
            (s.mode, s.tone_ticks, s.los_clear),
            (SeekerMode::Search, 0, false)
        );
    }

    #[test]
    fn ms2_the_lock_is_lost_outside_the_gimbal() {
        let p = dart1();
        let t = north_target(3_000.0);
        let lock = || SeekerState {
            mode: SeekerMode::Locked,
            tone_ticks: 30,
            los_clear: true,
        };
        // The target 29° and 31° off the seeker's axis (the axis swings; the target holds still).
        for (deg, keeps) in [(29.0, true), (31.0, false)] {
            let a = f64::to_radians(deg);
            let axis = DVec3::new(dmath::sin(a), dmath::cos(a), 0.0);
            let mut s = lock();
            seeker_step(&mut s, &p, axis, SHOOTER, &t, true);
            assert_eq!(s.mode == SeekerMode::Locked, keeps, "{deg}°");
        }
        // Caged at 3° off: no tone, but a lock 3° off is kept.
        let axis = DVec3::new(dmath::sin(0.052), dmath::cos(0.052), 0.0);
        let mut s = SeekerState::searching();
        seeker_step(&mut s, &p, axis, SHOOTER, &t, true);
        assert_eq!(s.mode, SeekerMode::Search);
        let mut s = lock();
        seeker_step(&mut s, &p, axis, SHOOTER, &t, true);
        assert_eq!(s.mode, SeekerMode::Locked);
        // The lock holds down to signal 0.8 and to 40° of aspect, then goes.
        let mut s = lock();
        seeker_step(
            &mut s,
            &p,
            NORTH,
            SHOOTER,
            &north_target(6_000.0 / 0.85f64.sqrt()),
            true,
        );
        assert_eq!(s.mode, SeekerMode::Locked);
        seeker_step(
            &mut s,
            &p,
            NORTH,
            SHOOTER,
            &north_target(6_000.0 / 0.75f64.sqrt()),
            true,
        );
        assert_eq!(s.mode, SeekerMode::Search);
        for (deg, keeps) in [(39.0, true), (41.0, false)] {
            let mut t = north_target(2_000.0);
            t.att = DQuat::from_axis_angle(DVec3::Z, f64::to_radians(deg)) * t.att;
            let mut s = lock();
            seeker_step(&mut s, &p, NORTH, SHOOTER, &t, true);
            assert_eq!(s.mode == SeekerMode::Locked, keeps, "{deg}°");
        }
    }

    #[test]
    fn off_and_non_finite_inputs() {
        let p = dart1();
        let mut s = SeekerState {
            mode: SeekerMode::Off,
            tone_ticks: 7,
            los_clear: false,
        };
        assert_eq!(
            seeker_step(&mut s, &p, NORTH, SHOOTER, &north_target(1_000.0), true),
            0.0
        );
        assert_eq!(
            (s.mode, s.tone_ticks, s.los_clear),
            (SeekerMode::Off, 0, true)
        );
        let mut s = SeekerState::searching();
        let bad = TargetView {
            pos_bf: DVec3::new(f64::NAN, 0.0, 0.0),
            ..north_target(1_000.0)
        };
        assert_eq!(seeker_step(&mut s, &p, NORTH, SHOOTER, &bad, true), 0.0);
        assert_eq!(s.mode, SeekerMode::Search);
        let mut s = SeekerState {
            mode: SeekerMode::Locked,
            tone_ticks: 30,
            los_clear: true,
        };
        seeker_step(&mut s, &p, NORTH, SHOOTER, &bad, true);
        assert_eq!(s.mode, SeekerMode::Search);
        assert_eq!(
            [
                SeekerMode::Off,
                SeekerMode::Search,
                SeekerMode::Tone,
                SeekerMode::Locked
            ]
            .map(SeekerMode::code),
            [0, 1, 2, 3]
        );
    }
}
