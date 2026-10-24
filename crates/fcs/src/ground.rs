//! Ground mode (GDD 11.13, ADR-34): weight on the wheels switches every law but Manual to direct
//! control. The stick moves the elevator around the held trim with pitch-rate damping, the aileron
//! is direct, the pedals move the rudder and steer the nose wheel, and the brakes come from the
//! toe brakes. The weight-on-wheels signal is debounced, and the ground law is blended with the
//! flight law over a short time after touchdown and a longer one after lift-off.

use flight::actuators::Actuators;
use flight::aircraft::{AircraftModel, AircraftState};
use flight::controls::PilotControls;
use flight::gear::{MAX_GEAR, WheelCommands};

use crate::model::FcsModel;
use crate::state::{FcsState, Mode};

/// The surface commands and wheel commands of the ground law.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroundCommand {
    /// Elevator, aileron, rudder, rad.
    pub surfaces: [f64; 3],
    /// Brakes and nose-wheel steering.
    pub wheels: WheelCommands,
}

fn authority(min: f64, max: f64) -> f64 {
    min.abs().max(max.abs())
}

/// The toe brakes on each braked leg: the left or right pedal by side of the aircraft, both for
/// a leg on the centreline.
#[must_use]
pub fn brakes(model: &AircraftModel, c: &PilotControls) -> [f64; MAX_GEAR] {
    let mut out = [0.0; MAX_GEAR];
    for (b, leg) in out.iter_mut().zip(model.gear.legs()) {
        *b = if !leg.braked {
            0.0
        } else if leg.pos_ext_body_m.y > 0.0 {
            c.brake_r
        } else if leg.pos_ext_body_m.y < 0.0 {
            c.brake_l
        } else {
            c.brake_l.max(c.brake_r)
        };
    }
    out
}

/// The ground law for the controls and the state.
#[must_use]
pub fn command(
    model: &AircraftModel,
    fcs: &FcsModel,
    ac: &AircraftState,
    c: &PilotControls,
    ground_speed_mps: f64,
    pitch_rate_rps: f64,
) -> GroundCommand {
    let a: &Actuators = &model.actuators;
    let elevator = ac.trim_pitch_rad - c.pitch * authority(a.elevator.min, a.elevator.max)
        + fcs.ground.k_q_ground * pitch_rate_rps;
    let aileron = c.roll * authority(a.aileron.min, a.aileron.max);
    let rudder = -c.yaw * authority(a.rudder.min, a.rudder.max);
    GroundCommand {
        surfaces: [
            elevator.clamp(a.elevator.min, a.elevator.max),
            aileron,
            rudder,
        ],
        wheels: WheelCommands {
            brake: brakes(model, c),
            steer_rad: c.yaw * fcs.ground.nws.limit_rad(ground_speed_mps),
        },
    }
}

/// Advances the debounced weight-on-wheels signal and the blend weight by one tick. Returns true
/// when the flight law has just taken over (its state must start from the present one).
pub fn update_blend(fcs: &FcsModel, fs: &mut FcsState, raw_wow: bool) -> bool {
    let dt = fcs.tick_dt_s;
    let g = &fcs.ground;
    if raw_wow == fs.on_ground {
        fs.wow_timer_s = 0.0;
    } else {
        fs.wow_timer_s += dt;
        if fs.wow_timer_s + 1e-6 >= g.wow_debounce_s {
            fs.on_ground = raw_wow;
            fs.wow_timer_s = 0.0;
        }
    }
    let before = fs.ground_weight;
    if fs.on_ground {
        let rate = if g.touchdown_blend_s > 0.0 {
            dt / g.touchdown_blend_s
        } else {
            1.0
        };
        fs.ground_weight = (fs.ground_weight + rate).min(1.0);
    } else {
        let rate = if g.liftoff_blend_s > 0.0 {
            dt / g.liftoff_blend_s
        } else {
            1.0
        };
        fs.ground_weight = (fs.ground_weight - rate).max(0.0);
    }
    before >= 1.0 && fs.ground_weight < 1.0
}

/// The mode to report for a weight and a blend.
#[must_use]
pub fn mode_for(fs: &FcsState) -> Mode {
    if fs.ground_weight >= 1.0 {
        Mode::Ground
    } else if fs.ground_weight > 0.0 || fs.blend_left_s > 0.0 {
        Mode::Blend
    } else {
        Mode::Flight
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Assist;
    use crate::test_content::kestrel;

    #[test]
    fn the_weight_on_wheels_is_debounced_and_the_blend_runs_at_its_own_rates() {
        let (_, fcs) = kestrel();
        let mut fs = FcsState::new(Assist::Standard, true);
        // A single tick of "no weight" does nothing.
        update_blend(&fcs, &mut fs, false);
        assert!(fs.on_ground && (fs.ground_weight - 1.0).abs() < 1e-12);
        update_blend(&fcs, &mut fs, true);
        assert_eq!(fs.wow_timer_s, 0.0, "it agreed again");
        // Six ticks (0.1 s) of no weight lift the aircraft off; the weight then falls over 2 s.
        let mut took_over = false;
        for _ in 0..6 {
            took_over |= update_blend(&fcs, &mut fs, false);
        }
        assert!(!fs.on_ground);
        assert!(
            took_over,
            "the flight law starts when the weight first drops"
        );
        let mut ticks = 0;
        while fs.ground_weight > 0.0 {
            update_blend(&fcs, &mut fs, false);
            ticks += 1;
        }
        assert!((f64::from(ticks) / 60.0 - 2.0).abs() < 0.1, "{ticks} ticks");
        // Touchdown: a quarter of a second after the debounce the weight is half.
        for _ in 0..6 {
            update_blend(&fcs, &mut fs, true);
        }
        assert!(fs.on_ground);
        for _ in 0..15 {
            update_blend(&fcs, &mut fs, true);
        }
        assert!((fs.ground_weight - 0.5).abs() < 0.1, "{}", fs.ground_weight);
        for _ in 0..60 {
            update_blend(&fcs, &mut fs, true);
        }
        assert_eq!(fs.ground_weight, 1.0);
    }

    #[test]
    fn the_ground_law_signs_and_the_steering_schedule() {
        let (model, fcs) = kestrel();
        let world = flight::env::SphereSea::default();
        let (ac, _) = model
            .static_state(
                1800.0,
                0.0,
                0.0,
                simmath::math::DVec3::ZERO,
                &world,
                flight::engine::EngineCommand::IDLE,
            )
            .unwrap();
        let none = command(&model, &fcs, &ac, &PilotControls::default(), 0.0, 0.0);
        assert_eq!(none.surfaces, [0.0, 0.0, 0.0]);
        // Stick back: trailing edge up (negative), as Manual. Right stick: positive aileron.
        let back = command(
            &model,
            &fcs,
            &ac,
            &PilotControls {
                pitch: 1.0,
                roll: 1.0,
                ..PilotControls::default()
            },
            0.0,
            0.0,
        );
        assert!(back.surfaces[0] < -0.4 && back.surfaces[1] > 0.3);
        // Right pedal: rudder negative, nose wheel to the right; full range slow, 5 deg fast.
        let slow = command(
            &model,
            &fcs,
            &ac,
            &PilotControls {
                yaw: 1.0,
                ..PilotControls::default()
            },
            5.0,
            0.0,
        );
        assert!(slow.surfaces[2] < 0.0);
        assert!((slow.wheels.steer_rad - 60f64.to_radians()).abs() < 1e-6);
        let fast = command(
            &model,
            &fcs,
            &ac,
            &PilotControls {
                yaw: 1.0,
                ..PilotControls::default()
            },
            60.0,
            0.0,
        );
        assert!((fast.wheels.steer_rad - 5f64.to_radians()).abs() < 1e-6);
        // Pitch-rate damping: a nose-up rate adds nose-down (positive) elevator.
        let damped = command(&model, &fcs, &ac, &PilotControls::default(), 0.0, 0.2);
        assert!((damped.surfaces[0] - 0.25 * 0.2).abs() < 1e-12);
        // The toe brakes go to the legs on their side; the nose leg takes the larger.
        let toe = brakes(
            &model,
            &PilotControls {
                brake_l: 0.3,
                brake_r: 0.9,
                ..PilotControls::default()
            },
        );
        assert_eq!(toe[0], 0.9, "the centreline leg");
        assert_eq!(toe[1], 0.3, "left");
        assert_eq!(toe[2], 0.9, "right");
        assert_eq!(toe[3], 0.0);
    }

    #[test]
    fn the_mode_follows_the_weight() {
        let (_, _) = kestrel();
        let mut fs = FcsState::new(Assist::Standard, false);
        assert_eq!(mode_for(&fs), Mode::Flight);
        fs.ground_weight = 0.4;
        assert_eq!(mode_for(&fs), Mode::Blend);
        fs.ground_weight = 1.0;
        assert_eq!(mode_for(&fs), Mode::Ground);
        fs.ground_weight = 0.0;
        fs.blend_left_s = 0.2;
        assert_eq!(
            mode_for(&fs),
            Mode::Blend,
            "an assist switch is a blend too"
        );
    }
}
