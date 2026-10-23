//! G-forces and pilot physiology (GDD 11.14, ADR-38).
//!
//! ```text
//! G_f' = (n_z - G_f) / tau_f                         tau_f = 2 s, the blood-pressure lag
//! G_tol = G_base + G_suit + G_agsm - 0.4 F           4.5 + 1.5 + 0 - 0.4 F
//! S' = k_g (G_f - G_tol) / G_tol                     G_f > G_tol, k_g = 0.25 /s
//! S' = -S / tau_rec                                  otherwise, tau_rec = 4 s
//! F' = +0.01 /s (n_z > 5), -0.005 /s otherwise
//! R' = (-1 - n_z) / tau_r                            n_z < -1, tau_r = 3 s (red-out)
//! ```
//!
//! The state is stepped once per tick with `n_z` held over it. `G_f` uses the exact first-order
//! update, and `S` integrates the mean `G_f` of the tick (also exact for a held `n_z`), so the
//! result does not depend on the step size beyond the one tick at which a threshold is crossed.
//!
//! `n_z` is the accelerometer value at the CG; the pilot station's rotational terms are a
//! recorded simplification (ADR-38).
//!
//! What the stress does depends on the player's dial ([`GEffects`], GDD 57, 59):
//!
//! | Dial | Tolerance | Visual | Control |
//! |---|---|---|---|
//! | Full | `G_tol` | grey, black, red | blackout delays the controls 300 ms; G-LOC zeroes them for 6 s, then a 4 s ramp back |
//! | Reduced | `G_tol` + 1.5 g | grey, black, red | none |
//! | Off | `G_tol` | none | none |
//!
//! In every mode the state advances, so switching the dial on does not start from rest.

use simmath::dmath;

/// Longest blackout input delay the effects ask for, in ticks (0.3 s at 60 Hz). The caller's delay
/// ring has this many past entries.
pub const MAX_DELAY_TICKS: usize = 18;

/// `S` never grows past this. // ALGO: keeps the recovery time after a long pull bounded
const STRESS_CAP: f64 = 2.0;

/// The G-model constants. // DATA: GDD 11.14 design targets (not aircraft data)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysioParams {
    /// Lag of the heart-level G, s.
    pub tau_f_s: f64,
    /// Unaided tolerance, g.
    pub g_base: f64,
    /// Added by the anti-G suit, g.
    pub g_suit: f64,
    /// Added by the straining manoeuvre (a player setting, 0 to 1.5), g.
    pub g_agsm: f64,
    /// Tolerance lost per unit of fatigue, g.
    pub fatigue_g: f64,
    /// Stress growth, 1/s.
    pub k_g_per_s: f64,
    /// Stress recovery time constant, s.
    pub tau_rec_s: f64,
    /// Fatigue gained per second while `n_z` is above [`PhysioParams::fatigue_nz`], 1/s.
    pub fatigue_up_per_s: f64,
    /// Fatigue lost per second otherwise, 1/s.
    pub fatigue_down_per_s: f64,
    /// Load factor above which fatigue builds, g.
    pub fatigue_nz: f64,
    /// Red-out time constant, s.
    pub tau_r_s: f64,
    /// Stress at which the grey-out starts.
    pub grey_start: f64,
    /// Stress at which the blackout starts.
    pub blackout: f64,
    /// Stress at which G-LOC starts.
    pub gloc: f64,
    /// Input delay during a blackout, s.
    pub blackout_delay_s: f64,
    /// Controls are zeroed this long after G-LOC starts, s.
    pub gloc_zero_s: f64,
    /// Then they come back linearly over this long, s.
    pub gloc_ramp_s: f64,
    /// Extra tolerance of the Reduced dial, g.
    pub reduced_bonus_g: f64,
}

impl PhysioParams {
    /// The GDD 11.14 values.
    pub const GDD: Self = Self {
        tau_f_s: 2.0,
        g_base: 4.5,
        g_suit: 1.5,
        g_agsm: 0.0,
        fatigue_g: 0.4,
        k_g_per_s: 0.25,
        tau_rec_s: 4.0,
        fatigue_up_per_s: 0.01,
        fatigue_down_per_s: 0.005,
        fatigue_nz: 5.0,
        tau_r_s: 3.0,
        grey_start: 0.35,
        blackout: 0.7,
        gloc: 1.0,
        blackout_delay_s: 0.3,
        gloc_zero_s: 6.0,
        gloc_ramp_s: 4.0,
        reduced_bonus_g: 1.5,
    };

    /// The tolerance for a fatigue and a dial, g.
    #[must_use]
    pub fn tolerance_g(&self, fatigue: f64, effects: GEffects) -> f64 {
        let bonus = if effects == GEffects::Reduced {
            self.reduced_bonus_g
        } else {
            0.0
        };
        (self.g_base + self.g_suit + self.g_agsm + bonus - self.fatigue_g * fatigue).max(1.0)
    }
}

impl Default for PhysioParams {
    fn default() -> Self {
        Self::GDD
    }
}

/// How much of the physiology reaches the player (GDD 57, 59).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GEffects {
    /// Computed and shown to nobody.
    Off,
    /// Visual effects only, with a higher tolerance.
    Reduced,
    /// The full model: visuals, delayed controls in a blackout, G-LOC.
    #[default]
    Full,
}

/// The pilot's physiological state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysioState {
    /// Lagged heart-level G, g.
    pub g_f: f64,
    /// Stress `S` (0 fine, 0.7 blackout, 1.0 G-LOC).
    pub stress: f64,
    /// Fatigue `F`, 0 to 1.
    pub fatigue: f64,
    /// Red-out `R` (0.5 tint, 1.0 vision loss).
    pub redout: f64,
    /// Seconds of G-LOC left (controls zeroed).
    pub gloc_s: f64,
    /// Seconds of disorientation left (controls ramping back); runs once `gloc_s` is spent.
    pub disorient_s: f64,
}

impl PhysioState {
    /// A rested pilot at 1 g.
    pub const REST: Self = Self {
        g_f: 1.0,
        stress: 0.0,
        fatigue: 0.0,
        redout: 0.0,
        gloc_s: 0.0,
        disorient_s: 0.0,
    };
}

impl Default for PhysioState {
    fn default() -> Self {
        Self::REST
    }
}

/// What the pilot's state does to the aircraft and the picture this tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysioEffects {
    /// The stress is in the blackout band (a visual flag in Reduced, too).
    pub blackout: bool,
    /// The pilot is out (controls zeroed) or still ramping back.
    pub gloc: bool,
    /// G-LOC started this tick.
    pub gloc_started: bool,
    /// Multiplier on the pilot's controls: 1 normally, 0 in G-LOC, rising to 1 over the ramp.
    pub control_scale: f64,
    /// Ticks the controls are delayed (the blackout), 0 to [`MAX_DELAY_TICKS`].
    pub delay_ticks: usize,
    /// Grey-out and tunnel vision, 0 to 1.
    pub grey: f64,
    /// Black screen, 0 to 1.
    pub black: f64,
    /// Red tint and, at 1, vision loss.
    pub red: f64,
}

impl PhysioEffects {
    /// Nothing applied.
    pub const NONE: Self = Self {
        blackout: false,
        gloc: false,
        gloc_started: false,
        control_scale: 1.0,
        delay_ticks: 0,
        grey: 0.0,
        black: 0.0,
        red: 0.0,
    };
}

fn ramp(x: f64, lo: f64, hi: f64) -> f64 {
    ((x - lo) / (hi - lo)).clamp(0.0, 1.0)
}

impl PhysioState {
    /// Advances the state by `dt_s` with the load factor `nz_g` held over it, and returns what it
    /// does. A non-finite `nz_g` counts as 1 g.
    pub fn step(
        &mut self,
        nz_g: f64,
        dt_s: f64,
        params: &PhysioParams,
        effects: GEffects,
    ) -> PhysioEffects {
        let nz = if nz_g.is_finite() { nz_g } else { 1.0 };
        // Heart-level G: the exact update, and its mean over the tick.
        let g_old = self.g_f;
        let g_new = nz + (g_old - nz) * dmath::exp(-dt_s / params.tau_f_s);
        let g_mean = nz + params.tau_f_s * (g_old - g_new) / dt_s;
        self.g_f = g_new;

        let tol = params.tolerance_g(self.fatigue, effects);
        if g_mean > tol {
            self.stress += params.k_g_per_s * (g_mean - tol) / tol * dt_s;
        } else {
            self.stress *= dmath::exp(-dt_s / params.tau_rec_s);
        }
        self.stress = self.stress.clamp(0.0, STRESS_CAP);

        let rate = if nz > params.fatigue_nz {
            params.fatigue_up_per_s
        } else {
            -params.fatigue_down_per_s
        };
        self.fatigue = (self.fatigue + rate * dt_s).clamp(0.0, 1.0);

        if nz < -1.0 {
            self.redout += (-1.0 - nz) / params.tau_r_s * dt_s;
        } else {
            self.redout *= dmath::exp(-dt_s / params.tau_r_s);
        }
        self.redout = self.redout.clamp(0.0, 1.0);

        self.effects(dt_s, params, effects)
    }

    /// The G-LOC timers and the effects for the state after a step.
    fn effects(&mut self, dt_s: f64, params: &PhysioParams, dial: GEffects) -> PhysioEffects {
        if dial == GEffects::Off {
            self.gloc_s = 0.0;
            self.disorient_s = 0.0;
            return PhysioEffects::NONE;
        }
        let mut out = PhysioEffects::NONE;
        let s = self.stress;
        out.grey = ramp(s, params.grey_start, params.blackout);
        out.black = ramp(s, params.blackout - 0.1, params.blackout);
        out.red = self.redout;
        out.blackout = s >= params.blackout;
        if dial == GEffects::Reduced {
            self.gloc_s = 0.0;
            self.disorient_s = 0.0;
            return out;
        }

        // Full: the controls are affected.
        if self.gloc_s <= 0.0 && self.disorient_s <= 0.0 && s >= params.gloc {
            self.gloc_s = params.gloc_zero_s;
            self.disorient_s = params.gloc_ramp_s;
            out.gloc_started = true;
        } else if self.gloc_s > 0.0 {
            self.gloc_s = (self.gloc_s - dt_s).max(0.0);
        } else if self.disorient_s > 0.0 {
            self.disorient_s = (self.disorient_s - dt_s).max(0.0);
        }
        if self.gloc_s > 0.0 {
            out.gloc = true;
            out.control_scale = 0.0;
            out.black = 1.0;
        } else if self.disorient_s > 0.0 {
            out.gloc = true;
            out.control_scale = 1.0 - self.disorient_s / params.gloc_ramp_s;
        } else if out.blackout {
            let ticks = (params.blackout_delay_s / dt_s + 0.5).floor();
            out.delay_ticks = (ticks as usize).min(MAX_DELAY_TICKS);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f64 = 1.0 / 60.0;
    const FULL: GEffects = GEffects::Full;

    /// The GDD constants with fatigue frozen at zero: the AC-11.9 conditions.
    fn no_fatigue() -> PhysioParams {
        PhysioParams {
            fatigue_up_per_s: 0.0,
            ..PhysioParams::GDD
        }
    }

    /// Seconds of a constant `nz` until the stress reaches `level`, or `None` within 60 s.
    fn time_to(level: f64, nz: f64, params: &PhysioParams) -> Option<f64> {
        let mut p = PhysioState::REST;
        for i in 1..=(60 * 60) {
            p.step(nz, DT, params, FULL);
            if p.stress >= level {
                return Some(f64::from(i) * DT);
            }
        }
        None
    }

    #[test]
    fn ac_11_9_a_sustained_nine_g_blacks_out_at_9_6_and_g_loc_at_12_0() {
        // Hand integration of GDD 11.14: G_f = 9 - 8 exp(-t/2) passes G_tol = 6 at 1.96 s, and
        // S = 0.25/6 (3 (t - 1.96) + 16 (exp(-t/2) - exp(-0.98))) reaches 0.7 at 9.50 s and 1.0
        // at 11.93 s. The acceptance bands are 9.6 +- 0.5 and 12.0 +- 0.5.
        let p = no_fatigue();
        let black = time_to(0.7, 9.0, &p).unwrap();
        let gloc = time_to(1.0, 9.0, &p).unwrap();
        assert!((black - 9.50).abs() < 0.03, "blackout at {black}");
        assert!((gloc - 11.93).abs() < 0.03, "G-LOC at {gloc}");
        assert!((black - 9.6).abs() <= 0.5 && (gloc - 12.0).abs() <= 0.5);
    }

    #[test]
    fn with_fatigue_building_the_times_are_a_little_shorter_and_still_in_band() {
        let p = PhysioParams::GDD;
        let black = time_to(0.7, 9.0, &p).unwrap();
        let gloc = time_to(1.0, 9.0, &p).unwrap();
        assert!((black - 9.42).abs() < 0.05, "blackout at {black}");
        assert!((gloc - 11.80).abs() < 0.05, "G-LOC at {gloc}");
        assert!((black - 9.6).abs() <= 0.5 && (gloc - 12.0).abs() <= 0.5);
    }

    #[test]
    fn ac_11_9_a_three_second_nine_g_pull_barely_registers() {
        for p in [no_fatigue(), PhysioParams::GDD] {
            let mut s = PhysioState::REST;
            let mut peak = 0.0_f64;
            let mut any_blackout = false;
            for i in 0..(10 * 60) {
                let nz = if i < 3 * 60 { 9.0 } else { 1.0 };
                let e = s.step(nz, DT, &p, FULL);
                peak = peak.max(s.stress);
                any_blackout |= e.blackout;
            }
            assert!(peak < 0.05, "peak {peak}");
            assert!(peak > 0.03, "but not nothing: {peak}");
            assert!(!any_blackout);
        }
    }

    #[test]
    fn the_heart_level_g_lags_with_the_exact_first_order_update() {
        // A step from 1 to 9 g: one time constant gets 1 - 1/e of the way, whatever the step.
        for dt in [1.0_f64 / 60.0, 1.0 / 600.0, 0.5] {
            let mut p = PhysioState::REST;
            let n = (2.0 / dt).round() as usize;
            for _ in 0..n {
                p.step(9.0, dt, &PhysioParams::GDD, FULL);
            }
            let expected = 9.0 - 8.0 * dmath::exp(-1.0);
            assert!((p.g_f - expected).abs() < 1e-9, "{dt}: {}", p.g_f);
        }
    }

    #[test]
    fn the_stress_is_indifferent_to_the_step_size() {
        let run = |dt: f64| {
            let mut p = PhysioState::REST;
            let n = (8.0 / dt).round() as usize;
            for _ in 0..n {
                p.step(9.0, dt, &no_fatigue(), FULL);
            }
            p.stress
        };
        let (fine, coarse) = (run(1.0 / 600.0), run(1.0 / 60.0));
        assert!((fine - coarse).abs() < 5e-3, "{fine} vs {coarse}");
    }

    #[test]
    fn the_stress_recovers_with_a_four_second_time_constant() {
        let mut p = PhysioState {
            stress: 0.5,
            g_f: 1.0,
            ..PhysioState::REST
        };
        for _ in 0..(4 * 60) {
            p.step(1.0, DT, &PhysioParams::GDD, FULL);
        }
        let expected = 0.5 * dmath::exp(-1.0);
        assert!((p.stress - expected).abs() < 1e-6, "{}", p.stress);
    }

    #[test]
    fn below_the_tolerance_nothing_builds() {
        let mut p = PhysioState::REST;
        for _ in 0..(120 * 60) {
            p.step(5.5, DT, &no_fatigue(), FULL);
        }
        assert_eq!(p.stress, 0.0);
        // And at one g an unrested pilot stays rested.
        let mut q = PhysioState::REST;
        for _ in 0..600 {
            q.step(1.0, DT, &PhysioParams::GDD, FULL);
        }
        assert_eq!(q, PhysioState::REST);
    }

    #[test]
    fn fatigue_builds_above_five_g_falls_otherwise_and_lowers_the_tolerance() {
        let p = PhysioParams::GDD;
        let mut s = PhysioState::REST;
        for _ in 0..(10 * 60) {
            s.step(6.0, DT, &p, FULL);
        }
        assert!((s.fatigue - 0.1).abs() < 1e-9, "{}", s.fatigue);
        assert!((p.tolerance_g(s.fatigue, FULL) - 5.96).abs() < 1e-12);
        for _ in 0..(10 * 60) {
            s.step(1.0, DT, &p, FULL);
        }
        assert!((s.fatigue - 0.05).abs() < 1e-9, "{}", s.fatigue);
        for _ in 0..(60 * 60) {
            s.step(1.0, DT, &p, FULL);
        }
        assert_eq!(s.fatigue, 0.0, "clamped at zero");
        for _ in 0..(200 * 60) {
            s.step(6.0, DT, &p, FULL);
        }
        assert_eq!(s.fatigue, 1.0, "clamped at one");
    }

    #[test]
    fn negative_g_builds_red_out_with_the_gdd_threshold_times() {
        let p = PhysioParams::GDD;
        let mut s = PhysioState::REST;
        let mut tint = None;
        let mut loss = None;
        for i in 1..=(5 * 60) {
            let e = s.step(-3.0, DT, &p, FULL);
            if tint.is_none() && s.redout >= 0.5 {
                tint = Some(f64::from(i) * DT);
                assert!(e.red >= 0.5);
            }
            if loss.is_none() && s.redout >= 1.0 {
                loss = Some(f64::from(i) * DT);
            }
        }
        // dR/dt = (-1 - (-3)) / 3 = 2/3 per second.
        assert!((tint.unwrap() - 0.75).abs() < 0.02, "{tint:?}");
        assert!((loss.unwrap() - 1.5).abs() < 0.02, "{loss:?}");
        // Mild negative G (down to -1) builds nothing.
        let mut m = PhysioState::REST;
        for _ in 0..600 {
            m.step(-1.0, DT, &p, FULL);
        }
        assert_eq!(m.redout, 0.0);
        // And it recovers once the load is gone.
        for _ in 0..(10 * 60) {
            s.step(1.0, DT, &p, FULL);
        }
        assert!(s.redout < 0.05, "{}", s.redout);
    }

    #[test]
    fn the_visual_bands_follow_the_gdd_table() {
        let p = PhysioParams::GDD;
        let mut s = PhysioState::REST;
        s.stress = 0.2;
        assert_eq!(s.effects(DT, &p, FULL).grey, 0.0);
        s.stress = 0.35;
        assert_eq!(s.effects(DT, &p, FULL).grey, 0.0);
        s.stress = 0.525;
        let e = s.effects(DT, &p, FULL);
        assert!((e.grey - 0.5).abs() < 1e-12 && e.black == 0.0 && !e.blackout);
        s.stress = 0.7;
        let e = s.effects(DT, &p, FULL);
        assert_eq!((e.grey, e.black), (1.0, 1.0));
        assert!(e.blackout);
    }

    #[test]
    fn a_blackout_delays_the_controls_by_300_ms_and_g_loc_zeroes_them() {
        let p = PhysioParams::GDD;
        let mut s = PhysioState::REST;
        s.stress = 0.75;
        let e = s.effects(DT, &p, FULL);
        assert_eq!(e.delay_ticks, 18);
        assert_eq!(e.control_scale, 1.0);
        assert!(e.blackout && !e.gloc);
        assert_eq!(MAX_DELAY_TICKS, 18);

        // Cross 1.0: G-LOC for 6 s, then a 4 s ramp back.
        s.stress = 1.0;
        let e = s.effects(DT, &p, FULL);
        assert!(e.gloc && e.gloc_started);
        assert_eq!((e.control_scale, e.delay_ticks, e.black), (0.0, 0, 1.0));
        let mut zero_s = 0.0;
        let mut ramp_s = 0.0;
        let mut last = 0.0;
        for _ in 0..(12 * 60) {
            // Hold the stress above 1 to prove a second trigger cannot happen mid-event.
            s.stress = 1.2;
            let e = s.effects(DT, &p, FULL);
            assert!(!e.gloc_started);
            if e.control_scale == 0.0 {
                zero_s += DT;
            } else if e.gloc {
                ramp_s += DT;
                assert!(e.control_scale >= last, "the ramp only rises");
                last = e.control_scale;
            } else {
                break;
            }
        }
        // The trigger tick was one of the six seconds.
        assert!((zero_s - 6.0).abs() < 2.0 * DT, "{zero_s}");
        assert!((ramp_s - 4.0).abs() < 2.0 * DT, "{ramp_s}");
        assert!(last > 0.98);
    }

    #[test]
    fn a_pilot_who_stays_under_load_goes_out_again() {
        let p = no_fatigue();
        let mut s = PhysioState::REST;
        let mut events = 0;
        for _ in 0..(40 * 60) {
            if s.step(9.0, DT, &p, FULL).gloc_started {
                events += 1;
            }
        }
        assert!(events >= 2, "{events}");
    }

    #[test]
    fn reduced_adds_tolerance_and_never_touches_the_controls() {
        let p = no_fatigue();
        let mut s = PhysioState::REST;
        let mut grey = 0.0_f64;
        for _ in 0..(60 * 60) {
            let e = s.step(9.0, DT, &p, GEffects::Reduced);
            assert_eq!((e.control_scale, e.delay_ticks, e.gloc), (1.0, 0, false));
            grey = grey.max(e.grey);
        }
        assert!(grey > 0.0, "the picture still greys out");
        // G_tol 7.5: dS/dt = 0.25 (9 - 7.5) / 7.5 = 0.05 /s at the plateau, so S crosses G-LOC
        // far later than in Full, but it does cross and the visuals follow it.
        assert!(
            time_to_dial(0.7, 9.0, &p, GEffects::Reduced).unwrap()
                > time_to(0.7, 9.0, &p).unwrap() + 5.0
        );
        assert_eq!(PhysioParams::GDD.tolerance_g(0.0, GEffects::Reduced), 7.5);
        assert_eq!(PhysioParams::GDD.tolerance_g(0.0, FULL), 6.0);
    }

    fn time_to_dial(level: f64, nz: f64, params: &PhysioParams, dial: GEffects) -> Option<f64> {
        let mut p = PhysioState::REST;
        for i in 1..=(600 * 60) {
            p.step(nz, DT, params, dial);
            if p.stress >= level {
                return Some(f64::from(i) * DT);
            }
        }
        None
    }

    #[test]
    fn off_computes_the_state_and_applies_nothing() {
        let p = no_fatigue();
        let mut s = PhysioState::REST;
        for _ in 0..(20 * 60) {
            let e = s.step(9.0, DT, &p, GEffects::Off);
            assert_eq!(e, PhysioEffects::NONE);
        }
        assert!(s.stress > 1.0, "the state advanced: {}", s.stress);
        // Switching it on does not start from rest: the pilot is out straight away.
        let e = s.step(9.0, DT, &p, FULL);
        assert!(e.gloc && e.gloc_started);
    }

    #[test]
    fn leaving_full_clears_a_g_loc_in_progress() {
        let p = PhysioParams::GDD;
        let mut s = PhysioState::REST;
        s.stress = 1.0;
        assert!(s.effects(DT, &p, FULL).gloc);
        let e = s.effects(DT, &p, GEffects::Reduced);
        assert!(!e.gloc);
        assert_eq!(e.control_scale, 1.0);
        assert_eq!((s.gloc_s, s.disorient_s), (0.0, 0.0));
    }

    #[test]
    fn a_bad_load_factor_counts_as_one_g_and_the_state_stays_finite() {
        let mut s = PhysioState::REST;
        for nz in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            s.step(nz, DT, &PhysioParams::GDD, FULL);
        }
        assert_eq!(s, PhysioState::REST);
        // And the same history gives the same state (determinism of the pure step).
        let run = || {
            let mut p = PhysioState::REST;
            for i in 0..2000 {
                p.step(
                    1.0 + 8.0 * dmath::sin(f64::from(i) * 0.01).abs(),
                    DT,
                    &PhysioParams::GDD,
                    FULL,
                );
            }
            p
        };
        assert_eq!(run(), run());
    }
}
