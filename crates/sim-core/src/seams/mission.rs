//! `MissionProvider`: what the player is supposed to be doing (GDD 21, 61.5). The first
//! implementation is a scripted list of objectives, each "wait" or "reach"; generated missions
//! and the mission editor's output replace it in M2.

use crate::math::DVec3;

/// One step of a scripted mission.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Objective {
    /// Complete after this many seconds have passed since the objective became active.
    Wait {
        /// Seconds to wait.
        seconds: f64,
    },
    /// Complete when the player is within `radius_m` of `point`.
    Reach {
        /// Target point, BF.
        point: DVec3,
        /// Completion radius, metres.
        radius_m: f64,
    },
}

/// Where a mission stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissionStatus {
    /// Working on objective `index` (zero-based).
    Active {
        /// Index of the current objective.
        index: usize,
    },
    /// Every objective is done.
    Completed,
    /// The time limit ran out first.
    Failed,
}

/// A mission the director consults once per tick.
pub trait MissionProvider: Send {
    /// Advances the mission to simulation time `t_s` with the player at `player_pos`, and says
    /// where it stands. `t_s` never decreases between calls; once `Completed` or `Failed` the
    /// status never changes again.
    fn update(&mut self, t_s: f64, player_pos: DVec3) -> MissionStatus;

    /// Number of objectives.
    fn objective_count(&self) -> usize;
}

/// A fixed list of objectives done in order, with an optional overall time limit.
#[derive(Clone, Debug)]
pub struct ScriptedMission {
    objectives: Vec<Objective>,
    time_limit_s: Option<f64>,
    current: usize,
    /// Time the current objective became active (the first call to `update` for objective 0).
    active_since_s: Option<f64>,
    started_s: Option<f64>,
    done: Option<MissionStatus>,
}

impl ScriptedMission {
    /// A mission over `objectives`, failing if `time_limit_s` elapses first.
    #[must_use]
    pub fn new(objectives: Vec<Objective>, time_limit_s: Option<f64>) -> Self {
        Self {
            objectives,
            time_limit_s,
            current: 0,
            active_since_s: None,
            started_s: None,
            done: None,
        }
    }
}

impl MissionProvider for ScriptedMission {
    fn update(&mut self, t_s: f64, player_pos: DVec3) -> MissionStatus {
        if let Some(status) = self.done {
            return status;
        }
        if !t_s.is_finite() {
            return MissionStatus::Active {
                index: self.current,
            };
        }
        let started = *self.started_s.get_or_insert(t_s);
        let mut since = *self.active_since_s.get_or_insert(t_s);

        // Objectives may complete back to back in one call (a "reach" the player is already at).
        while let Some(objective) = self.objectives.get(self.current) {
            let complete = match *objective {
                Objective::Wait { seconds } => t_s - since >= seconds.max(0.0),
                Objective::Reach { point, radius_m } => {
                    player_pos.is_finite() && (player_pos - point).length() <= radius_m
                }
            };
            if !complete {
                break;
            }
            // A `Wait` that follows starts counting from now, not from the call before.
            self.current += 1;
            self.active_since_s = Some(t_s);
            since = t_s;
        }

        if self.current >= self.objectives.len() {
            self.done = Some(MissionStatus::Completed);
            return MissionStatus::Completed;
        }
        if self
            .time_limit_s
            .is_some_and(|limit| t_s - started >= limit)
        {
            self.done = Some(MissionStatus::Failed);
            return MissionStatus::Failed;
        }
        MissionStatus::Active {
            index: self.current,
        }
    }

    fn objective_count(&self) -> usize {
        self.objectives.len()
    }
}

/// Behaviour every `MissionProvider` must have (`AC-61.5`). `fresh` makes a new mission.
///
/// # Panics
/// Panics, with a message naming the property, if the implementation violates the contract.
pub fn contract(fresh: &dyn Fn() -> Box<dyn MissionProvider>) {
    let mut m = fresh();
    let n = m.objective_count();
    let route = [
        DVec3::new(0.0, -6_000.0, 1_500.0),
        DVec3::new(0.0, 0.0, 1_500.0),
        DVec3::new(3_000.0, 5_000.0, 2_000.0),
    ];

    // Time moves forward; the objective index never goes backwards; finished stays finished.
    let mut last_index = 0;
    let mut finished: Option<MissionStatus> = None;
    for step in 0..4_000 {
        let t = f64::from(step) * 0.25;
        let pos = route[(step as usize / 40) % route.len()];
        let status = m.update(t, pos);
        if let Some(done) = finished {
            assert_eq!(status, done, "a finished mission stays finished");
        }
        match status {
            MissionStatus::Active { index } => {
                assert!(
                    index < n,
                    "the active index is a real objective: {index} of {n}"
                );
                assert!(index >= last_index, "objectives are never undone");
                last_index = index;
            }
            MissionStatus::Completed | MissionStatus::Failed => finished = Some(status),
        }
    }

    // Non-finite time or position cannot advance or finish anything.
    let mut m = fresh();
    let before = m.update(0.0, route[0]);
    assert_eq!(
        m.update(f64::NAN, route[0]),
        before,
        "NaN time changes nothing"
    );
    let mut m = fresh();
    for _ in 0..3 {
        let _ = m.update(1.0, DVec3::new(f64::NAN, 0.0, 0.0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn route() -> ScriptedMission {
        ScriptedMission::new(
            vec![
                Objective::Wait { seconds: 5.0 },
                Objective::Reach {
                    point: DVec3::new(0.0, 0.0, 1_500.0),
                    radius_m: 200.0,
                },
                Objective::Wait { seconds: 2.0 },
            ],
            Some(120.0),
        )
    }

    #[test]
    fn it_passes_the_contract() {
        contract(&|| Box::new(route()));
        contract(&|| Box::new(ScriptedMission::new(vec![], None)));
    }

    #[test]
    fn objectives_complete_in_order() {
        let mut m = route();
        let far = DVec3::new(0.0, -6_000.0, 1_500.0);
        let at = DVec3::new(10.0, 20.0, 1_500.0);
        assert_eq!(m.update(0.0, far), MissionStatus::Active { index: 0 });
        assert_eq!(m.update(4.9, far), MissionStatus::Active { index: 0 });
        assert_eq!(m.update(5.0, far), MissionStatus::Active { index: 1 });
        assert_eq!(
            m.update(30.0, far),
            MissionStatus::Active { index: 1 },
            "still not there"
        );
        assert_eq!(m.update(31.0, at), MissionStatus::Active { index: 2 });
        assert_eq!(
            m.update(32.9, at),
            MissionStatus::Active { index: 2 },
            "the last wait counts from 31"
        );
        assert_eq!(m.update(33.0, at), MissionStatus::Completed);
        assert_eq!(
            m.update(34.0, far),
            MissionStatus::Completed,
            "and stays completed"
        );
    }

    #[test]
    fn running_out_of_time_fails_the_mission() {
        let mut m = route();
        let far = DVec3::new(0.0, -6_000.0, 1_500.0);
        assert_eq!(m.update(0.0, far), MissionStatus::Active { index: 0 });
        assert_eq!(m.update(119.0, far), MissionStatus::Active { index: 1 });
        assert_eq!(m.update(120.0, far), MissionStatus::Failed);
        assert_eq!(
            m.update(121.0, DVec3::new(0.0, 0.0, 1_500.0)),
            MissionStatus::Failed
        );
    }

    #[test]
    fn an_empty_mission_is_complete_at_once() {
        let mut m = ScriptedMission::new(vec![], None);
        assert_eq!(m.update(0.0, DVec3::ZERO), MissionStatus::Completed);
        assert_eq!(m.objective_count(), 0);
    }

    #[test]
    fn reaching_a_point_you_are_already_at_skips_straight_on() {
        let mut m = ScriptedMission::new(
            vec![
                Objective::Reach {
                    point: DVec3::ZERO,
                    radius_m: 10.0,
                },
                Objective::Reach {
                    point: DVec3::ZERO,
                    radius_m: 10.0,
                },
            ],
            None,
        );
        assert_eq!(m.update(0.0, DVec3::ZERO), MissionStatus::Completed);
    }
}
