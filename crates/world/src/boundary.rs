//! The boundary's four stages (GDD 8.5, ADR-53), stepped once per tick.
//!
//! - **Outside:** the SDF is positive, or the aircraft is above the ceiling (GDD 8.4).
//! - **Advisory:** inside and within the warning band (`sdf ≥ −W`).
//! - **The countdown:**
//!   - It starts on the tick that time outside first exceeds 3 s, at `countdown_s`.
//!   - It runs down only while outside, and holds while inside.
//!   - It is reset once the aircraft has been inside for 5 s; time outside resets with it.
//!   - Leaving again before then resumes the same countdown.
//! - **When it runs out:**
//!   - `mission_fail` and `deserter` lose the aircraft (`Lost`);
//!   - `assist_turn` engages the turn-back pilot until the reset.
//!   - `Enforced` latches until the respawn clears the state, or for `assist_turn` until the reset.

/// The enforcement policy (GDD 8.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Policy {
    /// Easy: an autopilot turns back until the reset.
    AssistTurn = 0,
    /// Normal and Hard: the aircraft is lost, "Out of Area".
    MissionFail = 1,
    /// Simulator: lost, and counted as a loss in a campaign.
    Deserter = 2,
}

impl Policy {
    /// The policy with this code.
    #[must_use]
    pub const fn from_code(code: u32) -> Option<Self> {
        match code {
            0 => Some(Self::AssistTurn),
            1 => Some(Self::MissionFail),
            2 => Some(Self::Deserter),
            _ => None,
        }
    }
}

/// A boundary stage (GDD 8.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Stage {
    /// Well inside.
    #[default]
    Inside = 0,
    /// 1, inside the warning band.
    Advisory = 1,
    /// 2, outside.
    Restricted = 2,
    /// 3, the countdown runs (or holds).
    Countdown = 3,
    /// 4, the countdown ran out.
    Enforced = 4,
}

/// The boundary's parameters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoundaryParams {
    /// The warning band `W`, m.
    pub warn_width_m: f64,
    /// The countdown, s.
    pub countdown_s: f64,
    /// The hard ceiling, m MSL (GDD 8.4).
    pub ceiling_m: f64,
    /// What happens when the countdown runs out.
    pub policy: Policy,
}

/// What a step did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoundaryEvent {
    /// Nothing new.
    None,
    /// The stage changed to this one.
    StageChanged(Stage),
    /// The countdown ran out under `mission_fail` or `deserter`: the aircraft is lost.
    Lost,
}

/// Time outside before the countdown starts, s. // DATA: GDD 8.5
pub const GRACE_S: f64 = 3.0;
/// Time inside that resets the countdown, s. // DATA: GDD 8.5
pub const RESET_S: f64 = 5.0;
/// Slack on the timers' comparisons, s. // ALGO: absorbs the rounding of summed 1/60 s ticks
const SLACK_S: f64 = 1e-9;

/// The boundary's state.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct BoundaryState {
    /// The current stage.
    pub stage: Stage,
    /// Time outside since the last reset, s.
    pub outside_s: f64,
    /// Time inside since the last exit, s.
    pub inside_s: f64,
    /// The countdown's remaining time, s, while one is running or holding.
    pub remaining_s: Option<f64>,
    /// Whether the turn-back pilot flies (`assist_turn`).
    pub assist_active: bool,
    /// Whether the countdown ran out (latched).
    pub enforced: bool,
}

impl BoundaryState {
    /// The state of a fresh spawn.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The current stage.
    #[must_use]
    pub fn stage(&self) -> Stage {
        self.stage
    }

    fn reset_countdown(&mut self) {
        self.remaining_s = None;
        self.outside_s = 0.0;
        self.assist_active = false;
        self.enforced = false;
    }

    /// One tick: the SDF at the aircraft, its altitude MSL and the tick's length.
    pub fn step(
        &mut self,
        p: &BoundaryParams,
        sdf_m: f64,
        alt_msl_m: f64,
        dt: f64,
    ) -> BoundaryEvent {
        let outside = sdf_m > 0.0 || alt_msl_m > p.ceiling_m;
        let advisory = !outside && sdf_m >= -p.warn_width_m;
        let mut lost = false;
        if outside {
            self.outside_s += dt;
            self.inside_s = 0.0;
            match self.remaining_s {
                None if self.outside_s > GRACE_S + SLACK_S => {
                    self.remaining_s = Some(p.countdown_s);
                }
                Some(r) if !self.enforced => {
                    let r = r - dt;
                    self.remaining_s = Some(r);
                    if r <= SLACK_S {
                        self.enforced = true;
                        match p.policy {
                            Policy::AssistTurn => self.assist_active = true,
                            Policy::MissionFail | Policy::Deserter => lost = true,
                        }
                    }
                }
                _ => {}
            }
        } else {
            self.inside_s += dt;
            if self.inside_s >= RESET_S - SLACK_S
                && (self.remaining_s.is_some() || self.outside_s > 0.0)
            {
                self.reset_countdown();
            }
        }
        let stage = if self.enforced {
            Stage::Enforced
        } else if self.remaining_s.is_some() {
            Stage::Countdown
        } else if outside {
            Stage::Restricted
        } else if advisory {
            Stage::Advisory
        } else {
            Stage::Inside
        };
        let changed = stage != self.stage;
        self.stage = stage;
        if lost {
            BoundaryEvent::Lost
        } else if changed {
            BoundaryEvent::StageChanged(stage)
        } else {
            BoundaryEvent::None
        }
    }

    /// The state as six words for the `ownship` hash (world sessions only).
    #[must_use]
    pub fn words(&self) -> [u64; 6] {
        [
            self.stage as u64,
            self.outside_s.to_bits(),
            self.inside_s.to_bits(),
            self.remaining_s.map_or(u64::MAX, f64::to_bits),
            u64::from(self.assist_active),
            u64::from(self.enforced),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f64 = 1.0 / 60.0;

    fn params(policy: Policy) -> BoundaryParams {
        BoundaryParams {
            warn_width_m: 2_000.0,
            countdown_s: 10.0,
            ceiling_m: 30_000.0,
            policy,
        }
    }

    fn run(
        s: &mut BoundaryState,
        p: &BoundaryParams,
        sdf: f64,
        ticks: usize,
    ) -> Vec<BoundaryEvent> {
        (0..ticks)
            .map(|_| s.step(p, sdf, 3_000.0, DT))
            .filter(|e| *e != BoundaryEvent::None)
            .collect()
    }

    #[test]
    fn bf8_the_stages_in_order() {
        let p = params(Policy::MissionFail);
        let mut s = BoundaryState::new();
        assert_eq!(run(&mut s, &p, -5_000.0, 10), vec![]);
        assert_eq!(
            run(&mut s, &p, -1_000.0, 1),
            vec![BoundaryEvent::StageChanged(Stage::Advisory)]
        );
        assert_eq!(
            run(&mut s, &p, 100.0, 1),
            vec![BoundaryEvent::StageChanged(Stage::Restricted)]
        );
        // 3 s outside: the countdown starts on the tick after 180 (outside_s first exceeds 3 s).
        let ev = run(&mut s, &p, 100.0, 180);
        assert_eq!(ev, vec![BoundaryEvent::StageChanged(Stage::Countdown)]);
        assert_eq!(s.remaining_s, Some(10.0));
        // 10 s later, lost.
        let mut n = 0;
        while s.step(&p, 100.0, 3_000.0, DT) != BoundaryEvent::Lost {
            n += 1;
            assert!(n < 1_000);
        }
        assert_eq!(n + 1, 600, "the countdown runs exactly 600 ticks");
        assert_eq!(s.stage(), Stage::Enforced);
    }

    #[test]
    fn bf8_the_countdown_holds_inside_and_resets_after_5_s() {
        let p = params(Policy::MissionFail);
        let mut s = BoundaryState::new();
        run(&mut s, &p, 100.0, 181 + 120); // countdown running for 2 s
        let r = s.remaining_s.unwrap();
        // Back inside for 4 s: it holds.
        run(&mut s, &p, -500.0, 240);
        assert_eq!(s.remaining_s, Some(r));
        assert_eq!(s.stage(), Stage::Countdown);
        // Out again before 5 s: the same countdown resumes.
        run(&mut s, &p, 100.0, 60);
        assert!((s.remaining_s.unwrap() - (r - 1.0)).abs() < 1e-9);
        // Inside for 5 s: reset.
        let ev = run(&mut s, &p, -500.0, 300);
        assert_eq!(s.remaining_s, None);
        assert_eq!(s.outside_s, 0.0);
        assert_eq!(ev, vec![BoundaryEvent::StageChanged(Stage::Advisory)]);
    }

    #[test]
    fn bf8_the_ceiling_counts_as_outside() {
        let p = params(Policy::MissionFail);
        let mut s = BoundaryState::new();
        assert_eq!(
            s.step(&p, -20_000.0, 30_001.0, DT),
            BoundaryEvent::StageChanged(Stage::Restricted)
        );
    }

    #[test]
    fn bf8_assist_turn_engages_and_clears_on_the_reset() {
        let p = params(Policy::AssistTurn);
        let mut s = BoundaryState::new();
        let ev = run(&mut s, &p, 100.0, 181 + 600);
        assert!(!ev.contains(&BoundaryEvent::Lost));
        assert!(s.assist_active && s.enforced);
        assert_eq!(s.stage(), Stage::Enforced);
        run(&mut s, &p, -5_000.0, 299);
        assert!(s.assist_active, "still engaged after 299 ticks inside");
        let ev = run(&mut s, &p, -5_000.0, 1);
        assert_eq!(ev, vec![BoundaryEvent::StageChanged(Stage::Inside)]);
        assert!(!s.assist_active && !s.enforced);
    }

    #[test]
    fn the_words_move_with_the_state() {
        let p = params(Policy::Deserter);
        let mut s = BoundaryState::new();
        let w0 = s.words();
        s.step(&p, 100.0, 3_000.0, DT);
        assert_ne!(s.words(), w0);
        assert_eq!(Policy::from_code(2), Some(Policy::Deserter));
        assert_eq!(Policy::from_code(3), None);
    }
}
