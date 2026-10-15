//! The engine entry (kind 2): thrust and fuel decks and spool dynamics (GDD 11.8, A.3, C.3).

use crate::codec::{Reader, Writer};
use crate::cpack::CpackError;
use crate::enums::EngineKind;
use crate::table::Curve;

/// What the sim knows about one engine type. Thrust and fuel flow are per engine.
#[derive(Clone, Debug, PartialEq)]
pub struct EngineDef {
    /// Locale key of the display name.
    pub name_key: String,
    /// Engine type.
    pub kind: EngineKind,
    /// Sea-level static thrust at military (dry) power, N.
    pub sl_dry_n: f32,
    /// Sea-level static thrust in full afterburner, N (equal to dry if there is none).
    pub sl_ab_n: f32,
    /// Exponent of the density lapse `(rho / rho0)^n`.
    pub density_exp: f32,
    /// Thrust factor against Mach, dry.
    pub mach_factor_dry: Curve,
    /// Thrust factor against Mach, afterburner.
    pub mach_factor_ab: Curve,
    /// Idle thrust as a fraction of military thrust.
    pub idle_fraction: f32,
    /// Thrust-specific fuel consumption, dry, kg/(N s).
    pub tsfc_dry_kg_per_ns: f32,
    /// Thrust-specific fuel consumption, full afterburner, kg/(N s).
    pub tsfc_ab_kg_per_ns: f32,
    /// Spool-up time constant, s.
    pub spool_up_s: f32,
    /// Spool-down time constant, s.
    pub spool_down_s: f32,
    /// Afterburner light-off delay, s.
    pub ab_lightoff_s: f32,
    /// Time constant of the afterburner level rising once lit, s.
    pub ab_stage_s: f32,
    /// Time constant of the afterburner level falling when it is throttled back or cut, s.
    pub ab_cutoff_s: f32,
    /// Afterburner stages (0 for none).
    pub ab_stages: u8,
    /// Angle of attack where inlet distortion begins, degrees.
    pub inlet_alpha_start_deg: f32,
    /// Angle-of-attack span over which the inlet loss reaches its maximum, degrees.
    pub inlet_alpha_span_deg: f32,
    /// Angle of attack above which the compressor stalls at low speed, degrees.
    pub compressor_stall_alpha_deg: f32,
    /// Lowest airspeed for a windmill relight, m/s.
    pub windmill_relight_mps: f32,
}

impl EngineDef {
    pub(crate) fn encode(&self, w: &mut Writer<'_>) -> Result<(), CpackError> {
        w.str(&self.name_key)?;
        w.u8(self.kind.code());
        w.u8(self.ab_stages);
        w.f32s(&[
            self.sl_dry_n,
            self.sl_ab_n,
            self.density_exp,
            self.idle_fraction,
        ]);
        w.curve(&self.mach_factor_dry)?;
        w.curve(&self.mach_factor_ab)?;
        w.f32s(&[
            self.tsfc_dry_kg_per_ns,
            self.tsfc_ab_kg_per_ns,
            self.spool_up_s,
            self.spool_down_s,
            self.ab_lightoff_s,
            self.ab_stage_s,
            self.ab_cutoff_s,
            self.inlet_alpha_start_deg,
            self.inlet_alpha_span_deg,
            self.compressor_stall_alpha_deg,
            self.windmill_relight_mps,
        ]);
        Ok(())
    }

    pub(crate) fn decode(r: &mut Reader<'_>) -> Result<Self, CpackError> {
        let name_key = r.str()?;
        let kind =
            EngineKind::from_code(r.u8()?).ok_or(CpackError::Record("unknown engine kind"))?;
        let ab_stages = r.u8()?;
        Ok(Self {
            name_key,
            kind,
            ab_stages,
            sl_dry_n: r.f32()?,
            sl_ab_n: r.f32()?,
            density_exp: r.f32()?,
            idle_fraction: r.f32()?,
            mach_factor_dry: r.curve()?,
            mach_factor_ab: r.curve()?,
            tsfc_dry_kg_per_ns: r.f32()?,
            tsfc_ab_kg_per_ns: r.f32()?,
            spool_up_s: r.f32()?,
            spool_down_s: r.f32()?,
            ab_lightoff_s: r.f32()?,
            ab_stage_s: r.f32()?,
            ab_cutoff_s: r.f32()?,
            inlet_alpha_start_deg: r.f32()?,
            inlet_alpha_span_deg: r.f32()?,
            compressor_stall_alpha_deg: r.f32()?,
            windmill_relight_mps: r.f32()?,
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::codec::Strings;

    /// The F129 of GDD C.3.
    pub(crate) fn sample() -> EngineDef {
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

    #[test]
    fn a_record_round_trips() {
        let def = sample();
        let mut strings = Strings::default();
        let mut w = Writer::new(&mut strings);
        def.encode(&mut w).unwrap();
        let rec = w.finish();
        let text = String::from_utf8(strings.bytes).unwrap();
        let mut r = Reader::new(&rec, &text);
        assert_eq!(EngineDef::decode(&mut r).unwrap(), def);
        r.finish().unwrap();
    }
}
