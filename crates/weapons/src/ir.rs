//! Infrared intensity of an aircraft as a seeker sees it (GDD 15.7, C.11, ADR-46).
//!
//! `I = engine · f(AA) + skin · M²`, in units of the Kestrel's full-afterburner intensity. The
//! engine term runs from idle to military with the core thrust and from military to full
//! afterburner with the afterburner level; the aspect pattern `f` falls from 1 dead astern to the
//! side value at 90° and to the front value head on, linearly between.

use content::aircraft::IrSignature as IrSignatureDef;
use simmath::dmath;
use simmath::math::DVec3;

/// An aircraft's infrared signature (relative intensities).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IrSignature {
    /// Engine intensity at idle.
    pub engine_idle: f64,
    /// Engine intensity at military power.
    pub engine_mil: f64,
    /// Engine intensity in full afterburner.
    pub engine_ab: f64,
    /// Aspect factor seen from the side (90°).
    pub aspect_side: f64,
    /// Aspect factor seen from the front (180°).
    pub aspect_front: f64,
    /// Skin-heating intensity per Mach number squared.
    pub skin_per_mach2: f64,
}

impl IrSignature {
    /// The signature from the aircraft's content.
    #[must_use]
    pub fn from_def(d: &IrSignatureDef) -> Self {
        Self {
            engine_idle: f64::from(d.engine_idle),
            engine_mil: f64::from(d.engine_mil),
            engine_ab: f64::from(d.engine_ab),
            aspect_side: f64::from(d.aspect_side),
            aspect_front: f64::from(d.aspect_front),
            skin_per_mach2: f64::from(d.skin_per_mach2),
        }
    }
}

/// The engine term: `mil + (ab_max − mil) · ab` with the afterburner lit (`ab` in `(0, 1]`), else
/// `idle + (mil − idle) · core` (`core` the core thrust as a fraction of military, 0 to 1).
#[must_use]
pub fn engine_term(sig: &IrSignature, core: f64, ab: f64) -> f64 {
    if ab > 0.0 {
        sig.engine_mil + (sig.engine_ab - sig.engine_mil) * ab.min(1.0)
    } else {
        sig.engine_idle + (sig.engine_mil - sig.engine_idle) * core.clamp(0.0, 1.0)
    }
}

/// The aspect pattern at aspect angle `aa_rad` (0 dead astern, π head on).
#[must_use]
pub fn aspect_factor(sig: &IrSignature, aa_rad: f64) -> f64 {
    let x = aa_rad.clamp(0.0, core::f64::consts::PI) / core::f64::consts::FRAC_PI_2;
    if x <= 1.0 {
        1.0 + (sig.aspect_side - 1.0) * x
    } else {
        sig.aspect_side + (sig.aspect_front - sig.aspect_side) * (x - 1.0)
    }
}

/// The intensity seen from aspect angle `aa_rad` with the engine at `core` and `ab` and the
/// aircraft at Mach `mach`.
#[must_use]
pub fn intensity(sig: &IrSignature, core: f64, ab: f64, mach: f64, aa_rad: f64) -> f64 {
    engine_term(sig, core, ab) * aspect_factor(sig, aa_rad) + sig.skin_per_mach2 * mach * mach
}

/// The aspect angle of a target seen from a viewer: `cos AA = (−nose) · ŝ`, `ŝ` the unit vector
/// from the target to the viewer. 0 from dead astern, π head on; 0 if the two coincide.
#[must_use]
pub fn aspect_angle_rad(target_nose_bf: DVec3, target_pos_bf: DVec3, viewer_pos_bf: DVec3) -> f64 {
    let s = (viewer_pos_bf - target_pos_bf).normalized_or(-target_nose_bf);
    dmath::acos((-target_nose_bf.dot(s)).clamp(-1.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_content;

    fn kestrel() -> IrSignature {
        IrSignature::from_def(test_content::kestrel().ir.as_ref().expect("a signature"))
    }

    #[test]
    fn the_kestrels_signature() {
        let s = kestrel();
        assert_eq!(
            (s.engine_idle, s.engine_mil, s.engine_ab),
            (f64::from(0.1f32), 0.5, 1.0)
        );
        assert_eq!(
            (s.aspect_side, s.aspect_front),
            (f64::from(0.45f32), f64::from(0.15f32))
        );
        assert_eq!(s.skin_per_mach2, f64::from(0.05f32));
    }

    #[test]
    fn engine_and_aspect_worked_examples() {
        let s = kestrel();
        let near = |a: f64, b: f64| (a - b).abs() < 1e-7;
        // Idle, military, half and full afterburner.
        assert!(near(engine_term(&s, 0.0, 0.0), 0.1));
        assert!(near(engine_term(&s, 0.5, 0.0), 0.3));
        assert!(near(engine_term(&s, 1.0, 0.0), 0.5));
        assert!(near(engine_term(&s, 1.0, 0.5), 0.75));
        assert!(near(engine_term(&s, 1.0, 1.0), 1.0));
        // Astern, 45°, beam, 135°, head on.
        let deg = |d: f64| d.to_radians();
        assert!(near(aspect_factor(&s, 0.0), 1.0));
        assert!(near(aspect_factor(&s, deg(45.0)), 0.725));
        assert!(near(aspect_factor(&s, deg(90.0)), 0.45));
        assert!(near(aspect_factor(&s, deg(135.0)), 0.3));
        assert!(near(aspect_factor(&s, deg(180.0)), 0.15));
        // Military power seen from astern at Mach 0.8: 0.5 + 0.05 · 0.64 = 0.532.
        assert!(near(intensity(&s, 1.0, 0.0, 0.8, 0.0), 0.532));
        // Full afterburner from the beam at Mach 1.2: 0.45 + 0.072 = 0.522.
        assert!(near(intensity(&s, 1.0, 1.0, 1.2, deg(90.0)), 0.522));
    }

    #[test]
    fn aspect_angles() {
        let nose = DVec3::new(0.0, 1.0, 0.0);
        let t = DVec3::new(100.0, 100.0, 1_000.0);
        let aa = |v: DVec3| aspect_angle_rad(nose, t, t + v);
        assert!(aa(DVec3::new(0.0, -500.0, 0.0)).abs() < 1e-12);
        assert!((aa(DVec3::new(0.0, 500.0, 0.0)) - core::f64::consts::PI).abs() < 1e-12);
        assert!((aa(DVec3::new(300.0, 0.0, 0.0)) - core::f64::consts::FRAC_PI_2).abs() < 1e-12);
        assert!((aa(DVec3::new(100.0, -100.0, 0.0)) - core::f64::consts::FRAC_PI_4).abs() < 1e-12);
        assert_eq!(aspect_angle_rad(nose, t, t), 0.0);
    }
}
