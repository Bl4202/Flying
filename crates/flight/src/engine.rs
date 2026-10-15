//! Propulsion (GDD 11.8, Appendix C.3).
//!
//! An engine has a small state ([`EngineState`]) advanced once per sub-step by
//! [`EngineModel::step`]: core spool, afterburner light-off and level, flameout and compressor
//! stall. Thrust and fuel flow are pure functions of that state and the local air, so thrust is
//! re-evaluated in every RK4 stage as density, Mach and angle of attack change.
//!
//! ```text
//! F_net = (F_dry,SL core f_M,dry(M) + (F_ab,SL - F_dry,SL) ab f_M,ab(M)) (rho/rho0)^n eta_inlet(alpha)
//! fuel  = (TSFC_dry + (TSFC_ab - TSFC_dry) ab) F_net sqrt(T / T0)
//! ```
//!
//! `core` is the core's thrust as a fraction of military (idle fraction to 1) and `ab` the
//! afterburner's share of its increment (0 to 1).

use content::EngineDef;
use simmath::dmath;

use crate::error::{ModelError, table};
use crate::physical::{RHO0, T0};
use crate::table::Table1D;

/// Most engines an aircraft may have.
pub const MAX_ENGINES: usize = 4;

/// The throttle position of the military-power detent. // ALGO: GDD 11.8.2
pub const DETENT: f32 = 0.8;
/// How far past the detent (and past each afterburner stage's edge) the lever must be to select
/// the next stage, so that a lever resting on an edge, in either precision, selects the lower
/// one. // ALGO: f32 detent guard (ADR-29)
pub const DETENT_GUARD: f32 = 1e-4;
/// Largest inlet-distortion thrust loss, reached at `start + span`. // ALGO: GDD 11.8.1
pub const INLET_MAX_LOSS: f64 = 0.5;
/// Thrust remaining during a compressor stall. // ALGO: GDD 11.8.4
pub const COMPRESSOR_STALL_THRUST: f64 = 0.3;
/// Angle of attack below which a stalled compressor can recover, rad. // ALGO: GDD 11.8.4 (30 deg)
pub const COMPRESSOR_RECOVER_ALPHA_RAD: f64 = 30.0 * core::f64::consts::PI / 180.0;
/// Time to relight after a flameout or compressor stall, s. // ALGO: GDD 11.8.4
pub const RELIGHT_S: f64 = 3.0;

/// What the throttle asks of an engine.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EngineCommand {
    /// Dry power, 0 (idle) to 1 (military).
    pub dry: f64,
    /// Afterburner stage, 0 for none.
    pub ab_stage: u8,
}

impl EngineCommand {
    /// Idle.
    pub const IDLE: Self = Self {
        dry: 0.0,
        ab_stage: 0,
    };
    /// Military power (full dry).
    pub const MILITARY: Self = Self {
        dry: 1.0,
        ab_stage: 0,
    };

    /// Full afterburner for an engine with `stages` stages (military if it has none).
    #[must_use]
    pub const fn max_ab(stages: u8) -> Self {
        Self {
            dry: 1.0,
            ab_stage: stages,
        }
    }

    /// The command for a throttle lever position `t` in 0 to 1 (GDD 11.8.2): 0 to 0.8 is idle to
    /// military, the detent is 0.8, and past it the afterburner's stages share the rest equally.
    ///
    /// The lever is compared in `f32` with a small guard past the detent, because the lever
    /// reaches the sim as an `f32` and `f64::from(0.8_f32)` is slightly more than 0.8.
    #[must_use]
    pub fn from_lever(t: f64, stages: u8) -> Self {
        let t = if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) } as f32;
        let mut ab_stage = 0;
        for k in 0..stages {
            let edge = DETENT + (1.0 - DETENT) * f32::from(k) / f32::from(stages);
            if t > edge + DETENT_GUARD {
                ab_stage = k + 1;
            }
        }
        if ab_stage == 0 {
            return Self {
                dry: (f64::from(t) / f64::from(DETENT)).min(1.0),
                ab_stage: 0,
            };
        }
        Self { dry: 1.0, ab_stage }
    }
}

/// The air an engine breathes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EngineConditions {
    /// Density, kg/m^3.
    pub rho_kgpm3: f64,
    /// Static temperature, K.
    pub t_k: f64,
    /// Flight Mach number.
    pub mach: f64,
    /// Angle of attack, rad.
    pub alpha_rad: f64,
    /// True airspeed, m/s.
    pub tas_mps: f64,
}

/// One engine's dynamic state. Everything here is integrated state and part of the state hash.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EngineState {
    /// Core thrust as a fraction of military thrust.
    pub core: f64,
    /// Afterburner level, a fraction of the afterburner's thrust increment.
    pub ab_level: f64,
    /// The afterburner is lit.
    pub ab_lit: bool,
    /// Time the afterburner has been selected without being lit, s.
    pub ab_wait_s: f64,
    /// The engine is burning fuel (false after a flameout until it relights).
    pub running: bool,
    /// The compressor is stalled.
    pub compressor_stalled: bool,
    /// Progress of a relight or a compressor-stall recovery, s.
    pub relight_s: f64,
}

impl EngineState {
    /// Running at a steady dry power, afterburner out.
    #[must_use]
    pub const fn running_at(core: f64) -> Self {
        Self {
            core,
            ab_level: 0.0,
            ab_lit: false,
            ab_wait_s: 0.0,
            running: true,
            compressor_stalled: false,
            relight_s: 0.0,
        }
    }

    /// True if every number is finite.
    #[must_use]
    pub fn is_finite(&self) -> bool {
        self.core.is_finite()
            && self.ab_level.is_finite()
            && self.ab_wait_s.is_finite()
            && self.relight_s.is_finite()
    }
}

/// An engine model: state dynamics, thrust and fuel flow.
pub trait EngineModel {
    /// Advances the state by one sub-step. `fuel_available` is false when the tanks feeding the
    /// engine are empty.
    fn step(
        &self,
        state: &mut EngineState,
        cmd: EngineCommand,
        air: &EngineConditions,
        fuel_available: bool,
    );

    /// Net thrust along the engine's axis, N.
    fn thrust_n(&self, state: &EngineState, air: &EngineConditions) -> f64;

    /// Fuel flow at a given net thrust, kg/s.
    fn fuel_flow_kgps(&self, state: &EngineState, thrust_n: f64, air: &EngineConditions) -> f64;

    /// The state of an engine running steadily under `cmd` (afterburner lit and settled if
    /// selected): what a trimmed spawn starts with.
    fn steady(&self, cmd: EngineCommand) -> EngineState;

    /// The afterburner stage currently lit, 0 for none.
    fn ab_stage(&self, state: &EngineState, cmd: EngineCommand) -> u8;
}

/// A turbofan or turbojet with an optional staged afterburner (GDD 11.8).
#[derive(Clone, Debug, PartialEq)]
pub struct Turbofan {
    sl_dry_n: f64,
    sl_ab_n: f64,
    density_exp: f64,
    mach_dry: Table1D,
    mach_ab: Table1D,
    idle_fraction: f64,
    tsfc_dry: f64,
    tsfc_ab: f64,
    ab_stages: u8,
    ab_lightoff_s: f64,
    inlet_start_rad: f64,
    inlet_span_rad: f64,
    stall_alpha_rad: f64,
    windmill_relight_mps: f64,
    dt_s: f64,
    /// `1 - exp(-dt / tau)` for the core spooling up and down and the afterburner rising and
    /// falling: the exact first-order update for the fixed step.
    k_up: f64,
    k_down: f64,
    k_ab_up: f64,
    k_ab_down: f64,
}

fn lag_factor(tau_s: f64, dt_s: f64) -> f64 {
    if tau_s > 0.0 {
        -dmath::exp_m1(-dt_s / tau_s)
    } else {
        1.0
    }
}

impl Turbofan {
    /// Builds the model from an engine entry, stepped every `dt_s` seconds.
    ///
    /// # Errors
    /// [`ModelError::Table`] if a thrust table is malformed.
    pub fn from_def(def: &EngineDef, dt_s: f64) -> Result<Self, ModelError> {
        let f = f64::from;
        let curve = |name, c: &content::Curve| table(name, Table1D::from_f32(&c.xs, &c.ys));
        Ok(Self {
            sl_dry_n: f(def.sl_dry_n),
            sl_ab_n: f(def.sl_ab_n),
            density_exp: f(def.density_exp),
            mach_dry: curve("mach_factor_dry", &def.mach_factor_dry)?,
            mach_ab: curve("mach_factor_ab", &def.mach_factor_ab)?,
            idle_fraction: f(def.idle_fraction),
            tsfc_dry: f(def.tsfc_dry_kg_per_ns),
            tsfc_ab: f(def.tsfc_ab_kg_per_ns),
            ab_stages: def.ab_stages,
            ab_lightoff_s: f(def.ab_lightoff_s),
            inlet_start_rad: f(def.inlet_alpha_start_deg).to_radians(),
            inlet_span_rad: f(def.inlet_alpha_span_deg).to_radians(),
            stall_alpha_rad: f(def.compressor_stall_alpha_deg).to_radians(),
            windmill_relight_mps: f(def.windmill_relight_mps),
            dt_s,
            k_up: lag_factor(f(def.spool_up_s), dt_s),
            k_down: lag_factor(f(def.spool_down_s), dt_s),
            k_ab_up: lag_factor(f(def.ab_stage_s), dt_s),
            k_ab_down: lag_factor(f(def.ab_cutoff_s), dt_s),
        })
    }

    /// Afterburner stages.
    #[must_use]
    pub fn ab_stages(&self) -> u8 {
        self.ab_stages
    }

    /// Inlet efficiency at an angle of attack: `1 - 0.5 s^2`, `s = clamp((|alpha| - start) / span)`.
    #[must_use]
    pub fn inlet_efficiency(&self, alpha_rad: f64) -> f64 {
        let s = ((alpha_rad.abs() - self.inlet_start_rad) / self.inlet_span_rad).clamp(0.0, 1.0);
        1.0 - INLET_MAX_LOSS * s * s
    }

    fn core_target(&self, cmd: EngineCommand) -> f64 {
        self.idle_fraction + (1.0 - self.idle_fraction) * cmd.dry.clamp(0.0, 1.0)
    }

    fn ab_target(&self, cmd: EngineCommand) -> f64 {
        if self.ab_stages == 0 {
            0.0
        } else {
            f64::from(cmd.ab_stage.min(self.ab_stages)) / f64::from(self.ab_stages)
        }
    }

    /// Flameout, relight and compressor stall.
    fn step_failures(&self, s: &mut EngineState, air: &EngineConditions, fuel_available: bool) {
        if s.running && !fuel_available {
            s.running = false;
            s.compressor_stalled = false;
            s.relight_s = 0.0;
        }
        if !s.running {
            // A windmilling relight needs fuel and enough airflow (or a core still turning).
            if fuel_available
                && (air.tas_mps >= self.windmill_relight_mps || s.core >= self.idle_fraction)
            {
                s.relight_s += self.dt_s;
                if s.relight_s >= RELIGHT_S {
                    s.running = true;
                    s.relight_s = 0.0;
                }
            } else {
                s.relight_s = 0.0;
            }
            return;
        }
        let alpha = air.alpha_rad.abs();
        if !s.compressor_stalled && alpha > self.stall_alpha_rad {
            s.compressor_stalled = true;
            s.relight_s = 0.0;
        }
        if s.compressor_stalled {
            if alpha < COMPRESSOR_RECOVER_ALPHA_RAD {
                s.relight_s += self.dt_s;
                if s.relight_s >= RELIGHT_S {
                    s.compressor_stalled = false;
                    s.relight_s = 0.0;
                }
            } else {
                s.relight_s = 0.0;
            }
        }
    }
}

impl EngineModel for Turbofan {
    fn step(
        &self,
        s: &mut EngineState,
        cmd: EngineCommand,
        air: &EngineConditions,
        fuel_available: bool,
    ) {
        self.step_failures(s, air, fuel_available);
        let core_target = if s.running {
            self.core_target(cmd)
        } else {
            0.0
        };
        let k = if core_target > s.core {
            self.k_up
        } else {
            self.k_down
        };
        s.core += k * (core_target - s.core);

        let ab_wanted = s.running && !s.compressor_stalled && self.ab_target(cmd) > 0.0;
        if ab_wanted {
            if !s.ab_lit {
                s.ab_wait_s += self.dt_s;
                if s.ab_wait_s >= self.ab_lightoff_s {
                    s.ab_lit = true;
                    s.ab_wait_s = 0.0;
                }
            }
        } else {
            s.ab_lit = false;
            s.ab_wait_s = 0.0;
        }
        let ab_target = if s.ab_lit { self.ab_target(cmd) } else { 0.0 };
        let k = if ab_target > s.ab_level {
            self.k_ab_up
        } else {
            self.k_ab_down
        };
        s.ab_level += k * (ab_target - s.ab_level);
    }

    fn thrust_n(&self, s: &EngineState, air: &EngineConditions) -> f64 {
        let sigma = air.rho_kgpm3 / RHO0;
        if sigma.is_nan() || sigma <= 0.0 {
            return 0.0;
        }
        let lapse = dmath::pow(sigma, self.density_exp);
        let dry = self.sl_dry_n * s.core * self.mach_dry.eval(air.mach);
        let ab = (self.sl_ab_n - self.sl_dry_n) * s.ab_level * self.mach_ab.eval(air.mach);
        let stall = if s.compressor_stalled {
            COMPRESSOR_STALL_THRUST
        } else {
            1.0
        };
        (dry + ab) * lapse * self.inlet_efficiency(air.alpha_rad) * stall
    }

    fn fuel_flow_kgps(&self, s: &EngineState, thrust_n: f64, air: &EngineConditions) -> f64 {
        if !s.running || air.t_k.is_nan() || air.t_k <= 0.0 {
            return 0.0;
        }
        let tsfc = self.tsfc_dry + (self.tsfc_ab - self.tsfc_dry) * s.ab_level;
        tsfc * thrust_n.max(0.0) * dmath::sqrt(air.t_k / T0)
    }

    fn steady(&self, cmd: EngineCommand) -> EngineState {
        let ab_level = self.ab_target(cmd);
        EngineState {
            ab_level,
            ab_lit: ab_level > 0.0,
            ..EngineState::running_at(self.core_target(cmd))
        }
    }

    fn ab_stage(&self, s: &EngineState, cmd: EngineCommand) -> u8 {
        if s.ab_lit {
            cmd.ab_stage.min(self.ab_stages)
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use content::{Curve, EngineKind};

    const DT: f64 = 1.0 / 120.0;

    /// The F129 (content/core/engines/f129.json, GDD C.3).
    pub(crate) fn f129_def() -> EngineDef {
        let mach = Curve::from_points(&[
            (0.0, 1.0),
            (0.5, 0.98),
            (0.9, 1.02),
            (1.2, 1.12),
            (1.6, 1.2),
            (2.0, 1.08),
            (2.4, 0.85),
        ]);
        EngineDef {
            name_key: "eng.f129.name".into(),
            kind: EngineKind::Turbofan,
            sl_dry_n: 79000.0,
            sl_ab_n: 128_000.0,
            density_exp: 0.75,
            mach_factor_dry: mach.clone(),
            mach_factor_ab: mach,
            idle_fraction: 0.06,
            tsfc_dry_kg_per_ns: 1.983e-5,
            tsfc_ab_kg_per_ns: 5.382e-5,
            spool_up_s: 1.8,
            spool_down_s: 1.2,
            ab_lightoff_s: 0.6,
            ab_stage_s: 0.5,
            ab_cutoff_s: 0.8,
            ab_stages: 4,
            inlet_alpha_start_deg: 15.0,
            inlet_alpha_span_deg: 25.0,
            compressor_stall_alpha_deg: 45.0,
            windmill_relight_mps: 120.0,
        }
    }

    fn f129() -> Turbofan {
        Turbofan::from_def(&f129_def(), DT).unwrap()
    }

    fn sea_level_static() -> EngineConditions {
        EngineConditions {
            rho_kgpm3: RHO0,
            t_k: T0,
            mach: 0.0,
            alpha_rad: 0.0,
            tas_mps: 0.0,
        }
    }

    fn run(
        e: &Turbofan,
        s: &mut EngineState,
        cmd: EngineCommand,
        air: &EngineConditions,
        secs: f64,
    ) {
        for _ in 0..(secs / DT).round() as usize {
            e.step(s, cmd, air, true);
        }
    }

    #[test]
    fn sea_level_static_thrust_and_fuel_flow_match_c3() {
        // C.3 / C.9: 79 kN dry at 1.566 kg/s, 128 kN with afterburner at 6.889 kg/s (3 %).
        let e = f129();
        let air = sea_level_static();
        let mil = e.steady(EngineCommand::MILITARY);
        let t = e.thrust_n(&mil, &air);
        assert!((t - 79_000.0).abs() < 1e-6, "{t}");
        let ff = e.fuel_flow_kgps(&mil, t, &air);
        assert!((ff / 1.566 - 1.0).abs() < 0.03, "{ff}");
        let ab = e.steady(EngineCommand::max_ab(4));
        let t = e.thrust_n(&ab, &air);
        assert!((t - 128_000.0).abs() < 1e-6, "{t}");
        let ff = e.fuel_flow_kgps(&ab, t, &air);
        assert!((ff / 6.889 - 1.0).abs() < 0.03, "{ff}");
        // From idle on the bench, both have settled after 15 s (the ADR-31 procedure).
        for (cmd, target) in [
            (EngineCommand::MILITARY, 79_000.0),
            (EngineCommand::max_ab(4), 128_000.0),
        ] {
            let mut s = e.steady(EngineCommand::IDLE);
            run(&e, &mut s, cmd, &air, 15.0);
            let t = e.thrust_n(&s, &air);
            assert!((t / target - 1.0).abs() < 0.001, "{t} vs {target}");
        }
    }

    #[test]
    fn the_core_pack_engine_is_the_one_tested_here() {
        assert_eq!(crate::test_content::kestrel().engine, f129_def());
    }

    #[test]
    fn idle_and_the_lapse_with_density_and_mach() {
        let e = f129();
        let idle = e.steady(EngineCommand::IDLE);
        assert!((e.thrust_n(&idle, &sea_level_static()) / (0.06 * 79_000.0) - 1.0).abs() < 1e-7);
        // 11 km: rho 0.3639, (0.3639 / 1.225)^0.75 = 0.4016; Mach 1.2 factor 1.12.
        let air = EngineConditions {
            rho_kgpm3: 0.3639,
            t_k: 216.65,
            mach: 1.2,
            alpha_rad: 0.0,
            tas_mps: 354.0,
        };
        let mil = e.steady(EngineCommand::MILITARY);
        let expected = 79_000.0 * dmath::pow(0.3639 / RHO0, 0.75) * 1.12;
        assert!(
            (e.thrust_n(&mil, &air) / expected - 1.0).abs() < 1e-7,
            "f32 data"
        );
        // Fuel flow falls with sqrt(T / T0) at the same thrust.
        let ff = e.fuel_flow_kgps(&mil, 10_000.0, &air);
        assert!((ff - f64::from(1.983e-5_f32) * 10_000.0 * dmath::sqrt(216.65 / T0)).abs() < 1e-12);
        let vacuum = EngineConditions {
            rho_kgpm3: 0.0,
            ..air
        };
        assert_eq!(e.thrust_n(&mil, &vacuum), 0.0);
    }

    #[test]
    fn the_spool_reaches_63_percent_in_one_time_constant() {
        let e = f129();
        let air = sea_level_static();
        let mut s = e.steady(EngineCommand::IDLE);
        run(&e, &mut s, EngineCommand::MILITARY, &air, 1.8);
        let frac = (s.core - 0.06) / (1.0 - 0.06);
        assert!((frac - 0.632).abs() < 0.632 * 0.02, "{frac}");
        let mut s = e.steady(EngineCommand::MILITARY);
        run(&e, &mut s, EngineCommand::IDLE, &air, 1.2);
        let frac = (1.0 - s.core) / (1.0 - 0.06);
        assert!((frac - 0.632).abs() < 0.632 * 0.02, "{frac}");
    }

    #[test]
    fn the_afterburner_waits_for_light_off_then_rises() {
        let e = f129();
        let air = sea_level_static();
        let mut s = e.steady(EngineCommand::MILITARY);
        let cmd = EngineCommand::max_ab(4);
        run(&e, &mut s, cmd, &air, 0.55);
        assert!(!s.ab_lit && s.ab_level == 0.0, "still waiting at 0.55 s");
        assert_eq!(e.ab_stage(&s, cmd), 0);
        run(&e, &mut s, cmd, &air, 0.1);
        assert!(s.ab_lit, "lit by 0.65 s");
        assert_eq!(e.ab_stage(&s, cmd), 4);
        run(&e, &mut s, cmd, &air, 3.0);
        assert!(s.ab_level > 0.99);
        // Back to military: the afterburner goes out and runs down with its own time constant.
        run(&e, &mut s, EngineCommand::MILITARY, &air, 0.8);
        assert!(!s.ab_lit);
        assert!(
            (s.ab_level - dmath::exp(-1.0)).abs() < 0.02,
            "{}",
            s.ab_level
        );
    }

    #[test]
    fn the_detent_is_military_in_both_precisions() {
        let at = |t: f64| EngineCommand::from_lever(t, 4);
        assert_eq!(at(0.0), EngineCommand::IDLE);
        assert_eq!(
            at(0.4),
            EngineCommand {
                dry: 0.5,
                ab_stage: 0
            }
        );
        assert_eq!(at(0.8), EngineCommand::MILITARY, "the f64 detent");
        assert_eq!(
            at(f64::from(0.8_f32)),
            EngineCommand::MILITARY,
            "the f32 detent"
        );
        assert_eq!(at(0.80009), EngineCommand::MILITARY, "inside the guard");
        assert_eq!(at(0.801).ab_stage, 1);
        assert_eq!(at(0.85).ab_stage, 1, "an edge belongs to the lower stage");
        assert_eq!(at(0.851).ab_stage, 2);
        assert_eq!(at(0.95).ab_stage, 3);
        assert_eq!(at(1.0), EngineCommand::max_ab(4));
        assert_eq!(at(7.0), EngineCommand::max_ab(4));
        assert_eq!(at(f64::NAN), EngineCommand::IDLE);
        assert_eq!(
            EngineCommand::from_lever(1.0, 0),
            EngineCommand::MILITARY,
            "no afterburner"
        );
    }

    #[test]
    fn inlet_loss_follows_the_data() {
        let e = f129();
        assert_eq!(e.inlet_efficiency(0.2), 1.0, "11.5 deg: no loss");
        assert_eq!(e.inlet_efficiency(-15f64.to_radians()), 1.0);
        // 27.5 deg: s = 0.5, loss 0.125.
        assert!((e.inlet_efficiency(27.5f64.to_radians()) - 0.875).abs() < 1e-12);
        assert!((e.inlet_efficiency(-60f64.to_radians()) - 0.5).abs() < 1e-12);
    }

    #[test]
    fn running_dry_flames_out_and_fuel_brings_it_back() {
        let e = f129();
        let air = EngineConditions {
            tas_mps: 200.0,
            ..sea_level_static()
        };
        let mut s = e.steady(EngineCommand::MILITARY);
        e.step(&mut s, EngineCommand::MILITARY, &air, false);
        assert!(!s.running);
        assert_eq!(e.fuel_flow_kgps(&s, 1000.0, &air), 0.0);
        for _ in 0..(1.2 / DT) as usize {
            e.step(&mut s, EngineCommand::MILITARY, &air, false);
        }
        assert!(
            (s.core - dmath::exp(-1.0)).abs() < 0.02,
            "spools down with 1.2 s"
        );
        // Fuel again at 200 m/s: relight after 3 s, then spool up.
        run(&e, &mut s, EngineCommand::MILITARY, &air, 2.9);
        assert!(!s.running);
        run(&e, &mut s, EngineCommand::MILITARY, &air, 0.2);
        assert!(s.running);
        // Too slow and spooled down: no relight.
        let slow = EngineConditions {
            tas_mps: 50.0,
            ..air
        };
        let mut s = EngineState {
            core: 0.0,
            running: false,
            ..EngineState::running_at(0.0)
        };
        run(&e, &mut s, EngineCommand::MILITARY, &slow, 10.0);
        assert!(!s.running);
    }

    #[test]
    fn a_compressor_stall_cuts_thrust_until_alpha_recovers() {
        let e = f129();
        let mut air = EngineConditions {
            alpha_rad: 50f64.to_radians(),
            tas_mps: 60.0,
            ..sea_level_static()
        };
        let mut s = e.steady(EngineCommand::max_ab(4));
        let before = e.thrust_n(&s, &air);
        e.step(&mut s, EngineCommand::max_ab(4), &air, true);
        assert!(
            s.compressor_stalled && !s.ab_lit,
            "stall blows out the afterburner"
        );
        let during = e.thrust_n(&EngineState { ab_level: 1.0, ..s }, &air);
        assert!((during / before - COMPRESSOR_STALL_THRUST).abs() < 1e-3);
        air.alpha_rad = 35f64.to_radians();
        run(&e, &mut s, EngineCommand::MILITARY, &air, 5.0);
        assert!(s.compressor_stalled, "no recovery above 30 deg");
        air.alpha_rad = 10f64.to_radians();
        run(&e, &mut s, EngineCommand::MILITARY, &air, 2.9);
        assert!(s.compressor_stalled);
        run(&e, &mut s, EngineCommand::MILITARY, &air, 0.2);
        assert!(!s.compressor_stalled, "recovered after 3 s below 30 deg");
    }
}
