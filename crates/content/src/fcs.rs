//! The flight-control gains entry (kind 4): the data of the control laws of one airframe
//! (GDD 11.11, A.2 `fcs`, ADR-34).
//!
//! The aircraft entry holds what the airframe *is* (limits, actuators); this entry holds how its
//! control laws are tuned. An aircraft refers to its gains by handle. Gains are scalars, with
//! `Table1D` schedules where the authority changes with dynamic pressure, angle of attack or load
//! factor. Angles are degrees, rates per second, pressures kilopascals; the sim converts at load.

use crate::codec::{Reader, Writer};
use crate::cpack::CpackError;
use crate::table::Curve;

/// Pitch axis: the load-factor loop of the Standard law, the pitch-rate command of Reduced, and
/// the angle-of-attack limiter.
#[derive(Clone, Debug, PartialEq)]
pub struct PitchGains {
    /// Pitch-rate damping gain of the inner loop, per second.
    pub k_q_per_s: f32,
    /// Proportional gain on the load-factor error (rate per g, scaled by `g/V` in the law).
    pub k_n: f32,
    /// Integral gain on the load-factor error, per second.
    pub k_ni_per_s: f32,
    /// Time constant of the command prefilter, s.
    pub tau_cmd_s: f32,
    /// Width of the angle-of-attack limiter band below the limit, degrees.
    pub alpha_band_deg: f32,
    /// Angle-of-attack feedback gain inside the band, per second.
    pub k_alpha_per_s: f32,
    /// Most negative angle of attack the limiter allows, degrees.
    pub alpha_min_deg: f32,
    /// Largest commanded pitch rate of the Reduced law, deg/s, against dynamic pressure, kPa.
    pub q_max_dps: Curve,
}

/// Roll axis: the roll-rate command and its authority schedules.
#[derive(Clone, Debug, PartialEq)]
pub struct RollGains {
    /// Roll-rate damping gain, per second.
    pub k_p_per_s: f32,
    /// Integral gain on the roll-rate error, per second.
    pub k_pi_per_s: f32,
    /// Time constant of the command prefilter, s.
    pub tau_cmd_s: f32,
    /// Fraction of the maximum roll rate available, against dynamic pressure, kPa.
    pub p_max_qbar: Curve,
    /// Fraction of the maximum roll rate available, against angle of attack, degrees.
    pub p_max_alpha: Curve,
    /// Fraction of the maximum roll rate available, against load factor, g.
    pub p_max_nz: Curve,
}

/// Yaw axis: coordination and damping.
#[derive(Clone, Debug, PartialEq)]
pub struct YawGains {
    /// Sideslip feedback gain, per second.
    pub k_beta_per_s: f32,
    /// Yaw-rate feedback gain, per second.
    pub k_r_per_s: f32,
    /// Largest sideslip the pedals command, degrees, against dynamic pressure, kPa.
    pub beta_max_deg: Curve,
    /// Time constant of the Reduced-law yaw damper, s.
    pub yaw_damper_s: f32,
    /// Washout time constant of the yaw damper, s.
    pub washout_s: f32,
}

/// The Full law: flight-path-rate and bank-angle commands.
#[derive(Clone, Debug, PartialEq)]
pub struct FullGains {
    /// Largest commanded flight-path rate, deg/s.
    pub gamma_rate_max_dps: f32,
    /// Flight-path hold gain, per second.
    pub k_gamma_per_s: f32,
    /// Largest commanded bank angle, degrees.
    pub bank_max_deg: f32,
    /// Bank-angle gain (roll rate per radian of error), per second.
    pub k_bank_per_s: f32,
    /// How much tighter than the Standard limit the angle-of-attack limit is, degrees.
    pub alpha_margin_deg: f32,
}

/// The overspeed soft wall.
#[derive(Clone, Debug, PartialEq)]
pub struct OverspeedGains {
    /// Fraction of `vmo` and `mmo` where the wall begins.
    pub onset: f32,
    /// Pull-up command added per unit of speed ratio past the onset, g.
    pub k_pull: f32,
}

/// Configuration-dependent limits.
#[derive(Clone, Debug, PartialEq)]
pub struct ConfigGains {
    /// Negative load limit with the gear or flaps out, g (negative).
    pub n_min_config: f32,
}

/// Nose-wheel steering: the deflection limit falls with speed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NwsGains {
    /// Below this ground speed the full range is available, m/s.
    pub low_speed_mps: f32,
    /// Above this ground speed only the small range is available, m/s.
    pub high_speed_mps: f32,
    /// Full range, degrees.
    pub low_deg: f32,
    /// Small range, degrees.
    pub high_deg: f32,
}

/// The ground mode and its blends.
#[derive(Clone, Debug, PartialEq)]
pub struct GroundGains {
    /// Weight-on-wheels debounce, s.
    pub wow_debounce_s: f32,
    /// Blend from the ground law to the flight law after lift-off, s.
    pub liftoff_blend_s: f32,
    /// Blend from the flight law to the ground law after touchdown, s.
    pub touchdown_blend_s: f32,
    /// Pitch-rate damping of the ground law, per second.
    pub k_q_ground_s: f32,
    /// Nose-wheel steering.
    pub nws: NwsGains,
}

/// The mouse-aim controller (GDD 58.4).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MouseAimGains {
    /// Bank command per unit of heading error.
    pub k_bank: f32,
    /// Pitch command per radian of elevation error, per radian.
    pub k_pitch_per_rad: f32,
    /// Aim error below which no input is given, degrees.
    pub deadzone_deg: f32,
    /// Stick ramp, full scale per second.
    pub ramp_per_s: f32,
}

/// The recovery button.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RecoverGains {
    /// Load factor of the recovery pull-up, g.
    pub n_pull: f32,
    /// Flight-path angle the recovery aims for, degrees.
    pub gamma_deg: f32,
}

/// Automatic ground-collision avoidance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GcasGains {
    /// Load factor of the avoidance pull-up, g.
    pub n_pull: f32,
    /// Terrain clearance below which it triggers, m.
    pub buffer_m: f32,
    /// Look-ahead, s.
    pub horizon_s: f32,
    /// Look-ahead step, s.
    pub step_s: f32,
}

/// The control-law gains of one airframe.
#[derive(Clone, Debug, PartialEq)]
pub struct FcsDef {
    /// Locale key of the display name.
    pub name_key: String,
    /// Pitch axis.
    pub pitch: PitchGains,
    /// Roll axis.
    pub roll: RollGains,
    /// Yaw axis.
    pub yaw: YawGains,
    /// The Full law.
    pub full: FullGains,
    /// The overspeed soft wall.
    pub overspeed: OverspeedGains,
    /// Configuration limits.
    pub config: ConfigGains,
    /// Ground mode.
    pub ground: GroundGains,
    /// Time over which an assist switch blends the surface command, s.
    pub assist_blend_s: f32,
    /// Mouse-aim.
    pub mouse_aim: MouseAimGains,
    /// The recovery button.
    pub recover: RecoverGains,
    /// Automatic ground-collision avoidance.
    pub gcas: GcasGains,
}

impl FcsDef {
    pub(crate) fn encode(&self, w: &mut Writer<'_>) -> Result<(), CpackError> {
        w.str(&self.name_key)?;
        let p = &self.pitch;
        w.f32s(&[
            p.k_q_per_s,
            p.k_n,
            p.k_ni_per_s,
            p.tau_cmd_s,
            p.alpha_band_deg,
            p.k_alpha_per_s,
            p.alpha_min_deg,
        ]);
        w.curve(&p.q_max_dps)?;
        let r = &self.roll;
        w.f32s(&[r.k_p_per_s, r.k_pi_per_s, r.tau_cmd_s]);
        w.curve(&r.p_max_qbar)?;
        w.curve(&r.p_max_alpha)?;
        w.curve(&r.p_max_nz)?;
        let y = &self.yaw;
        w.f32s(&[y.k_beta_per_s, y.k_r_per_s, y.yaw_damper_s, y.washout_s]);
        w.curve(&y.beta_max_deg)?;
        let f = &self.full;
        w.f32s(&[
            f.gamma_rate_max_dps,
            f.k_gamma_per_s,
            f.bank_max_deg,
            f.k_bank_per_s,
            f.alpha_margin_deg,
        ]);
        w.f32s(&[self.overspeed.onset, self.overspeed.k_pull]);
        w.f32(self.config.n_min_config);
        let g = &self.ground;
        w.f32s(&[
            g.wow_debounce_s,
            g.liftoff_blend_s,
            g.touchdown_blend_s,
            g.k_q_ground_s,
            g.nws.low_speed_mps,
            g.nws.high_speed_mps,
            g.nws.low_deg,
            g.nws.high_deg,
        ]);
        w.f32(self.assist_blend_s);
        let m = &self.mouse_aim;
        w.f32s(&[m.k_bank, m.k_pitch_per_rad, m.deadzone_deg, m.ramp_per_s]);
        w.f32s(&[self.recover.n_pull, self.recover.gamma_deg]);
        let c = &self.gcas;
        w.f32s(&[c.n_pull, c.buffer_m, c.horizon_s, c.step_s]);
        Ok(())
    }

    pub(crate) fn decode(r: &mut Reader<'_>) -> Result<Self, CpackError> {
        let name_key = r.str()?;
        let pitch = PitchGains {
            k_q_per_s: r.f32()?,
            k_n: r.f32()?,
            k_ni_per_s: r.f32()?,
            tau_cmd_s: r.f32()?,
            alpha_band_deg: r.f32()?,
            k_alpha_per_s: r.f32()?,
            alpha_min_deg: r.f32()?,
            q_max_dps: r.curve()?,
        };
        let roll = RollGains {
            k_p_per_s: r.f32()?,
            k_pi_per_s: r.f32()?,
            tau_cmd_s: r.f32()?,
            p_max_qbar: r.curve()?,
            p_max_alpha: r.curve()?,
            p_max_nz: r.curve()?,
        };
        let yaw = YawGains {
            k_beta_per_s: r.f32()?,
            k_r_per_s: r.f32()?,
            yaw_damper_s: r.f32()?,
            washout_s: r.f32()?,
            beta_max_deg: r.curve()?,
        };
        let full = FullGains {
            gamma_rate_max_dps: r.f32()?,
            k_gamma_per_s: r.f32()?,
            bank_max_deg: r.f32()?,
            k_bank_per_s: r.f32()?,
            alpha_margin_deg: r.f32()?,
        };
        let overspeed = OverspeedGains {
            onset: r.f32()?,
            k_pull: r.f32()?,
        };
        let config = ConfigGains {
            n_min_config: r.f32()?,
        };
        let ground = GroundGains {
            wow_debounce_s: r.f32()?,
            liftoff_blend_s: r.f32()?,
            touchdown_blend_s: r.f32()?,
            k_q_ground_s: r.f32()?,
            nws: NwsGains {
                low_speed_mps: r.f32()?,
                high_speed_mps: r.f32()?,
                low_deg: r.f32()?,
                high_deg: r.f32()?,
            },
        };
        let assist_blend_s = r.f32()?;
        let mouse_aim = MouseAimGains {
            k_bank: r.f32()?,
            k_pitch_per_rad: r.f32()?,
            deadzone_deg: r.f32()?,
            ramp_per_s: r.f32()?,
        };
        let recover = RecoverGains {
            n_pull: r.f32()?,
            gamma_deg: r.f32()?,
        };
        let gcas = GcasGains {
            n_pull: r.f32()?,
            buffer_m: r.f32()?,
            horizon_s: r.f32()?,
            step_s: r.f32()?,
        };
        Ok(Self {
            name_key,
            pitch,
            roll,
            yaw,
            full,
            overspeed,
            config,
            ground,
            assist_blend_s,
            mouse_aim,
            recover,
            gcas,
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::codec::Strings;

    /// A complete, valid set of gains (not the Kestrel's, which are content).
    pub(crate) fn sample() -> FcsDef {
        let flat = |x: f32, v: f32| Curve::from_points(&[(0.0, v), (x, v)]);
        FcsDef {
            name_key: "fcs.sparrow.name".into(),
            pitch: PitchGains {
                k_q_per_s: 6.0,
                k_n: 1.5,
                k_ni_per_s: 1.0,
                tau_cmd_s: 0.1,
                alpha_band_deg: 3.0,
                k_alpha_per_s: 3.0,
                alpha_min_deg: -10.0,
                q_max_dps: Curve::from_points(&[(0.0, 10.0), (5.0, 20.0), (20.0, 30.0)]),
            },
            roll: RollGains {
                k_p_per_s: 6.0,
                k_pi_per_s: 0.5,
                tau_cmd_s: 0.08,
                p_max_qbar: Curve::from_points(&[(0.0, 0.25), (10.0, 1.0), (100.0, 0.8)]),
                p_max_alpha: Curve::from_points(&[(0.0, 1.0), (26.0, 0.5), (35.0, 0.3)]),
                p_max_nz: flat(9.0, 1.0),
            },
            yaw: YawGains {
                k_beta_per_s: 2.0,
                k_r_per_s: 4.0,
                yaw_damper_s: 0.5,
                washout_s: 1.0,
                beta_max_deg: Curve::from_points(&[(0.0, 10.0), (10.0, 6.0), (40.0, 3.0)]),
            },
            full: FullGains {
                gamma_rate_max_dps: 10.0,
                k_gamma_per_s: 0.8,
                bank_max_deg: 60.0,
                k_bank_per_s: 2.0,
                alpha_margin_deg: 2.0,
            },
            overspeed: OverspeedGains {
                onset: 0.97,
                k_pull: 30.0,
            },
            config: ConfigGains { n_min_config: -1.0 },
            ground: GroundGains {
                wow_debounce_s: 0.1,
                liftoff_blend_s: 2.0,
                touchdown_blend_s: 0.5,
                k_q_ground_s: 0.25,
                nws: NwsGains {
                    low_speed_mps: 15.0,
                    high_speed_mps: 30.0,
                    low_deg: 60.0,
                    high_deg: 5.0,
                },
            },
            assist_blend_s: 0.5,
            mouse_aim: MouseAimGains {
                k_bank: 1.6,
                k_pitch_per_rad: 3.0,
                deadzone_deg: 1.0,
                ramp_per_s: 3.0,
            },
            recover: RecoverGains {
                n_pull: 4.0,
                gamma_deg: 5.0,
            },
            gcas: GcasGains {
                n_pull: 5.0,
                buffer_m: 50.0,
                horizon_s: 10.0,
                step_s: 0.1,
            },
        }
    }

    #[test]
    fn a_record_round_trips() {
        let def = sample();
        let mut strings = Strings::default();
        let mut w = Writer::new(&mut strings);
        def.encode(&mut w).unwrap();
        let rec = w.finish();
        let text = String::from_utf8(strings.bytes).unwrap();
        let mut r = Reader::new(&rec, &text);
        assert_eq!(FcsDef::decode(&mut r).unwrap(), def);
        r.finish().unwrap();
    }

    #[test]
    fn an_invalid_schedule_cannot_be_written() {
        let mut def = sample();
        def.pitch.q_max_dps = Curve::from_points(&[(5.0, 10.0), (1.0, 20.0)]);
        let mut strings = Strings::default();
        let mut w = Writer::new(&mut strings);
        assert!(def.encode(&mut w).is_err());
    }

    #[test]
    fn a_truncated_record_is_refused() {
        let def = sample();
        let mut strings = Strings::default();
        let mut w = Writer::new(&mut strings);
        def.encode(&mut w).unwrap();
        let mut rec = w.finish();
        rec.truncate(rec.len() - 16);
        let text = String::from_utf8(strings.bytes).unwrap();
        let mut r = Reader::new(&rec, &text);
        assert!(FcsDef::decode(&mut r).and_then(|_| r.finish()).is_err());
    }
}
