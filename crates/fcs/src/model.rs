//! The control-law gains of an airframe in the units the laws use (SI, radians, pascals), built
//! once from the `fcs/1` entry (ADR-34). Nothing here names a content entry: the gains are found
//! through the aircraft's `gains_ref`.

use content::{ContentId, Registry};
use flight::ModelError;
use flight::aircraft::{AircraftModel, Limits};
use flight::table::Table1D;

/// Pitch axis gains.
#[derive(Clone, Debug, PartialEq)]
pub struct PitchK {
    /// Pitch-rate loop bandwidth, 1/s.
    pub k_q: f64,
    /// Load-factor loop bandwidth, 1/s.
    pub k_n: f64,
    /// Integral gain relative to the bandwidth, 1/s.
    pub k_ni: f64,
    /// Command prefilter time constant, s.
    pub tau_cmd_s: f64,
    /// Width of the angle-of-attack limiter band, rad.
    pub alpha_band_rad: f64,
    /// Angle-of-attack feedback inside the band, 1/s.
    pub k_alpha: f64,
    /// Most negative angle of attack the limiter allows, rad.
    pub alpha_min_rad: f64,
    /// Largest Reduced-law pitch rate, rad/s, against dynamic pressure in kPa.
    pub q_max: Table1D,
}

/// Roll axis gains.
#[derive(Clone, Debug, PartialEq)]
pub struct RollK {
    /// Roll-rate loop bandwidth, 1/s.
    pub k_p: f64,
    /// Integral gain relative to the bandwidth, 1/s.
    pub k_pi: f64,
    /// Command prefilter time constant, s.
    pub tau_cmd_s: f64,
    /// Fraction of the largest roll rate, against dynamic pressure in kPa.
    pub p_max_qbar: Table1D,
    /// Fraction, against |angle of attack| in degrees.
    pub p_max_alpha: Table1D,
    /// Fraction, against load factor in g.
    pub p_max_nz: Table1D,
}

/// Yaw axis gains.
#[derive(Clone, Debug, PartialEq)]
pub struct YawK {
    /// Sideslip loop bandwidth, 1/s.
    pub k_beta: f64,
    /// Yaw-rate loop bandwidth, 1/s.
    pub k_r: f64,
    /// Largest sideslip the pedals command, rad, against dynamic pressure in kPa.
    pub beta_max: Table1D,
    /// Time constant of the Reduced yaw damper, s.
    pub yaw_damper_s: f64,
    /// Washout time constant of the damper, s.
    pub washout_s: f64,
}

/// The Full law.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FullK {
    /// Largest flight-path rate, rad/s.
    pub gamma_rate_max: f64,
    /// Flight-path hold gain, 1/s.
    pub k_gamma: f64,
    /// Largest bank, rad.
    pub bank_max_rad: f64,
    /// Bank-angle gain, 1/s.
    pub k_bank: f64,
    /// How much tighter than the Standard limit the angle-of-attack limit is, rad.
    pub alpha_margin_rad: f64,
}

/// Nose-wheel steering range against speed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NwsK {
    /// Full range below this ground speed, m/s.
    pub low_speed_mps: f64,
    /// Small range above this ground speed, m/s.
    pub high_speed_mps: f64,
    /// Full range, rad.
    pub low_rad: f64,
    /// Small range, rad.
    pub high_rad: f64,
}

impl NwsK {
    /// The steering range at a ground speed, rad: linear between the two.
    #[must_use]
    pub fn limit_rad(&self, gs_mps: f64) -> f64 {
        let w = ((gs_mps - self.low_speed_mps) / (self.high_speed_mps - self.low_speed_mps))
            .clamp(0.0, 1.0);
        self.low_rad + w * (self.high_rad - self.low_rad)
    }
}

/// Ground mode.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroundK {
    /// Weight-on-wheels debounce, s.
    pub wow_debounce_s: f64,
    /// Blend from the ground law to the flight law after lift-off, s.
    pub liftoff_blend_s: f64,
    /// Blend from the flight law to the ground law after touchdown, s.
    pub touchdown_blend_s: f64,
    /// Pitch-rate damping of the ground law, s.
    pub k_q_ground: f64,
    /// Nose-wheel steering.
    pub nws: NwsK,
}

/// Mouse-aim.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MouseAimK {
    /// Roll command per radian of bank error with the aim near.
    pub k_bank: f64,
    /// Roll command per radian of bank error with the aim far.
    pub k_bank_far: f64,
    /// Turn rate asked for per radian of aim error, 1/s.
    pub k_pitch: f64,
    /// Aim error within which the turn rate asked for fades to nothing, rad.
    pub deadzone_rad: f64,
    /// Stick ramp, full scale per second.
    pub ramp_per_s: f64,
    /// How far ahead along the rate it closes at the nose is looked at, s.
    pub lead_s: f64,
    /// The largest turn rate toward the aim asked for, rad/s.
    pub rate_cap_rps: f64,
    /// The largest pull the aim asks for, as a fraction of the pitch stick.
    pub pitch_stick_max: f64,
    /// Roll command taken off per unit of roll rate, as a fraction of the largest roll rate.
    pub k_roll_rate: f64,
}

/// The recovery button.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RecoverK {
    /// Load factor of the pull-up, g.
    pub n_pull: f64,
    /// Flight-path angle aimed for, rad.
    pub gamma_rad: f64,
}

/// The gains and limits of one airframe's control laws.
#[derive(Clone, Debug, PartialEq)]
pub struct FcsModel {
    /// The tick the laws run at, s.
    pub tick_dt_s: f64,
    /// The airframe's limits.
    pub limits: Limits,
    /// Pitch.
    pub pitch: PitchK,
    /// Roll.
    pub roll: RollK,
    /// Yaw.
    pub yaw: YawK,
    /// Full law.
    pub full: FullK,
    /// Fraction of `vmo` and `mmo` where the overspeed wall starts.
    pub overspeed_onset: f64,
    /// Pull-up added per unit of speed ratio past the onset, g.
    pub overspeed_k_pull: f64,
    /// Negative load limit with the gear or flaps out, g.
    pub n_min_config: f64,
    /// Ground mode.
    pub ground: GroundK,
    /// Surface blend after an assist switch, s.
    pub assist_blend_s: f64,
    /// Mouse-aim.
    pub mouse_aim: MouseAimK,
    /// Recovery.
    pub recover: RecoverK,
}

fn curve(name: &'static str, c: &content::Curve, y_scale: f64) -> Result<Table1D, ModelError> {
    let xs: Vec<f64> = c.xs.iter().map(|&x| f64::from(x)).collect();
    let ys: Vec<f64> = c.ys.iter().map(|&y| f64::from(y) * y_scale).collect();
    Table1D::new(&xs, &ys).map_err(|error| ModelError::Table { table: name, error })
}

impl FcsModel {
    /// Builds the laws of the aircraft behind `handle`. The gains come from the entry its `fcs`
    /// block refers to; the limits from the airframe model.
    ///
    /// # Errors
    /// [`ModelError`] if the aircraft or its gains are missing or a table is malformed.
    pub fn from_registry(
        registry: &Registry,
        handle: ContentId,
        model: &AircraftModel,
    ) -> Result<Self, ModelError> {
        let ac = registry
            .aircraft(handle)
            .ok_or(ModelError::Missing("aircraft"))?;
        let g = registry
            .fcs(ac.fcs.gains)
            .ok_or(ModelError::Missing("fcs gains"))?;
        let f = f64::from;
        let rad = |deg: f32| f64::from(deg).to_radians();
        let (pi, y, full) = (&g.pitch, &g.yaw, &g.full);
        Ok(Self {
            tick_dt_s: model.tick_dt_s(),
            limits: model.limits,
            pitch: PitchK {
                k_q: f(pi.k_q_per_s),
                k_n: f(pi.k_n_per_s),
                k_ni: f(pi.k_ni_per_s),
                tau_cmd_s: f(pi.tau_cmd_s),
                alpha_band_rad: rad(pi.alpha_band_deg),
                k_alpha: f(pi.k_alpha_per_s),
                alpha_min_rad: rad(pi.alpha_min_deg),
                q_max: curve("q_max_dps", &pi.q_max_dps, 1.0_f64.to_radians())?,
            },
            roll: RollK {
                k_p: f(g.roll.k_p_per_s),
                k_pi: f(g.roll.k_pi_per_s),
                tau_cmd_s: f(g.roll.tau_cmd_s),
                p_max_qbar: curve("p_max_qbar", &g.roll.p_max_qbar, 1.0)?,
                p_max_alpha: curve("p_max_alpha", &g.roll.p_max_alpha, 1.0)?,
                p_max_nz: curve("p_max_nz", &g.roll.p_max_nz, 1.0)?,
            },
            yaw: YawK {
                k_beta: f(y.k_beta_per_s),
                k_r: f(y.k_r_per_s),
                beta_max: curve("beta_max_deg", &y.beta_max_deg, 1.0_f64.to_radians())?,
                yaw_damper_s: f(y.yaw_damper_s),
                washout_s: f(y.washout_s),
            },
            full: FullK {
                gamma_rate_max: rad(full.gamma_rate_max_dps),
                k_gamma: f(full.k_gamma_per_s),
                bank_max_rad: rad(full.bank_max_deg),
                k_bank: f(full.k_bank_per_s),
                alpha_margin_rad: rad(full.alpha_margin_deg),
            },
            overspeed_onset: f(g.overspeed.onset),
            overspeed_k_pull: f(g.overspeed.k_pull),
            n_min_config: f(g.config.n_min_config),
            ground: GroundK {
                wow_debounce_s: f(g.ground.wow_debounce_s),
                liftoff_blend_s: f(g.ground.liftoff_blend_s),
                touchdown_blend_s: f(g.ground.touchdown_blend_s),
                k_q_ground: f(g.ground.k_q_ground_s),
                nws: NwsK {
                    low_speed_mps: f(g.ground.nws.low_speed_mps),
                    high_speed_mps: f(g.ground.nws.high_speed_mps),
                    low_rad: rad(g.ground.nws.low_deg),
                    high_rad: rad(g.ground.nws.high_deg),
                },
            },
            assist_blend_s: f(g.assist_blend_s),
            mouse_aim: MouseAimK {
                k_bank: f(g.mouse_aim.k_bank),
                k_bank_far: f(g.mouse_aim.k_bank_far),
                k_pitch: f(g.mouse_aim.k_pitch_per_rad),
                deadzone_rad: rad(g.mouse_aim.deadzone_deg),
                ramp_per_s: f(g.mouse_aim.ramp_per_s),
                lead_s: f(g.mouse_aim.lead_s),
                rate_cap_rps: rad(g.mouse_aim.rate_cap_dps),
                pitch_stick_max: f(g.mouse_aim.pitch_stick_max),
                k_roll_rate: f(g.mouse_aim.k_roll_rate),
            },
            recover: RecoverK {
                n_pull: f(g.recover.n_pull),
                gamma_rad: rad(g.recover.gamma_deg),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_content::kestrel;

    #[test]
    fn the_kestrels_gains_are_read_in_si_units() {
        let (model, fcs) = kestrel();
        assert_eq!(fcs.tick_dt_s, 1.0 / 60.0);
        assert_eq!(fcs.limits, model.limits);
        // A bandwidth in 1/s: the file's number, read as it stands (it is tuned in the data).
        assert!((1.0..8.0).contains(&fcs.pitch.k_n), "{}", fcs.pitch.k_n);
        assert!((fcs.pitch.alpha_band_rad - 3f64.to_radians()).abs() < 1e-12);
        // 30 deg/s at 20 kPa, in rad/s; the curve is against kilopascals.
        assert!((fcs.pitch.q_max.eval(20.0) - 30f64.to_radians()).abs() < 1e-6);
        assert!((fcs.yaw.beta_max.eval(40.0) - 3f64.to_radians()).abs() < 1e-6);
        assert_eq!(fcs.roll.p_max_alpha.eval(26.0), 0.5);
        assert!((fcs.full.bank_max_rad - 60f64.to_radians()).abs() < 1e-12);
        assert!((fcs.ground.wow_debounce_s - 0.1).abs() < 1e-6);
        assert_eq!(fcs.recover.n_pull, 4.0);
    }

    #[test]
    fn nose_wheel_steering_narrows_with_speed() {
        let (_, fcs) = kestrel();
        let n = fcs.ground.nws;
        assert!((n.limit_rad(0.0) - 60f64.to_radians()).abs() < 1e-6);
        assert!((n.limit_rad(15.0) - 60f64.to_radians()).abs() < 1e-6);
        assert!(
            (n.limit_rad(22.5) - 32.5f64.to_radians()).abs() < 1e-6,
            "halfway"
        );
        assert!((n.limit_rad(30.0) - 5f64.to_radians()).abs() < 1e-6);
        assert!((n.limit_rad(80.0) - 5f64.to_radians()).abs() < 1e-6);
    }

    #[test]
    fn a_pack_without_gains_is_refused() {
        let registry = content::Registry::empty();
        let (model, _) = kestrel();
        let handle = content::ContentId(1);
        assert!(FcsModel::from_registry(&registry, handle, &model).is_err());
    }
}
