//! The tester's two additions for the v0.1d bot playtester (ADR-47): the direct load-factor sticks
//! (`sticks::air_n`) and the join onto the downwind leg (`Tester::join_downwind`), after which the
//! tester lands as it does at the end of its own circuit.

mod common;

use common::{DT, Rig, kestrel};
use fcs::tester::geom::Runway;
use fcs::tester::sticks::{LoadDemand, air_n, stick_for_n};
use fcs::tester::{Levers, Phase, Presses, Tester};
use fcs::{Assist, PilotInput, standard};
use flight::controls::PilotControls;
use flight::env::FlightEnv;
use flight::frames::euler;
use simmath::dmath;
use simmath::math::{DQuat, DVec3};

const MASS_KG: f64 = 10_260.0;

#[test]
fn air_n_asks_standard_for_the_load_factor_and_full_for_the_matching_path_rate() {
    let (model, fcs) = kestrel();
    // Standard: the stick is the one that asks for n, whatever the bank.
    let rig = Rig::new(&model, &fcs, Assist::Standard, 0.6, 3_000.0, MASS_KG).expect("trims");
    let sense = model.sense(&rig.ac, &rig.world, 0.0);
    let s = rig.sensed();
    let (lo, hi) = standard::n_limits(&fcs, &s);
    for n in [0.5, 1.0, 3.0, 6.5] {
        let st = air_n(
            Assist::Standard,
            &model,
            &fcs,
            &rig.ac,
            &sense,
            &s,
            &LoadDemand { n, bank: 1.0 },
        );
        assert_eq!(st.pitch, stick_for_n(n, lo, hi));
        assert!(
            (standard::n_for_stick(st.pitch, lo, hi) - n).abs() < 1e-12,
            "{n}"
        );
        assert!(
            st.roll > 0.0,
            "a bank to the right is asked for with right stick"
        );
    }
    // Full: a level turn's demand (n = 1 / cos φ at the bank it has) asks for no path rate.
    let mut rig = Rig::new(&model, &fcs, Assist::Full, 0.6, 3_000.0, MASS_KG).expect("trims");
    let roll = DQuat::from_axis_angle(DVec3::X, core::f64::consts::FRAC_PI_3);
    rig.ac.body.att = (rig.ac.body.att * roll).normalized();
    let sense = model.sense(&rig.ac, &rig.world, 0.0);
    let s = rig.sensed();
    assert!(
        (s.phi - core::f64::consts::FRAC_PI_3).abs() < 1e-6,
        "{}",
        s.phi
    );
    let level = 1.0 / dmath::cos(s.phi) * dmath::cos(s.gamma);
    let st = air_n(
        Assist::Full,
        &model,
        &fcs,
        &rig.ac,
        &sense,
        &s,
        &LoadDemand {
            n: level,
            bank: s.phi,
        },
    );
    assert!(st.pitch.abs() < 1e-9, "{}", st.pitch);
    // More load than that climbs; less descends.
    let up = air_n(
        Assist::Full,
        &model,
        &fcs,
        &rig.ac,
        &sense,
        &s,
        &LoadDemand {
            n: level + 0.5,
            bank: s.phi,
        },
    );
    let down = air_n(
        Assist::Full,
        &model,
        &fcs,
        &rig.ac,
        &sense,
        &s,
        &LoadDemand {
            n: level - 0.5,
            bank: s.phi,
        },
    );
    assert!(
        up.pitch > 0.0 && down.pitch < 0.0,
        "{} {}",
        up.pitch,
        down.pitch
    );
}

/// The levers the tester's presses work, and the controls they give (as `sim-core::pilot`).
fn work(levers: &mut Levers, p: Presses, wow: bool) {
    if p.gear {
        if !levers.gear_down {
            levers.gear_down = true;
        } else if !wow {
            levers.gear_down = false;
        }
    }
    if p.flaps_down {
        levers.flap_detent = (levers.flap_detent + 1).min(2);
    }
    if p.flaps_up {
        levers.flap_detent = levers.flap_detent.saturating_sub(1);
    }
    if p.parking_brake {
        levers.parking_brake = !levers.parking_brake;
    }
}

fn controls(c: &PilotControls, levers: Levers) -> PilotControls {
    let park = if levers.parking_brake { 1.0 } else { 0.0 };
    PilotControls {
        gear: if levers.gear_down { 1.0 } else { 0.0 },
        flaps: [0.0, 0.5, 1.0][usize::from(levers.flap_detent)],
        brake_l: c.brake_l.max(park),
        brake_r: c.brake_r.max(park),
        ..*c
    }
}

#[test]
fn after_joining_the_downwind_leg_the_tester_lands_under_every_assist() {
    let (model, fcs) = kestrel();
    let runway = Runway {
        threshold_bf: DVec3::ZERO,
        heading_rad: 0.0,
        length_m: 3_000.0,
        width_m: 45.0,
    };
    for assist in [
        Assist::Full,
        Assist::Standard,
        Assist::Reduced,
        Assist::Manual,
    ] {
        // Clean and level at 300 m and 130 m/s, on the downwind line (3.8 km left of the runway,
        // 3 km along it), heading south.
        let mut rig = Rig::trimmed(
            flight::env::SphereSea::default(),
            &model,
            &fcs,
            assist,
            130.0,
            300.0,
            MASS_KG,
            flight::actuators::Surfaces::NEUTRAL,
        )
        .expect("trims");
        rig.use_lever = true;
        let up = rig.world.up_bf(rig.ac.body.pos_bf);
        let heading = euler(up, rig.ac.body.att).heading_rad;
        let turn = DQuat::from_axis_angle(DVec3::Z, heading - core::f64::consts::PI);
        rig.ac.body.att = (turn * rig.ac.body.att).normalized();
        rig.ac.body.vel_bf = turn.rotate(rig.ac.body.vel_bf);
        rig.ac.body.pos_bf = DVec3::new(-3_800.0, 3_000.0, 300.0);
        let mut tester = Tester::new(runway);
        let mut levers = Levers::default();
        tester.join_downwind(0.0);
        assert_eq!(tester.phase(), Phase::Downwind);
        assert_eq!(tester.lift_off_s(), Some(0.0));
        assert_eq!(tester.runway(), runway);
        let mut seen_final = false;
        for _ in 0..(400.0 / DT) as u32 {
            let out = tester.step(&model, &fcs, &rig.ac, &rig.fs, levers, &rig.world, rig.t());
            work(&mut levers, out.presses, rig.ac.ground.wow);
            let input = PilotInput {
                controls: controls(&out.controls, levers),
                ..PilotInput::default()
            };
            rig.step(&input);
            seen_final |= tester.phase() == Phase::Final;
            if tester.phase() == Phase::Stopped || rig.destroyed.is_some() {
                break;
            }
        }
        let at = runway.locate(rig.ac.body.pos_bf);
        assert_eq!(rig.destroyed, None, "{assist:?}");
        assert!(seen_final, "{assist:?}: flew the final");
        assert_eq!(
            tester.phase(),
            Phase::Stopped,
            "{assist:?}: stopped at {at:?} after {} s",
            rig.t()
        );
        assert!(
            at.across.abs() <= 22.5 && (0.0..=3_000.0).contains(&at.along),
            "{assist:?}: {at:?}"
        );
        assert!(levers.gear_down, "{assist:?}: landed with the gear down");
    }
}
