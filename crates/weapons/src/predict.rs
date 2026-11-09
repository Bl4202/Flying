//! The live fly-out predictor behind `IN RNG` (ADR-46; the DLZ baker of GDD 15.5.5 is v0.3a).
//!
//! It flies a copy of the next missile as it would launch now, at 60 Hz (one RK4 sub-step a tick),
//! with a perfect lock and no seeker noise, against the target held at its current velocity and
//! attitude, for the missile's whole flight time. It answers whether the fuze would trigger
//! inside the warhead's lethal radius.

use flight::atmosphere::Atmosphere;
use flight::env::FlightEnv;

use crate::capsule::Capsule;
use crate::fuze::{fuze_armed, fuze_check};
use crate::missile::{Missile, MissileParams, guide, missile_expiry, step_missile_n};
use crate::seeker::{SeekerMode, TargetView};

/// Would `launch` (a missile as it leaves the rail now, at time `t_s`) fuze inside its lethal
/// radius on a target that holds its velocity? False below the minimum launch range. Ticks of
/// `dt` seconds. Allocates nothing.
#[must_use]
#[allow(clippy::too_many_arguments)] // ALGO: the missile, the target and the world
pub fn fly_out<E: FlightEnv>(
    params: &MissileParams,
    launch: Missile,
    target: &TargetView,
    caps: &[Capsule],
    env: &E,
    atmo: &Atmosphere,
    t_s: f64,
    dt: f64,
) -> bool {
    let range = (target.pos_bf - launch.pos).length();
    if !(range >= params.launch.min_range_m && dt > 0.0) {
        return false;
    }
    let mut m = launch;
    m.seeker.mode = SeekerMode::Locked;
    let mut tgt = *target;
    let ticks = (params.max_time_s / dt).ceil() as u32;
    for k in 0..ticks {
        let t = t_s + f64::from(k) * dt;
        let a_cmd = guide(&m, params, &tgt, [0.0, 0.0], env, atmo, t);
        let (m_prev, t_prev) = (m.pos, tgt.pos_bf);
        step_missile_n(&mut m, params, a_cmd, env, atmo, t, dt, 1);
        tgt.pos_bf += tgt.vel_bf * dt;
        if fuze_armed(&m, params)
            && let Some(d) = fuze_check(m_prev, m.pos, t_prev, tgt.pos_bf, tgt.att, caps, params)
        {
            return d.miss_m < params.lethal_radius_m;
        }
        if missile_expiry(&m, params, env, true).is_some() {
            return false;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::missile::launch;
    use crate::seeker::SeekerState;
    use crate::test_content;
    use flight::env::FlatSea;
    use simmath::math::{DQuat, DVec3};

    const DT: f64 = 1.0 / 60.0;

    fn dart1() -> MissileParams {
        MissileParams::from_def(test_content::missile(), DT).expect("a missile")
    }

    fn caps() -> Vec<Capsule> {
        let d = test_content::kestrel()
            .damage
            .as_ref()
            .expect("damage data");
        d.capsules.iter().map(Capsule::from_def).collect()
    }

    /// Level, nose north.
    fn north() -> DQuat {
        DQuat::from_axis_angle(DVec3::Z, core::f64::consts::FRAC_PI_2)
            * DQuat::from_axis_angle(DVec3::X, core::f64::consts::PI)
    }

    /// A tail chase at 4 000 m: launcher and target northbound at 230 m/s, `range` apart.
    fn chase(range: f64) -> (Missile, TargetView) {
        let p = dart1();
        let vel = DVec3::new(0.0, 230.0, 0.0);
        let m = launch(
            &p,
            DVec3::new(0.0, 0.0, 4_000.0),
            vel,
            SeekerState::searching(),
        );
        let t = TargetView {
            pos_bf: DVec3::new(0.0, range, 4_000.0),
            vel_bf: vel,
            att: north(),
            intensity: 0.5,
        };
        (m, t)
    }

    #[test]
    fn in_range_worked_examples() {
        let p = dart1();
        let c = caps();
        // A co-speed tail chase: 2 km is in range; 300 m is inside the minimum range; 15 km is
        // beyond what the missile can catch in its 25 s.
        for (range, expect) in [(2_000.0, true), (300.0, false), (15_000.0, false)] {
            let (m, t) = chase(range);
            assert_eq!(
                fly_out(&p, m, &t, &c, &FlatSea, &Atmosphere::new(), 0.0, DT),
                expect,
                "{range} m"
            );
        }
        // Head on at 6 km and closing at 460 m/s: in range.
        let (m, mut t) = chase(6_000.0);
        t.vel_bf = DVec3::new(0.0, -230.0, 0.0);
        t.att = DQuat::from_axis_angle(DVec3::Z, core::f64::consts::PI) * north();
        assert!(fly_out(
            &p,
            m,
            &t,
            &c,
            &FlatSea,
            &Atmosphere::new(),
            0.0,
            DT
        ));
    }
}
