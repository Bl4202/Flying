//! The weapon entry (kind 5): a gun or a short-range infrared missile (GDD 15, Appendix A.5 and
//! C.10, ADR-46).
//!
//! A weapon has a class and exactly the block of that class: a gun block for a gun, a missile
//! block for a missile. Aircraft refer to weapons by handle (the internal gun, the rail stores).
//! Floats are `f32`; the baker refuses authored values that `f32` cannot hold exactly.

use crate::codec::{Reader, Writer};
use crate::cpack::CpackError;
use crate::enums::{FuzeKind, GuidanceLaw, SeekerKind, WarheadKind, WeaponClass};
use crate::table::Curve;

/// One kind of round a gun can load.
#[derive(Clone, Debug, PartialEq)]
pub struct AmmoType {
    /// Name within the weapon (`hei`, ...).
    pub id: String,
    /// Every n-th round is a tracer (0 for none).
    pub tracer_every: u8,
}

/// A gun's data (GDD 15.2).
#[derive(Clone, Debug, PartialEq)]
pub struct GunBlock {
    /// Calibre, mm.
    pub caliber_mm: f32,
    /// Muzzle velocity, m/s.
    pub muzzle_mps: f32,
    /// Rate of fire, rounds per minute.
    pub rpm: f32,
    /// Time from the trigger to the full rate, s.
    pub spinup_s: f32,
    /// Dispersion (half-angle of the cone that holds every round), mrad.
    pub dispersion_mrad: f32,
    /// Mass of one round, kg.
    pub round_mass_kg: f32,
    /// Drag coefficient of a round against Mach number.
    pub drag_cd: Curve,
    /// Rounds the weapon holds when full.
    pub rounds: u16,
    /// The round types it can load.
    pub ammo_types: Vec<AmmoType>,
    /// Age after which a round is removed, s.
    pub max_age_s: f32,
    /// Speed below which a round is removed, m/s.
    pub min_speed_mps: f32,
}

/// A missile's rocket motor (single stage, constant thrust).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Motor {
    /// Thrust while it burns, N.
    pub thrust_n: f32,
    /// Burn time, s.
    pub burn_s: f32,
    /// Propellant mass, kg.
    pub propellant_kg: f32,
}

/// A missile's aerodynamics (GDD 15.5.2).
#[derive(Clone, Debug, PartialEq)]
pub struct MissileAero {
    /// Reference area, m^2.
    pub s_ref_m2: f32,
    /// Zero-lift drag coefficient against Mach number while the motor burns.
    pub cd0_boost: Curve,
    /// Zero-lift drag coefficient against Mach number after burnout.
    pub cd0_coast: Curve,
    /// Largest normal-force coefficient.
    pub cn_max: f32,
    /// Induced-drag factor.
    pub k_induced: f32,
}

/// A missile's first-order autopilot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Autopilot {
    /// Time constant, s.
    pub tau_s: f32,
    /// Largest lateral acceleration, g.
    pub g_max: f32,
    /// Speed below which it cannot manoeuvre, m/s.
    pub g_min_speed_mps: f32,
}

/// A missile's guidance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Guidance {
    /// The law.
    pub law: GuidanceLaw,
    /// Navigation constant N'.
    pub n_prime: f32,
}

/// A missile's seeker (GDD 15.7).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Seeker {
    /// The seeker type.
    pub kind: SeekerKind,
    /// Field of view while caged at the boresight, degrees (full angle).
    pub fov_deg: f32,
    /// Gimbal limit once locked, degrees (half angle).
    pub gimbal_deg: f32,
    /// Range at which a target of intensity `ref_intensity` is detected, m.
    pub lock_ref_range_m: f32,
    /// The intensity (relative to the Kestrel's afterburner) that `lock_ref_range_m` is quoted at.
    pub ref_intensity: f32,
    /// Largest aspect angle (from dead astern) at which it sees a target, degrees.
    pub max_aspect_deg: f32,
    /// Standard deviation of the line-of-sight noise, degrees.
    pub noise_deg: f32,
    /// Time of continuous tone before it locks, s.
    pub lock_time_s: f32,
}

/// A missile's fuze (GDD 15.9).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fuze {
    /// The fuze type.
    pub kind: FuzeKind,
    /// Distance flown before it arms, m.
    pub arm_dist_m: f32,
    /// Distance at which it triggers, m.
    pub trigger_radius_m: f32,
}

/// A missile's warhead (GDD 15.9).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Warhead {
    /// The warhead type.
    pub kind: WarheadKind,
    /// Explosive mass, kg.
    pub he_kg: f32,
    /// Distance beyond which it does no damage, m.
    pub lethal_radius_m: f32,
    /// Energy it delivers at zero distance, J.
    pub damage_j: f32,
}

/// What a launch needs (GDD 15.6).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Launch {
    /// Lowest launch-aircraft speed, m/s.
    pub min_speed_mps: f32,
    /// Largest load factor of the launch aircraft, g.
    pub max_g: f32,
    /// Largest angle of attack of the launch aircraft, degrees.
    pub max_alpha_deg: f32,
    /// Time before the motor lights and the guidance starts, s.
    pub safe_sep_s: f32,
    /// Shortest range at which a launch is allowed, m.
    pub min_range_m: f32,
}

/// When a missile gives up.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MissileLimits {
    /// Longest flight, s.
    pub max_time_s: f32,
    /// Speed below which a coasting missile self-destructs, m/s.
    pub min_speed_mps: f32,
    /// Time after its target is destroyed before it self-destructs, s.
    pub target_lost_s: f32,
}

/// How visible a missile is to the eye (GDD 16.6.2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MissileVisual {
    /// Effective size while the motor burns, m.
    pub l_eff_burning_m: f32,
    /// Effective size while coasting, m.
    pub l_eff_m: f32,
}

/// A missile's data (GDD 15.5 to 15.9).
#[derive(Clone, Debug, PartialEq)]
pub struct MissileBlock {
    /// The motor.
    pub motor: Motor,
    /// Aerodynamics.
    pub aero: MissileAero,
    /// Autopilot.
    pub autopilot: Autopilot,
    /// Guidance.
    pub guidance: Guidance,
    /// Seeker.
    pub seeker: Seeker,
    /// Fuze.
    pub fuze: Fuze,
    /// Warhead.
    pub warhead: Warhead,
    /// Launch constraints.
    pub launch: Launch,
    /// Self-destruct limits.
    pub limits: MissileLimits,
    /// Visibility.
    pub visual: MissileVisual,
}

/// What the sim knows about one weapon.
#[derive(Clone, Debug, PartialEq)]
pub struct WeaponDef {
    /// Locale key of the display name.
    pub name_key: String,
    /// Gun or missile.
    pub class: WeaponClass,
    /// Mass of the store, kg.
    pub mass_kg: f32,
    /// Length, m (0 when not authored).
    pub length_m: f32,
    /// Diameter, m (0 when not authored).
    pub diameter_m: f32,
    /// The gun's data; present exactly when `class` is [`WeaponClass::Gun`].
    pub gun: Option<GunBlock>,
    /// The missile's data; present exactly when `class` is [`WeaponClass::Srm`].
    pub missile: Option<MissileBlock>,
}

impl WeaponDef {
    pub(crate) fn encode(&self, w: &mut Writer<'_>) -> Result<(), CpackError> {
        self.check_class()?;
        w.str(&self.name_key)?;
        w.u8(self.class.code());
        w.f32s(&[self.mass_kg, self.length_m, self.diameter_m]);
        w.bool(self.gun.is_some());
        if let Some(g) = &self.gun {
            g.encode(w)?;
        }
        w.bool(self.missile.is_some());
        if let Some(m) = &self.missile {
            m.encode(w)?;
        }
        Ok(())
    }

    pub(crate) fn decode(r: &mut Reader<'_>) -> Result<Self, CpackError> {
        let name_key = r.str()?;
        let class =
            WeaponClass::from_code(r.u8()?).ok_or(CpackError::Record("unknown weapon class"))?;
        let (mass_kg, length_m, diameter_m) = (r.f32()?, r.f32()?, r.f32()?);
        let gun = if r.bool()? {
            Some(GunBlock::decode(r)?)
        } else {
            None
        };
        let missile = if r.bool()? {
            Some(MissileBlock::decode(r)?)
        } else {
            None
        };
        let def = Self {
            name_key,
            class,
            mass_kg,
            length_m,
            diameter_m,
            gun,
            missile,
        };
        def.check_class()?;
        Ok(def)
    }

    /// A gun has a gun block and no missile block; a missile the reverse.
    fn check_class(&self) -> Result<(), CpackError> {
        let ok = match self.class {
            WeaponClass::Gun => self.gun.is_some() && self.missile.is_none(),
            WeaponClass::Srm => self.missile.is_some() && self.gun.is_none(),
        };
        if ok {
            Ok(())
        } else {
            Err(CpackError::Record(
                "a weapon's blocks do not match its class",
            ))
        }
    }
}

impl GunBlock {
    fn encode(&self, w: &mut Writer<'_>) -> Result<(), CpackError> {
        w.f32s(&[
            self.caliber_mm,
            self.muzzle_mps,
            self.rpm,
            self.spinup_s,
            self.dispersion_mrad,
            self.round_mass_kg,
            self.max_age_s,
            self.min_speed_mps,
        ]);
        w.curve(&self.drag_cd)?;
        w.u16(self.rounds);
        w.count(self.ammo_types.len())?;
        for a in &self.ammo_types {
            w.str(&a.id)?;
            w.u8(a.tracer_every);
        }
        Ok(())
    }

    fn decode(r: &mut Reader<'_>) -> Result<Self, CpackError> {
        let (caliber_mm, muzzle_mps, rpm, spinup_s) = (r.f32()?, r.f32()?, r.f32()?, r.f32()?);
        let (dispersion_mrad, round_mass_kg) = (r.f32()?, r.f32()?);
        let (max_age_s, min_speed_mps) = (r.f32()?, r.f32()?);
        let drag_cd = r.curve()?;
        let rounds = r.u16()?;
        let ammo_types = (0..r.u8()?)
            .map(|_| {
                Ok(AmmoType {
                    id: r.str()?,
                    tracer_every: r.u8()?,
                })
            })
            .collect::<Result<Vec<_>, CpackError>>()?;
        Ok(Self {
            caliber_mm,
            muzzle_mps,
            rpm,
            spinup_s,
            dispersion_mrad,
            round_mass_kg,
            drag_cd,
            rounds,
            ammo_types,
            max_age_s,
            min_speed_mps,
        })
    }
}

impl MissileBlock {
    fn encode(&self, w: &mut Writer<'_>) -> Result<(), CpackError> {
        let (m, a, p, g) = (&self.motor, &self.aero, &self.autopilot, &self.guidance);
        w.f32s(&[m.thrust_n, m.burn_s, m.propellant_kg]);
        w.f32(a.s_ref_m2);
        w.curve(&a.cd0_boost)?;
        w.curve(&a.cd0_coast)?;
        w.f32s(&[a.cn_max, a.k_induced]);
        w.f32s(&[p.tau_s, p.g_max, p.g_min_speed_mps]);
        w.u8(g.law.code());
        w.f32(g.n_prime);
        let s = &self.seeker;
        w.u8(s.kind.code());
        w.f32s(&[
            s.fov_deg,
            s.gimbal_deg,
            s.lock_ref_range_m,
            s.ref_intensity,
            s.max_aspect_deg,
            s.noise_deg,
            s.lock_time_s,
        ]);
        let f = &self.fuze;
        w.u8(f.kind.code());
        w.f32s(&[f.arm_dist_m, f.trigger_radius_m]);
        let h = &self.warhead;
        w.u8(h.kind.code());
        w.f32s(&[h.he_kg, h.lethal_radius_m, h.damage_j]);
        let l = &self.launch;
        w.f32s(&[
            l.min_speed_mps,
            l.max_g,
            l.max_alpha_deg,
            l.safe_sep_s,
            l.min_range_m,
        ]);
        let x = &self.limits;
        w.f32s(&[x.max_time_s, x.min_speed_mps, x.target_lost_s]);
        w.f32s(&[self.visual.l_eff_burning_m, self.visual.l_eff_m]);
        Ok(())
    }

    fn decode(r: &mut Reader<'_>) -> Result<Self, CpackError> {
        let motor = Motor {
            thrust_n: r.f32()?,
            burn_s: r.f32()?,
            propellant_kg: r.f32()?,
        };
        let s_ref_m2 = r.f32()?;
        let (cd0_boost, cd0_coast) = (r.curve()?, r.curve()?);
        let aero = MissileAero {
            s_ref_m2,
            cd0_boost,
            cd0_coast,
            cn_max: r.f32()?,
            k_induced: r.f32()?,
        };
        let autopilot = Autopilot {
            tau_s: r.f32()?,
            g_max: r.f32()?,
            g_min_speed_mps: r.f32()?,
        };
        let guidance = Guidance {
            law: GuidanceLaw::from_code(r.u8()?)
                .ok_or(CpackError::Record("unknown guidance law"))?,
            n_prime: r.f32()?,
        };
        let seeker = Seeker {
            kind: SeekerKind::from_code(r.u8()?)
                .ok_or(CpackError::Record("unknown seeker type"))?,
            fov_deg: r.f32()?,
            gimbal_deg: r.f32()?,
            lock_ref_range_m: r.f32()?,
            ref_intensity: r.f32()?,
            max_aspect_deg: r.f32()?,
            noise_deg: r.f32()?,
            lock_time_s: r.f32()?,
        };
        let fuze = Fuze {
            kind: FuzeKind::from_code(r.u8()?).ok_or(CpackError::Record("unknown fuze type"))?,
            arm_dist_m: r.f32()?,
            trigger_radius_m: r.f32()?,
        };
        let warhead = Warhead {
            kind: WarheadKind::from_code(r.u8()?)
                .ok_or(CpackError::Record("unknown warhead type"))?,
            he_kg: r.f32()?,
            lethal_radius_m: r.f32()?,
            damage_j: r.f32()?,
        };
        let launch = Launch {
            min_speed_mps: r.f32()?,
            max_g: r.f32()?,
            max_alpha_deg: r.f32()?,
            safe_sep_s: r.f32()?,
            min_range_m: r.f32()?,
        };
        let limits = MissileLimits {
            max_time_s: r.f32()?,
            min_speed_mps: r.f32()?,
            target_lost_s: r.f32()?,
        };
        let visual = MissileVisual {
            l_eff_burning_m: r.f32()?,
            l_eff_m: r.f32()?,
        };
        Ok(Self {
            motor,
            aero,
            autopilot,
            guidance,
            seeker,
            fuze,
            warhead,
            launch,
            limits,
            visual,
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::codec::Strings;

    /// The GS-27 of GDD C.1 (the values of `content/core/weapons/gs27.json`).
    pub(crate) fn gun() -> WeaponDef {
        WeaponDef {
            name_key: "wpn.gs27.name".into(),
            class: WeaponClass::Gun,
            mass_kg: 100.0,
            length_m: 0.0,
            diameter_m: 0.0,
            gun: Some(GunBlock {
                caliber_mm: 27.0,
                muzzle_mps: 1025.0,
                rpm: 1700.0,
                spinup_s: 0.0,
                dispersion_mrad: 1.5,
                round_mass_kg: 0.26,
                drag_cd: Curve::from_points(&[
                    (0.0, 0.25),
                    (0.8, 0.25),
                    (1.2, 0.38),
                    (2.0, 0.31),
                    (3.0, 0.26),
                ]),
                rounds: 150,
                ammo_types: vec![AmmoType {
                    id: "hei".into(),
                    tracer_every: 4,
                }],
                max_age_s: 3.0,
                min_speed_mps: 300.0,
            }),
            missile: None,
        }
    }

    /// The Dart-1 of GDD C.10 (the values of `content/core/weapons/dart1.json`).
    pub(crate) fn missile() -> WeaponDef {
        WeaponDef {
            name_key: "wpn.dart1.name".into(),
            class: WeaponClass::Srm,
            mass_kg: 85.0,
            length_m: 2.9,
            diameter_m: 0.125,
            gun: None,
            missile: Some(MissileBlock {
                motor: Motor {
                    thrust_n: 11_000.0,
                    burn_s: 2.5,
                    propellant_kg: 11.5,
                },
                aero: MissileAero {
                    s_ref_m2: 0.01227,
                    cd0_boost: Curve::from_points(&[
                        (0.0, 0.31),
                        (1.2, 0.44),
                        (2.5, 0.34),
                        (4.0, 0.28),
                    ]),
                    cd0_coast: Curve::from_points(&[
                        (0.0, 0.41),
                        (1.2, 0.56),
                        (2.5, 0.44),
                        (4.0, 0.38),
                    ]),
                    cn_max: 12.0,
                    k_induced: 0.047,
                },
                autopilot: Autopilot {
                    tau_s: 0.25,
                    g_max: 30.0,
                    g_min_speed_mps: 150.0,
                },
                guidance: Guidance {
                    law: GuidanceLaw::Pn,
                    n_prime: 4.0,
                },
                seeker: Seeker {
                    kind: SeekerKind::IrRear,
                    fov_deg: 4.0,
                    gimbal_deg: 30.0,
                    lock_ref_range_m: 6000.0,
                    ref_intensity: 1.0,
                    max_aspect_deg: 35.0,
                    noise_deg: 0.1,
                    lock_time_s: 0.5,
                },
                fuze: Fuze {
                    kind: FuzeKind::Proximity,
                    arm_dist_m: 300.0,
                    trigger_radius_m: 8.0,
                },
                warhead: Warhead {
                    kind: WarheadKind::Frag,
                    he_kg: 9.0,
                    lethal_radius_m: 8.0,
                    damage_j: 2_548_000.0,
                },
                launch: Launch {
                    min_speed_mps: 80.0,
                    max_g: 7.5,
                    max_alpha_deg: 25.0,
                    safe_sep_s: 0.5,
                    min_range_m: 450.0,
                },
                limits: MissileLimits {
                    max_time_s: 25.0,
                    min_speed_mps: 150.0,
                    target_lost_s: 2.0,
                },
                visual: MissileVisual {
                    l_eff_burning_m: 3.0,
                    l_eff_m: 0.5,
                },
            }),
        }
    }

    /// Encodes `def` and returns the record and the string table it needs.
    fn encode(def: &WeaponDef) -> Result<(Vec<u8>, String), CpackError> {
        let mut strings = Strings::default();
        let mut w = Writer::new(&mut strings);
        def.encode(&mut w)?;
        let rec = w.finish();
        Ok((rec, String::from_utf8(strings.bytes).unwrap()))
    }

    fn round_trip(def: &WeaponDef) -> Result<WeaponDef, CpackError> {
        let (rec, text) = encode(def)?;
        let mut r = Reader::new(&rec, &text);
        let back = WeaponDef::decode(&mut r)?;
        r.finish()?;
        Ok(back)
    }

    #[test]
    fn a_gun_and_a_missile_round_trip() {
        assert_eq!(round_trip(&gun()).unwrap(), gun());
        assert_eq!(round_trip(&missile()).unwrap(), missile());
    }

    #[test]
    fn a_weapon_with_the_wrong_blocks_is_refused_by_the_writer_and_the_reader() {
        // The writer refuses a gun with no gun block, a missile with a gun block, and both blocks.
        let mut no_block = gun();
        no_block.gun = None;
        assert!(matches!(encode(&no_block), Err(CpackError::Record(_))));
        let mut wrong_class = gun();
        wrong_class.class = WeaponClass::Srm;
        assert!(matches!(encode(&wrong_class), Err(CpackError::Record(_))));
        let mut both = missile();
        both.gun = gun().gun;
        assert!(matches!(encode(&both), Err(CpackError::Record(_))));

        // The reader refuses the class byte edited after a valid write. The class is the byte
        // after the 8-byte name-key reference.
        let (mut rec, text) = encode(&gun()).unwrap();
        rec[8] = WeaponClass::Srm.code();
        let mut r = Reader::new(&rec, &text);
        assert!(matches!(
            WeaponDef::decode(&mut r),
            Err(CpackError::Record(_))
        ));
        rec[8] = 0;
        let mut r = Reader::new(&rec, &text);
        assert!(matches!(
            WeaponDef::decode(&mut r),
            Err(CpackError::Record(_))
        ));
    }

    #[test]
    fn unknown_codes_inside_a_missile_are_refused() {
        let (rec, text) = encode(&missile()).unwrap();
        // Where the four code bytes sit in the Dart-1's record: the name key (8), the class (1),
        // three floats (12), the two block flags (2), then the motor (12), the reference area (4),
        // the two drag curves (2 + 4 * 8 each), the normal force and induced drag (8) and the
        // autopilot (12) come before the guidance law; each later code follows the floats of the
        // block before it.
        let law = 8 + 1 + 12 + 2 + 12 + 4 + 2 * (2 + 4 * 8) + 8 + 12;
        let seeker = law + 1 + 4;
        let fuze = seeker + 1 + 7 * 4;
        let warhead = fuze + 1 + 2 * 4;
        for (name, at) in [
            ("guidance law", law),
            ("seeker", seeker),
            ("fuze", fuze),
            ("warhead", warhead),
        ] {
            assert_eq!(rec[at], 1, "{name}: the offset is wrong");
            let mut bad = rec.clone();
            bad[at] = 99;
            let mut r = Reader::new(&bad, &text);
            assert!(
                matches!(WeaponDef::decode(&mut r), Err(CpackError::Record(_))),
                "{name}"
            );
        }
        // No single-byte edit panics the reader.
        for i in 0..rec.len() {
            let mut bad = rec.clone();
            bad[i] = 0xFF;
            let mut r = Reader::new(&bad, &text);
            let _ = WeaponDef::decode(&mut r).and_then(|_| r.finish());
        }
    }

    #[test]
    fn a_truncated_record_is_refused() {
        let (rec, text) = encode(&missile()).unwrap();
        let cut = &rec[..rec.len() - 8];
        let mut r = Reader::new(cut, &text);
        assert!(WeaponDef::decode(&mut r).and_then(|_| r.finish()).is_err());
    }
}
