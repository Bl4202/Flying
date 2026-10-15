//! The aero entry (kind 3): an airframe's coefficient tables (GDD 11.6, Appendix C.4, C.5).
//!
//! Angles in tables are degrees (`alpha_deg`, deflection in degrees); derivatives are per radian.
//! Two-dimensional tables are `alpha_deg` along x and `mach` along y, so each row is one Mach
//! number. One-dimensional tables are against Mach unless stated.

use crate::codec::{Reader, Writer};
use crate::cpack::CpackError;
use crate::table::{Curve, Grid};

/// Lift.
#[derive(Clone, Debug, PartialEq)]
pub struct Lift {
    /// `C_L,base(alpha_deg, mach)`, clean, through the stall and beyond.
    pub cl_base: Grid,
    /// Trimmed whole-aircraft `C_L,max(mach)`, for limiters and reduced-order models.
    pub cl_max: Curve,
    /// Lift-curve slope `C_L,alpha(mach)`, per radian, for diagnostics and linearisation.
    pub cl_alpha_per_rad: Curve,
    /// Lift at zero angle of attack, the `C_L0` of the induced-drag term.
    pub cl0: f32,
    /// Lift per radian of elevator, `C_L,de(mach)`.
    pub cl_de_per_rad: Curve,
}

/// Drag: `C_D = C_D0 + dC_D,wave + K (C_L - C_L0)^2 + configuration`.
#[derive(Clone, Debug, PartialEq)]
pub struct Drag {
    /// Zero-lift drag, clean, against Mach.
    pub cd0: Curve,
    /// Transonic and supersonic drag rise against Mach.
    pub cd_wave: Curve,
    /// Induced-drag factor `K(mach)`.
    pub k_induced: Curve,
}

/// Pitching moment about the reference point.
#[derive(Clone, Debug, PartialEq)]
pub struct Pitch {
    /// `C_m0(alpha_deg, mach)`, elevator neutral.
    pub cm_0: Grid,
    /// Moment per radian of elevator, `C_m,de(mach)`.
    pub cm_de_per_rad: Curve,
    /// Pitch damping `C_mq`, per radian of `q c / 2V`.
    pub cm_q_per_rad: f32,
    /// `C_m,alpha_dot`, per radian of `alpha_dot c / 2V`.
    pub cm_alpha_dot_per_rad: f32,
}

/// Lateral and directional derivatives, per radian (GDD C.5).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lateral {
    /// Side force from sideslip.
    pub cy_beta_per_rad: f32,
    /// Rolling moment from sideslip (dihedral effect).
    pub cl_beta_per_rad: f32,
    /// Yawing moment from sideslip (weathercock stability).
    pub cn_beta_per_rad: f32,
    /// Roll damping.
    pub cl_p_per_rad: f32,
    /// Rolling moment from yaw rate.
    pub cl_r_per_rad: f32,
    /// Yawing moment from roll rate.
    pub cn_p_per_rad: f32,
    /// Yaw damping.
    pub cn_r_per_rad: f32,
    /// Rolling moment per radian of aileron.
    pub cl_da_per_rad: f32,
    /// Yawing moment per radian of aileron (adverse yaw).
    pub cn_da_per_rad: f32,
    /// Rolling moment per radian of rudder.
    pub cl_dr_per_rad: f32,
    /// Yawing moment per radian of rudder.
    pub cn_dr_per_rad: f32,
    /// Side force per radian of rudder.
    pub cy_dr_per_rad: f32,
}

/// The blend to a flat-plate model beyond the tables (GDD 11.6.3 as amended by ADR-30).
#[derive(Clone, Debug, PartialEq)]
pub struct PostStall {
    /// Angle of attack where the blend starts, degrees, against Mach.
    pub blend_start_deg: Curve,
    /// Angle of attack where the blend is complete, degrees, against Mach.
    pub blend_end_deg: Curve,
    /// Flat-plate normal-force coefficient at 90 degrees.
    pub cd90: f32,
    /// Aerodynamic centre of the flat plate, fraction of MAC from its leading edge.
    pub x_ac_post_mac: f32,
}

/// Configuration increments (GDD C.4).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConfigDeltas {
    /// Drag of the extended gear.
    pub gear_cd0: f32,
    /// Drag of take-off flaps.
    pub flap_to_cd0: f32,
    /// Drag of landing flaps.
    pub flap_ld_cd0: f32,
    /// Drag of the fully open airbrake.
    pub brake_cd0: f32,
    /// Lift of take-off flaps.
    pub flap_to_cl: f32,
    /// Lift of landing flaps.
    pub flap_ld_cl: f32,
}

/// One airframe's aerodynamic data.
#[derive(Clone, Debug, PartialEq)]
pub struct AeroDef {
    /// Lift.
    pub lift: Lift,
    /// Drag.
    pub drag: Drag,
    /// Pitching moment.
    pub pitch: Pitch,
    /// Lateral and directional derivatives.
    pub lateral: Lateral,
    /// Post-stall blend.
    pub post_stall: PostStall,
    /// Control power against deflection: `eta(deflection_deg)`, 1 for small deflections.
    pub control_effectiveness: Curve,
    /// Configuration increments.
    pub config: ConfigDeltas,
    /// Moment reference point as a fraction of MAC from its leading edge.
    pub x_ref_mac: f32,
    /// Ground-effect lift factor `k_g` (GDD 11.6.5).
    pub ground_effect_k: f32,
}

impl AeroDef {
    pub(crate) fn encode(&self, w: &mut Writer<'_>) -> Result<(), CpackError> {
        let l = &self.lift;
        w.grid(&l.cl_base)?;
        w.curve(&l.cl_max)?;
        w.curve(&l.cl_alpha_per_rad)?;
        w.f32(l.cl0);
        w.curve(&l.cl_de_per_rad)?;
        let d = &self.drag;
        w.curve(&d.cd0)?;
        w.curve(&d.cd_wave)?;
        w.curve(&d.k_induced)?;
        let p = &self.pitch;
        w.grid(&p.cm_0)?;
        w.curve(&p.cm_de_per_rad)?;
        w.f32(p.cm_q_per_rad);
        w.f32(p.cm_alpha_dot_per_rad);
        let t = &self.lateral;
        w.f32s(&[
            t.cy_beta_per_rad,
            t.cl_beta_per_rad,
            t.cn_beta_per_rad,
            t.cl_p_per_rad,
            t.cl_r_per_rad,
            t.cn_p_per_rad,
            t.cn_r_per_rad,
            t.cl_da_per_rad,
            t.cn_da_per_rad,
            t.cl_dr_per_rad,
            t.cn_dr_per_rad,
            t.cy_dr_per_rad,
        ]);
        let s = &self.post_stall;
        w.curve(&s.blend_start_deg)?;
        w.curve(&s.blend_end_deg)?;
        w.f32(s.cd90);
        w.f32(s.x_ac_post_mac);
        w.curve(&self.control_effectiveness)?;
        let c = &self.config;
        w.f32s(&[
            c.gear_cd0,
            c.flap_to_cd0,
            c.flap_ld_cd0,
            c.brake_cd0,
            c.flap_to_cl,
            c.flap_ld_cl,
        ]);
        w.f32(self.x_ref_mac);
        w.f32(self.ground_effect_k);
        Ok(())
    }

    pub(crate) fn decode(r: &mut Reader<'_>) -> Result<Self, CpackError> {
        let lift = Lift {
            cl_base: r.grid()?,
            cl_max: r.curve()?,
            cl_alpha_per_rad: r.curve()?,
            cl0: r.f32()?,
            cl_de_per_rad: r.curve()?,
        };
        let drag = Drag {
            cd0: r.curve()?,
            cd_wave: r.curve()?,
            k_induced: r.curve()?,
        };
        let pitch = Pitch {
            cm_0: r.grid()?,
            cm_de_per_rad: r.curve()?,
            cm_q_per_rad: r.f32()?,
            cm_alpha_dot_per_rad: r.f32()?,
        };
        let lateral = Lateral {
            cy_beta_per_rad: r.f32()?,
            cl_beta_per_rad: r.f32()?,
            cn_beta_per_rad: r.f32()?,
            cl_p_per_rad: r.f32()?,
            cl_r_per_rad: r.f32()?,
            cn_p_per_rad: r.f32()?,
            cn_r_per_rad: r.f32()?,
            cl_da_per_rad: r.f32()?,
            cn_da_per_rad: r.f32()?,
            cl_dr_per_rad: r.f32()?,
            cn_dr_per_rad: r.f32()?,
            cy_dr_per_rad: r.f32()?,
        };
        let post_stall = PostStall {
            blend_start_deg: r.curve()?,
            blend_end_deg: r.curve()?,
            cd90: r.f32()?,
            x_ac_post_mac: r.f32()?,
        };
        let control_effectiveness = r.curve()?;
        let config = ConfigDeltas {
            gear_cd0: r.f32()?,
            flap_to_cd0: r.f32()?,
            flap_ld_cd0: r.f32()?,
            brake_cd0: r.f32()?,
            flap_to_cl: r.f32()?,
            flap_ld_cl: r.f32()?,
        };
        Ok(Self {
            lift,
            drag,
            pitch,
            lateral,
            post_stall,
            control_effectiveness,
            config,
            x_ref_mac: r.f32()?,
            ground_effect_k: r.f32()?,
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::codec::Strings;

    /// A small but complete aero set (not the Kestrel's tables, which are content).
    pub(crate) fn sample() -> AeroDef {
        let flat = |v: f32| Curve::from_points(&[(0.0, v)]);
        let grid = |a: f32| Grid {
            xs: vec![-10.0, 0.0, 10.0, 20.0],
            ys: vec![0.0, 1.0],
            zs: vec![-a, 0.0, a, 1.5 * a, -0.8 * a, 0.0, 0.8 * a, 1.2 * a],
        };
        AeroDef {
            lift: Lift {
                cl_base: grid(0.6),
                cl_max: Curve::from_points(&[(0.0, 1.6), (1.2, 1.0)]),
                cl_alpha_per_rad: flat(3.6),
                cl0: 0.0,
                cl_de_per_rad: flat(0.41),
            },
            drag: Drag {
                cd0: flat(0.0235),
                cd_wave: Curve::from_points(&[(0.75, 0.0), (1.05, 0.026)]),
                k_induced: flat(0.135),
            },
            pitch: Pitch {
                cm_0: grid(-0.04),
                cm_de_per_rad: flat(-0.65),
                cm_q_per_rad: -9.0,
                cm_alpha_dot_per_rad: -3.0,
            },
            lateral: Lateral {
                cy_beta_per_rad: -0.9,
                cl_beta_per_rad: -0.1,
                cn_beta_per_rad: 0.14,
                cl_p_per_rad: -0.3,
                cl_r_per_rad: 0.1,
                cn_p_per_rad: -0.03,
                cn_r_per_rad: -0.3,
                cl_da_per_rad: 0.16,
                cn_da_per_rad: -0.01,
                cl_dr_per_rad: 0.01,
                cn_dr_per_rad: -0.07,
                cy_dr_per_rad: 0.2,
            },
            post_stall: PostStall {
                blend_start_deg: flat(40.0),
                blend_end_deg: flat(60.0),
                cd90: 1.2,
                x_ac_post_mac: 0.5,
            },
            control_effectiveness: Curve::from_points(&[(0.0, 1.0), (15.0, 1.0), (30.0, 0.7)]),
            config: ConfigDeltas {
                gear_cd0: 0.015,
                flap_to_cd0: 0.02,
                flap_ld_cd0: 0.03,
                brake_cd0: 0.02,
                flap_to_cl: 0.3,
                flap_ld_cl: 0.3,
            },
            x_ref_mac: 0.27,
            ground_effect_k: 0.1,
        }
    }

    #[test]
    fn a_record_round_trips() {
        let def = sample();
        let mut strings = Strings::default();
        let mut w = Writer::new(&mut strings);
        def.encode(&mut w).unwrap();
        let rec = w.finish();
        assert!(strings.bytes.is_empty(), "aero records hold no strings");
        let mut r = Reader::new(&rec, "");
        assert_eq!(AeroDef::decode(&mut r).unwrap(), def);
        r.finish().unwrap();
    }

    #[test]
    fn an_invalid_table_cannot_be_written() {
        let mut def = sample();
        def.drag.cd0 = Curve::from_points(&[(1.0, 0.02), (0.5, 0.02)]);
        let mut strings = Strings::default();
        let mut w = Writer::new(&mut strings);
        assert!(def.encode(&mut w).is_err());
    }
}
