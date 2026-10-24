//! The circuit gate (ADR-33): the virtual tester flying the aircraft from the runway spawn through
//! the real input path, and the report of whether it did what the gate asks.
//!
//! [`runway`] and [`frame`] put the tester into the core's input; [`report`] watches a run from
//! the outside, through the snapshot, and judges it.

pub mod report;

pub use fcs::tester::Phase;

use fcs::FcsModel;
use fcs::tester::geom::Runway;
use fcs::tester::{Levers, Tester};
use flight::aircraft::AircraftModel;

use crate::input::InputFrame;
use crate::layout::{airfield, axis, button};
use crate::math::DVec3;
use crate::ownship::AircraftSlot;
use crate::seams::geo::GeoService;
use crate::services::Services;
use crate::time::sim_time;

/// The runway of the test airfield as the tester sees it.
#[must_use]
pub fn runway(geo: &dyn GeoService) -> Runway {
    let y = -(airfield::THRESHOLD_SOUTH_M as f64);
    Runway {
        threshold_bf: DVec3::new(0.0, y, geo.ground_z_bf(0.0, y)),
        heading_rad: 0.0,
        length_m: airfield::LENGTH_M as f64,
        width_m: airfield::WIDTH_M as f64,
    }
}

/// The input frame the tester writes for `tick`, from the aircraft as the last tick left it.
#[must_use]
pub fn frame(
    tester: &mut Tester,
    slot: &AircraftSlot,
    model: &AircraftModel,
    fcs_model: &FcsModel,
    svc: &Services,
    tick: u64,
) -> InputFrame {
    let levers = Levers {
        gear_down: slot.gear_lever_down,
        flap_detent: slot.flap_detent,
        parking_brake: slot.parking_brake,
    };
    let out = tester.step(
        model,
        fcs_model,
        &slot.state,
        &slot.fcs,
        levers,
        &svc.flight_env(),
        sim_time(tick),
    );
    let mut f = InputFrame::neutral(u32::try_from((tick + 1) & 0xFFFF_FFFF).unwrap_or(0));
    f.t_ms = tick as f64 * (1_000.0 / 60.0);
    let c = &out.controls;
    f.axes[axis::PITCH] = c.pitch as f32;
    f.axes[axis::ROLL] = c.roll as f32;
    f.axes[axis::YAW] = c.yaw as f32;
    f.axes[axis::THROTTLE] = c.throttle as f32;
    f.axes[axis::BRAKE_L] = c.brake_l as f32;
    f.axes[axis::BRAKE_R] = c.brake_r as f32;
    let p = &out.presses;
    let mut buttons = 0_u64;
    for (on, bit) in [
        (p.gear, button::GEAR),
        (p.flaps_down, button::FLAPS_DOWN),
        (p.flaps_up, button::FLAPS_UP),
        (p.parking_brake, button::PARKING_BRAKE),
        (c.airbrake > 0.5, button::AIRBRAKE),
    ] {
        if on {
            buttons |= 1 << bit;
        }
    }
    f.buttons = buttons;
    f
}
