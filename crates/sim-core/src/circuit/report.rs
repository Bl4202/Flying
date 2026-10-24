//! Judging a circuit (ADR-33 section 1): the nine criteria, from what the player could see.
//!
//! The judge is fed one [`Sample`] per tick, read from the published snapshot and the tester's
//! phase, never from the tester's own beliefs. [`Judge::finish`] gives the [`CircuitReport`]. The
//! runner that feeds it from a core is [`fly`].

use fcs::tester::{Phase, stall_speed, take_off_tef};

use crate::layout::{airfield, ownship_flag, ownship_flag2, snapshot as snap};
use crate::sim::SimCore;
use crate::time::SIM_DT;

/// Lift-off within this distance of brake release, m. // DATA: ADR-33 C1
pub const LIFT_OFF_MAX_M: f64 = 900.0;
/// Lift-off speed over the take-off stall speed, at most. // DATA: ADR-33 C1
pub const LIFT_OFF_SPEED_MAX: f64 = 1.4;
/// Gear lever up within this long of lift-off, s. // DATA: ADR-33 C3
pub const GEAR_UP_MAX_S: f64 = 20.0;
/// The downwind leg's height band about the circuit height, m. // DATA: ADR-33 C4
pub const DOWNWIND_AGL_BAND_M: f64 = 60.0;
/// The downwind leg's speed band about its speed, m/s. // DATA: ADR-33 C4
pub const DOWNWIND_SPEED_BAND_MPS: f64 = 10.0;
/// How long the downwind leg must hold both, s. // DATA: ADR-33 C4
pub const DOWNWIND_HOLD_S: f64 = 20.0;
/// The wheel height at which the centreline is judged, m. // DATA: ADR-33 C5
pub const CENTRELINE_AGL_M: f64 = 50.0;
/// The offset allowed there, m. // DATA: ADR-33 C5
pub const CENTRELINE_MAX_M: f64 = 10.0;
/// The touchdown sink rate at most, m/s. // DATA: ADR-33 C6
pub const SINK_MAX_MPS: f64 = 3.0;
/// The touchdown zone past the threshold, m. // DATA: ADR-33 C6
pub const TOUCHDOWN_ZONE_M: (f64, f64) = (150.0, 900.0);
/// The offset allowed through the rollout, m. // DATA: ADR-33 C7
pub const ROLLOUT_MAX_M: f64 = 15.0;
/// The aircraft is stopped below this ground speed, m/s. // DATA: ADR-33 C7
pub const STOPPED_MPS: f64 = 2.0;
/// The whole sortie takes at most this long, s. // DATA: ADR-33 C9
pub const TOTAL_MAX_S: f64 = 420.0;
/// The half-track of the main gear, m: both mains must be on the runway at touchdown.
const MAIN_HALF_TRACK_M: f64 = 1.45;

/// What the judge sees of one tick.
#[allow(
    clippy::struct_excessive_bools,
    reason = "one flag per fact the judge reads"
)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample {
    /// Simulated time, s.
    pub t_s: f64,
    /// The tester's phase.
    pub phase: Phase,
    /// Distance along the runway from the threshold, m.
    pub along_m: f64,
    /// Distance to the right of the centreline, m.
    pub across_m: f64,
    /// Height of the lowest wheel above the ground, m.
    pub agl_m: f64,
    /// Calibrated airspeed, m/s.
    pub cas_mps: f64,
    /// Ground speed, m/s.
    pub gs_mps: f64,
    /// Angle of attack, rad.
    pub alpha_rad: f64,
    /// Load factor, g.
    pub nz_g: f64,
    /// Weight on wheels.
    pub wow: bool,
    /// The stall warning is on.
    pub stall_warning: bool,
    /// The aircraft is destroyed.
    pub crashed: bool,
    /// The integrator gave up.
    pub numerical_fault: bool,
    /// The overstress warning is on.
    pub overstress: bool,
    /// The pilot is out.
    pub gloc: bool,
    /// The gear lever is down.
    pub gear_lever_down: bool,
    /// Gear position, 0 up to 1 down.
    pub gear_pos: f64,
    /// The parking brake is set.
    pub parking_brake: bool,
    /// The sink rate of the last touchdown, m/s.
    pub touchdown_sink_mps: f64,
    /// The take-off stall speed at this mass, m/s.
    pub stall_speed_to_mps: f64,
}

/// One criterion's outcome.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Criterion {
    /// `C1` to `C9`.
    pub id: &'static str,
    /// What it asks, in words.
    pub what: &'static str,
    /// It held.
    pub pass: bool,
    /// What was measured (NaN if it never happened).
    pub value: f64,
    /// The limit.
    pub limit: f64,
}

/// The outcome of one circuit.
#[derive(Clone, Debug, PartialEq)]
pub struct CircuitReport {
    /// The assist level (`assist_level`).
    pub assist: u8,
    /// The mass at the start, kg.
    pub mass_kg: f64,
    /// Every criterion held.
    pub pass: bool,
    /// The run hit its time cap before the aircraft stopped.
    pub timeout: bool,
    /// The nine criteria.
    pub criteria: [Criterion; 9],
    /// When each phase began, s (NaN if it never did), indexed by `Phase as usize`.
    pub phase_times_s: [f64; Phase::COUNT],
    /// The roll from brake release to lift-off, m.
    pub lift_off_roll_m: f64,
    /// The calibrated airspeed at lift-off over the take-off stall speed.
    pub lift_off_speed_ratio: f64,
    /// The largest angle of attack, deg.
    pub max_alpha_deg: f64,
    /// The largest load factor, g.
    pub max_nz_g: f64,
    /// The sink rate at touchdown, m/s.
    pub touchdown_sink_mps: f64,
    /// Where the wheels first touched, m past the threshold.
    pub touchdown_along_m: f64,
    /// From touchdown to the stop, m.
    pub stop_distance_m: f64,
    /// Time of the whole sortie, s.
    pub total_s: f64,
}

impl CircuitReport {
    /// The criteria that failed.
    pub fn failures(&self) -> impl Iterator<Item = &Criterion> {
        self.criteria.iter().filter(|c| !c.pass)
    }
}

/// Collects the facts of a run and judges them.
#[allow(
    clippy::struct_excessive_bools,
    reason = "one latch per fact the judge keeps"
)]
#[derive(Clone, Debug)]
pub struct Judge {
    mass_kg: f64,
    assist: u8,
    last: Option<Sample>,
    release: Option<(f64, f64)>,
    lift_off: Option<(f64, f64, f64, f64)>,
    gear_up_lever_s: Option<f64>,
    gear_up_s: Option<f64>,
    stall_in_window: bool,
    stall_in_final: bool,
    downwind_run_s: f64,
    downwind_best_s: f64,
    centreline: Option<f64>,
    touchdown: Option<(f64, f64, f64, f64, bool)>,
    rollout_max_m: f64,
    stopped: Option<(f64, f64, f64)>,
    any_crash: bool,
    any_fault: bool,
    any_overstress: bool,
    any_gloc: bool,
    phase_times: [f64; Phase::COUNT],
    max_alpha: f64,
    max_nz: Option<f64>,
    was_airborne_late: bool,
}

impl Judge {
    /// A judge for a run at `assist` starting at `mass_kg`.
    #[must_use]
    pub fn new(assist: u8, mass_kg: f64) -> Self {
        Self {
            mass_kg,
            assist,
            last: None,
            release: None,
            lift_off: None,
            gear_up_lever_s: None,
            gear_up_s: None,
            stall_in_window: false,
            stall_in_final: false,
            downwind_run_s: 0.0,
            downwind_best_s: 0.0,
            centreline: None,
            touchdown: None,
            rollout_max_m: 0.0,
            stopped: None,
            any_crash: false,
            any_fault: false,
            any_overstress: false,
            any_gloc: false,
            phase_times: [f64::NAN; Phase::COUNT],
            max_alpha: 0.0,
            max_nz: None,
            was_airborne_late: false,
        }
    }

    /// Takes one tick.
    pub fn observe(&mut self, s: &Sample) {
        let prev = self.last.replace(*s);
        let pt = &mut self.phase_times[s.phase as usize];
        if pt.is_nan() {
            *pt = s.t_s;
        }
        self.max_alpha = self.max_alpha.max(s.alpha_rad.abs());
        self.max_nz = Some(self.max_nz.map_or(s.nz_g, |m| m.max(s.nz_g)));
        self.any_crash |= s.crashed;
        self.any_fault |= s.numerical_fault;
        self.any_overstress |= s.overstress;
        self.any_gloc |= s.gloc;

        // C1: from the brake release to the wheels leaving the ground.
        if self.release.is_none() && !s.parking_brake && s.wow {
            self.release = Some((s.t_s, s.along_m));
        }
        if let Some((_, _)) = self.release
            && self.lift_off.is_none()
            && !s.wow
            && prev.is_some_and(|p| p.wow)
        {
            self.lift_off = Some((s.t_s, s.along_m, s.cas_mps, s.stall_speed_to_mps));
        }
        // C2: the take-off (release to ten seconds after lift-off) and the final approach.
        let in_take_off = self.release.is_some()
            && self.lift_off.is_none_or(|(t, ..)| s.t_s <= t + 10.0)
            && s.phase <= Phase::Climb;
        self.stall_in_window |= in_take_off && s.stall_warning;
        self.stall_in_final |= matches!(s.phase, Phase::Final | Phase::Flare) && s.stall_warning;
        // C3: the gear lever and the gear.
        if let Some((t_lo, ..)) = self.lift_off {
            if self.gear_up_lever_s.is_none() && !s.gear_lever_down {
                self.gear_up_lever_s = Some(s.t_s - t_lo);
            }
            if self.gear_up_s.is_none() && s.gear_pos <= 0.001 {
                self.gear_up_s = Some(s.t_s - t_lo);
            }
        }
        // C4: the longest stretch of the downwind leg inside both bands.
        if s.phase == Phase::Downwind
            && (s.agl_m - fcs::tester::CIRCUIT_AGL_M).abs() <= DOWNWIND_AGL_BAND_M
            && (s.cas_mps - fcs::tester::DOWNWIND_CAS_MPS).abs() <= DOWNWIND_SPEED_BAND_MPS
        {
            self.downwind_run_s += SIM_DT;
            self.downwind_best_s = self.downwind_best_s.max(self.downwind_run_s);
        } else {
            self.downwind_run_s = 0.0;
        }
        // C5: the first tick on the final at or below 50 m.
        if self.centreline.is_none() && s.phase == Phase::Final && s.agl_m <= CENTRELINE_AGL_M {
            self.centreline = Some(s.across_m);
        }
        // C6: the first touchdown of the landing.
        let late = s.phase >= Phase::Final;
        if late && !s.wow {
            self.was_airborne_late = true;
        }
        if self.touchdown.is_none() && late && self.was_airborne_late && s.wow {
            let mains_on = s.across_m.abs() + MAIN_HALF_TRACK_M <= 0.5 * airfield::WIDTH_M as f64;
            self.touchdown = Some((s.t_s, s.along_m, s.touchdown_sink_mps, s.across_m, mains_on));
        }
        // C7: the rollout and the stop.
        if self.touchdown.is_some() && self.stopped.is_none() {
            self.rollout_max_m = self.rollout_max_m.max(s.across_m.abs());
            if s.wow && s.gs_mps < STOPPED_MPS {
                self.stopped = Some((s.t_s, s.along_m, s.across_m));
            }
        }
    }

    /// The report. `timed_out` says the run was cut off by its time cap.
    #[must_use]
    #[allow(clippy::too_many_lines, reason = "the nine criteria, one table")]
    pub fn finish(&self, timed_out: bool) -> CircuitReport {
        let c = |id, what, pass, value, limit| Criterion {
            id,
            what,
            pass,
            value,
            limit,
        };
        let (roll_m, lof_ratio) = match (self.release, self.lift_off) {
            (Some((_, a0)), Some((_, a1, cas, vs))) => (a1 - a0, cas / vs),
            _ => (f64::NAN, f64::NAN),
        };
        let nan = f64::NAN;
        let (td_along, td_sink, mains_on) = self
            .touchdown
            .map_or((nan, nan, false), |(_, a, v, _, m)| (a, v, m));
        let stop_distance = match (self.touchdown, self.stopped) {
            (Some((_, a0, ..)), Some((_, a1, _))) => a1 - a0,
            _ => nan,
        };
        let total_s = self
            .stopped
            .map_or(self.last.map_or(0.0, |s| s.t_s), |(t, ..)| t);
        let on_runway = self.stopped.is_some_and(|(_, along, across)| {
            (0.0..=airfield::LENGTH_M as f64).contains(&along)
                && across.abs() <= 0.5 * airfield::WIDTH_M as f64
        });
        let criteria = [
            c(
                "C1",
                "lift-off within 900 m of brake release, at most 1.4 V_s,TO",
                roll_m <= LIFT_OFF_MAX_M && lof_ratio <= LIFT_OFF_SPEED_MAX,
                roll_m,
                LIFT_OFF_MAX_M,
            ),
            c(
                "C2",
                "no stall warning on take-off or final",
                !self.stall_in_window && !self.stall_in_final,
                f64::from(u8::from(self.stall_in_window || self.stall_in_final)),
                0.0,
            ),
            c(
                "C3",
                "gear lever up within 20 s of lift-off, and the gear reaches 0",
                self.gear_up_lever_s.is_some_and(|t| t <= GEAR_UP_MAX_S)
                    && self.gear_up_s.is_some(),
                self.gear_up_lever_s.unwrap_or(nan),
                GEAR_UP_MAX_S,
            ),
            c(
                "C4",
                "downwind 300 +- 60 m and 130 +- 10 m/s for 20 s",
                self.downwind_best_s >= DOWNWIND_HOLD_S,
                self.downwind_best_s,
                DOWNWIND_HOLD_S,
            ),
            c(
                "C5",
                "within 10 m of the centreline at 50 m on final",
                self.centreline.is_some_and(|x| x.abs() <= CENTRELINE_MAX_M),
                self.centreline.map_or(nan, f64::abs),
                CENTRELINE_MAX_M,
            ),
            c(
                "C6",
                "touchdown at most 3.0 m/s, 150 to 900 m past the threshold, on the runway",
                self.touchdown.is_some()
                    && td_sink <= SINK_MAX_MPS
                    && (TOUCHDOWN_ZONE_M.0..=TOUCHDOWN_ZONE_M.1).contains(&td_along)
                    && mains_on,
                td_sink,
                SINK_MAX_MPS,
            ),
            c(
                "C7",
                "within 15 m of the centreline in the rollout, stopped on the runway",
                self.stopped.is_some() && self.rollout_max_m <= ROLLOUT_MAX_M && on_runway,
                self.rollout_max_m,
                ROLLOUT_MAX_M,
            ),
            c(
                "C8",
                "no crash, G-LOC, numerical fault or overstress",
                !(self.any_crash || self.any_gloc || self.any_fault || self.any_overstress),
                f64::from(
                    u8::from(self.any_crash)
                        + u8::from(self.any_gloc)
                        + u8::from(self.any_fault)
                        + u8::from(self.any_overstress),
                ),
                0.0,
            ),
            c(
                "C9",
                "stopped within 420 s",
                self.stopped.is_some() && total_s <= TOTAL_MAX_S,
                total_s,
                TOTAL_MAX_S,
            ),
        ];
        CircuitReport {
            assist: self.assist,
            mass_kg: self.mass_kg,
            pass: criteria.iter().all(|c| c.pass),
            timeout: timed_out && self.stopped.is_none(),
            criteria,
            phase_times_s: self.phase_times,
            lift_off_roll_m: roll_m,
            lift_off_speed_ratio: lof_ratio,
            max_alpha_deg: self.max_alpha.to_degrees(),
            max_nz_g: self.max_nz.unwrap_or(nan),
            touchdown_sink_mps: td_sink,
            touchdown_along_m: td_along,
            stop_distance_m: stop_distance,
            total_s,
        }
    }
}

/// The time the run is cut off at, s: the gate's 420 s and the margin that lets the aircraft
/// finish its stop.
pub const RUN_CAP_S: f64 = TOTAL_MAX_S + 30.0;

fn f32_le(b: &[u8], o: usize) -> f64 {
    f64::from(f32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]))
}

fn f64_le(b: &[u8], o: usize) -> f64 {
    f64::from_le_bytes([
        b[o],
        b[o + 1],
        b[o + 2],
        b[o + 3],
        b[o + 4],
        b[o + 5],
        b[o + 6],
        b[o + 7],
    ])
}

/// Reads one tick of a core into a sample. `None` without an aircraft or a tester.
#[must_use]
pub fn sample_of(core: &SimCore) -> Option<Sample> {
    let slot = core.ownship().aircraft()?;
    let tester = core.circuit_tester()?;
    let model = core.services().aircraft.as_ref()?;
    let b = core.snapshot_bytes();
    let f32_at = |o: usize| f32_le(b, o);
    let f64_at = |o: usize| f64_le(b, o);
    let flags = u32::from(b[snap::O_FLAGS]);
    let flags2 = u32::from(u16::from_le_bytes([
        b[snap::O_FLAGS2],
        b[snap::O_FLAGS2 + 1],
    ]));
    let x = f64_at(snap::O_POS_BF);
    let y = f64_at(snap::O_POS_BF + 8);
    let runway_y = -(airfield::THRESHOLD_SOUTH_M as f64);
    let mass = f32_at(snap::O_MASS);
    let (vx, vy) = (f32_at(snap::O_VEL_BF), f32_at(snap::O_VEL_BF + 4));
    Some(Sample {
        t_s: core.tick() as f64 * SIM_DT,
        phase: tester.phase(),
        along_m: y - runway_y,
        across_m: x,
        agl_m: f32_at(snap::O_AGL),
        cas_mps: f32_at(snap::O_CAS),
        gs_mps: (vx * vx + vy * vy).sqrt().max(f32_at(snap::O_GROUND_SPEED)),
        alpha_rad: f32_at(snap::O_ALPHA),
        nz_g: f32_at(snap::O_NZ),
        wow: flags2 & ownship_flag2::WOW as u32 != 0,
        stall_warning: flags & ownship_flag::STALL_WARN as u32 != 0,
        crashed: slot.crashed || flags & ownship_flag::CRASHED as u32 != 0,
        numerical_fault: flags & ownship_flag::NUMERICAL_FAULT as u32 != 0,
        overstress: flags2 & ownship_flag2::OVERSTRESS as u32 != 0,
        gloc: flags2 & ownship_flag2::GLOC as u32 != 0,
        gear_lever_down: b[snap::O_GEAR_BITS] & 0x40 != 0,
        gear_pos: f32_at(snap::O_GEAR_POS),
        parking_brake: flags2 & ownship_flag2::PARKING_BRAKE as u32 != 0,
        touchdown_sink_mps: f32_at(snap::O_TOUCHDOWN_VS),
        stall_speed_to_mps: stall_speed(model, mass, take_off_tef(model)),
    })
}

/// Flies the circuit on `core` (started, with the circuit script, on the runway) until the
/// aircraft stops or [`RUN_CAP_S`] passes, and judges it. `observe` sees every sample.
pub fn fly(core: &mut SimCore, assist: u8, mut observe: impl FnMut(&Sample)) -> CircuitReport {
    let mass = f32_le(core.snapshot_bytes(), snap::O_MASS);
    let mut judge = Judge::new(assist, mass);
    let mut stopped_at: Option<f64> = None;
    let ticks = (RUN_CAP_S / SIM_DT) as u64;
    for _ in 0..ticks {
        core.step(1);
        let Some(s) = sample_of(core) else { break };
        observe(&s);
        judge.observe(&s);
        if s.phase == Phase::Stopped {
            // A moment longer, so a creeping aircraft shows.
            let t0 = *stopped_at.get_or_insert(s.t_s);
            if s.t_s - t0 > 2.0 {
                break;
            }
        }
        if s.crashed {
            break;
        }
    }
    judge.finish(stopped_at.is_none())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A run that does everything right, tick by tick, for the judge to be shown breaking.
    struct Ideal {
        /// Where the wheels leave the ground, m past the threshold (the brake is released at 150).
        lift_off_along: f64,
        /// The calibrated speed at lift-off, m/s.
        lift_off_cas: f64,
        /// The take-off stall speed, m/s.
        stall_to: f64,
        /// The gear lever goes up this long after lift-off, s.
        lever_up_after_s: f64,
        /// The downwind leg's speed, m/s.
        downwind_cas: f64,
        /// The offset from the centreline at 50 m on final, m.
        offset_at_50: f64,
        /// The sink rate at touchdown, m/s.
        sink: f64,
        /// Where the wheels touch down, m past the threshold.
        td_along: f64,
        /// The largest offset in the rollout, m.
        rollout_offset: f64,
        /// When the aircraft stops, s.
        stop_s: f64,
        /// A stall warning on final.
        stall_on_final: bool,
        /// The aircraft is destroyed at the touchdown.
        crash: bool,
        /// The pilot is out at some point.
        gloc: bool,
    }

    impl Ideal {
        fn good() -> Self {
            Self {
                lift_off_along: 600.0,
                lift_off_cas: 75.0,
                stall_to: 55.6,
                lever_up_after_s: 6.0,
                downwind_cas: 130.0,
                offset_at_50: 3.0,
                sink: 1.5,
                td_along: 420.0,
                rollout_offset: 2.0,
                stop_s: 220.0,
                stall_on_final: false,
                crash: false,
                gloc: false,
            }
        }

        fn run(&self) -> CircuitReport {
            let mut j = Judge::new(2, 10_260.0);
            let dt = SIM_DT;
            let mut t = 0.0;
            let lift_off_s = 12.0;
            while t <= self.stop_s + 3.0 {
                let phase = match t {
                    t if t < 2.0 => Phase::Hold,
                    t if t < 9.0 => Phase::Roll,
                    t if t < lift_off_s => Phase::Rotate,
                    t if t < 30.0 => Phase::Climb,
                    t if t < 40.0 => Phase::Crosswind,
                    t if t < 110.0 => Phase::Downwind,
                    t if t < 140.0 => Phase::Base,
                    t if t < 195.0 => Phase::Final,
                    t if t < 200.0 => Phase::Flare,
                    t if t < self.stop_s => Phase::Rollout,
                    _ => Phase::Stopped,
                };
                let airborne = (lift_off_s..200.0).contains(&t);
                let landed = t >= 200.0;
                let on_final = phase == Phase::Final;
                // Height: 300 m on the circuit, falling to 50 m at t = 190 and to 0 at 200.
                let agl = if !airborne {
                    0.0
                } else if t < 30.0 {
                    (t - lift_off_s) * 16.0
                } else if t < 140.0 {
                    300.0
                } else if t < 190.0 {
                    300.0 - (t - 140.0) * 5.0
                } else {
                    50.0 * (200.0 - t) / 10.0
                };
                let across = if on_final && agl <= CENTRELINE_AGL_M {
                    self.offset_at_50
                } else if landed {
                    self.rollout_offset
                } else {
                    0.0
                };
                let lever_down = !(t >= lift_off_s + self.lever_up_after_s && t < 140.0);
                let gear_pos = if t < lift_off_s + 8.0 || (lever_down && t < 140.0) || t >= 140.0 {
                    1.0
                } else {
                    0.0
                };
                j.observe(&Sample {
                    t_s: t,
                    phase,
                    along_m: if t < lift_off_s {
                        150.0 + (self.lift_off_along - 150.0) * (t / lift_off_s) * (t / lift_off_s)
                    } else if landed {
                        self.td_along
                            + (900.0 - self.td_along)
                                * ((t - 200.0) / (self.stop_s - 200.0)).min(1.0)
                    } else {
                        self.lift_off_along + (t - lift_off_s) * 100.0
                    },
                    across_m: across,
                    agl_m: agl,
                    cas_mps: if t < lift_off_s {
                        self.lift_off_cas * t / lift_off_s
                    } else if phase == Phase::Climb {
                        self.lift_off_cas
                    } else if phase == Phase::Downwind {
                        self.downwind_cas
                    } else {
                        72.0
                    },
                    gs_mps: if t >= self.stop_s { 0.5 } else { 30.0 },
                    alpha_rad: 0.1,
                    nz_g: 1.2,
                    wow: !airborne,
                    stall_warning: self.stall_on_final && on_final,
                    crashed: self.crash && t >= 200.0,
                    numerical_fault: false,
                    overstress: false,
                    gloc: self.gloc && t > 100.0,
                    gear_lever_down: lever_down,
                    gear_pos,
                    parking_brake: t < 2.5,
                    touchdown_sink_mps: if landed { self.sink } else { 0.0 },
                    stall_speed_to_mps: self.stall_to,
                });
                t += dt;
            }
            j.finish(false)
        }
    }

    fn only_failure(r: &CircuitReport) -> Vec<&'static str> {
        r.failures().map(|c| c.id).collect()
    }

    #[test]
    fn a_clean_circuit_passes_every_criterion() {
        let r = Ideal::good().run();
        assert!(r.pass, "{:?}", only_failure(&r));
        assert!(!r.timeout);
        assert!((r.touchdown_along_m - 420.0).abs() < 2.0);
        assert!((r.touchdown_sink_mps - 1.5).abs() < 1e-9);
        assert!(r.stop_distance_m > 400.0 && r.total_s > 200.0 && r.total_s < 221.0);
        assert!(r.phase_times_s[Phase::Rotate as usize] > 8.9);
        assert_eq!(r.phase_times_s[Phase::Hold as usize], 0.0);
    }

    #[test]
    fn c1_a_long_roll_or_a_fast_lift_off_fails() {
        let r = Ideal {
            lift_off_along: 1_200.0,
            ..Ideal::good()
        }
        .run();
        assert_eq!(only_failure(&r), ["C1"], "too far");
        let r = Ideal {
            lift_off_cas: 80.0,
            ..Ideal::good()
        }
        .run();
        assert_eq!(only_failure(&r), ["C1"], "80 over 55.6 is 1.44");
    }

    #[test]
    fn c2_a_stall_warning_on_final_fails() {
        let r = Ideal {
            stall_on_final: true,
            ..Ideal::good()
        }
        .run();
        assert_eq!(only_failure(&r), ["C2"]);
    }

    #[test]
    fn c3_a_late_gear_fails() {
        let r = Ideal {
            lever_up_after_s: 25.0,
            ..Ideal::good()
        }
        .run();
        assert_eq!(only_failure(&r), ["C3"]);
    }

    #[test]
    fn c4_a_downwind_too_fast_fails() {
        let r = Ideal {
            downwind_cas: 145.0,
            ..Ideal::good()
        }
        .run();
        assert_eq!(only_failure(&r), ["C4"]);
    }

    #[test]
    fn c5_a_final_off_the_centreline_fails() {
        let r = Ideal {
            offset_at_50: 12.0,
            ..Ideal::good()
        }
        .run();
        assert_eq!(only_failure(&r), ["C5"]);
        let r = Ideal {
            offset_at_50: -12.0,
            ..Ideal::good()
        }
        .run();
        assert_eq!(only_failure(&r), ["C5"], "either side");
    }

    #[test]
    fn c6_a_hard_or_short_or_long_or_wide_touchdown_fails() {
        for (what, ideal) in [
            (
                "hard",
                Ideal {
                    sink: 3.4,
                    ..Ideal::good()
                },
            ),
            (
                "short",
                Ideal {
                    td_along: 100.0,
                    ..Ideal::good()
                },
            ),
            (
                "long",
                Ideal {
                    td_along: 950.0,
                    stop_s: 230.0,
                    ..Ideal::good()
                },
            ),
        ] {
            assert_eq!(only_failure(&ideal.run()), ["C6"], "{what}");
        }
        // Off the edge of the runway: a main wheel in the grass.
        let r = Ideal {
            rollout_offset: 21.9,
            ..Ideal::good()
        }
        .run();
        let failed = only_failure(&r);
        assert!(
            failed.contains(&"C6") && failed.contains(&"C7"),
            "{failed:?}"
        );
    }

    #[test]
    fn c7_a_wandering_rollout_fails() {
        let r = Ideal {
            rollout_offset: 16.0,
            ..Ideal::good()
        }
        .run();
        let failed = only_failure(&r);
        assert!(failed.contains(&"C7"), "{failed:?}");
    }

    #[test]
    fn c8_a_crash_or_a_g_loc_fails() {
        let r = Ideal {
            crash: true,
            ..Ideal::good()
        }
        .run();
        assert!(only_failure(&r).contains(&"C8"));
        let r = Ideal {
            gloc: true,
            ..Ideal::good()
        }
        .run();
        assert_eq!(only_failure(&r), ["C8"]);
    }

    #[test]
    fn c9_a_slow_sortie_fails_and_one_that_never_stops_times_out() {
        let r = Ideal {
            stop_s: 430.0,
            ..Ideal::good()
        }
        .run();
        assert_eq!(only_failure(&r), ["C9"]);
        // Never stopped: the judge was given nothing past the touchdown.
        let mut j = Judge::new(2, 10_000.0);
        j.observe(&Sample {
            t_s: 1.0,
            ..Ideal::good_sample()
        });
        let r = j.finish(true);
        assert!(r.timeout && !r.pass);
        assert!(r.failures().any(|c| c.id == "C9"));
    }

    impl Ideal {
        fn good_sample() -> Sample {
            Sample {
                t_s: 0.0,
                phase: Phase::Hold,
                along_m: 150.0,
                across_m: 0.0,
                agl_m: 0.0,
                cas_mps: 0.0,
                gs_mps: 0.0,
                alpha_rad: 0.0,
                nz_g: 1.0,
                wow: true,
                stall_warning: false,
                crashed: false,
                numerical_fault: false,
                overstress: false,
                gloc: false,
                gear_lever_down: true,
                gear_pos: 1.0,
                parking_brake: true,
                touchdown_sink_mps: 0.0,
                stall_speed_to_mps: 55.0,
            }
        }
    }

    #[test]
    fn the_numerical_fault_and_overstress_flags_count_as_failures() {
        let mut j = Judge::new(2, 10_000.0);
        j.observe(&Sample {
            overstress: true,
            ..Ideal::good_sample()
        });
        assert!(j.finish(false).failures().any(|c| c.id == "C8"));
        let mut j = Judge::new(2, 10_000.0);
        j.observe(&Sample {
            numerical_fault: true,
            ..Ideal::good_sample()
        });
        assert!(j.finish(false).failures().any(|c| c.id == "C8"));
    }

    #[test]
    fn nothing_observed_is_not_a_pass() {
        let r = Judge::new(2, 0.0).finish(false);
        assert!(!r.pass);
        assert_eq!(
            r.criteria.iter().filter(|c| c.pass).count(),
            2,
            "only C2 and C8 are vacuous"
        );
    }
}
