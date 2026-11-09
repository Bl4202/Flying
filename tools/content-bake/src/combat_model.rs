//! The authoring formats of the combat data (ADR-46, ADR-47): `weapon/1`, `ai_skill/1` and the
//! additions to `aircraft/1` (the internal gun, the launch rails, the signatures and the damage
//! data).
//!
//! The rules of [`crate::model`] hold here too: strict serde types that are also the source of
//! the JSON Schemas, unit-suffixed keys, unknown keys refused.

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize};

use crate::model::{AircraftFile, CurveFile, f, f3, s3};
use content::aircraft::{CapsuleDef, DamageDef, GunInstall, IrSignature, Rail};
use content::skill::{
    TACTIC_BREAK_TURN, TACTIC_EXTEND, TACTIC_JINK, TACTIC_LAG_PURSUIT, TACTIC_LEAD_PURSUIT,
    TACTIC_PURE_PURSUIT,
};
use content::weapon::{
    AmmoType, Autopilot, Fuze, Guidance, GunBlock, Launch, MissileAero, MissileBlock,
    MissileLimits, MissileVisual, Motor, Seeker, Warhead,
};
use content::{
    AircraftDef, ContentId, FuzeKind, GuidanceLaw, SeekerKind, SkillDef, WarheadKind, WeaponClass,
    WeaponDef,
};

/// Tag of the weapon format.
pub const WEAPON_SCHEMA: &str = "weapon/1";
/// Tag of the AI skill format.
pub const SKILL_SCHEMA: &str = "ai_skill/1";

/// The tactics a skill may list, in the bit order the pack stores them in.
pub const TACTICS: &[(&str, u8)] = &[
    ("pure_pursuit", TACTIC_PURE_PURSUIT),
    ("lead_pursuit", TACTIC_LEAD_PURSUIT),
    ("lag_pursuit", TACTIC_LAG_PURSUIT),
    ("break_turn", TACTIC_BREAK_TURN),
    ("jink", TACTIC_JINK),
    ("extend", TACTIC_EXTEND),
];

fn names_schema(names: &[&str]) -> Schema {
    json_schema!({ "type": "string", "enum": names })
}

fn class_schema(_: &mut SchemaGenerator) -> Schema {
    names_schema(
        &WeaponClass::ALL
            .iter()
            .map(|c| c.name())
            .collect::<Vec<_>>(),
    )
}

fn law_schema(_: &mut SchemaGenerator) -> Schema {
    names_schema(
        &GuidanceLaw::ALL
            .iter()
            .map(|c| c.name())
            .collect::<Vec<_>>(),
    )
}

fn seeker_schema(_: &mut SchemaGenerator) -> Schema {
    names_schema(&SeekerKind::ALL.iter().map(|c| c.name()).collect::<Vec<_>>())
}

fn fuze_schema(_: &mut SchemaGenerator) -> Schema {
    names_schema(&FuzeKind::ALL.iter().map(|c| c.name()).collect::<Vec<_>>())
}

fn warhead_schema(_: &mut SchemaGenerator) -> Schema {
    names_schema(
        &WarheadKind::ALL
            .iter()
            .map(|c| c.name())
            .collect::<Vec<_>>(),
    )
}

fn tactic_schema(_: &mut SchemaGenerator) -> Schema {
    json_schema!({
        "type": "array",
        "items": { "type": "string", "enum": TACTICS.iter().map(|t| t.0).collect::<Vec<_>>() },
        "minItems": 1,
        "maxItems": 6
    })
}

// ---------------------------------------------------------------------------------------------
// weapon/1

/// One weapon: a gun or a short-range missile (GDD 15, C.1, C.10).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WeaponFile {
    /// Format tag, always `weapon/1`.
    #[schemars(extend("const" = "weapon/1"))]
    pub schema: String,
    /// `namespace:type/name`, with namespace the pack id and type `wpn`.
    pub id: String,
    /// Key of the display name in the locale table.
    pub name_key: String,
    /// Gun or short-range missile.
    #[schemars(schema_with = "class_schema")]
    pub class: String,
    /// Mass of the store.
    #[schemars(extend("x-unit" = "kg"), range(min = 0.01, max = 10000.0))]
    pub mass_kg: f64,
    /// Length (leave out for a gun).
    #[serde(default)]
    #[schemars(extend("x-unit" = "m"), range(min = 0.0, max = 20.0))]
    pub length_m: f64,
    /// Diameter (leave out for a gun).
    #[serde(default)]
    #[schemars(extend("x-unit" = "m"), range(min = 0.0, max = 2.0))]
    pub diameter_m: f64,
    /// The gun's data; required for a gun, refused for a missile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gun: Option<GunFile>,
    /// The missile's data; required for a missile, refused for a gun.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub missile: Option<MissileFile>,
}

/// A gun (GDD 15.2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GunFile {
    /// Calibre.
    #[schemars(extend("x-unit" = "mm"), range(min = 1.0, max = 120.0))]
    pub caliber_mm: f64,
    /// Muzzle velocity.
    #[schemars(extend("x-unit" = "m/s"), range(min = 100.0, max = 2000.0))]
    pub muzzle_mps: f64,
    /// Rate of fire, rounds per minute.
    #[schemars(extend("x-unit" = "rounds/min"), range(min = 100.0, max = 10000.0))]
    pub rpm: f64,
    /// Time from the trigger to the full rate.
    #[schemars(extend("x-unit" = "s"), range(min = 0.0, max = 5.0))]
    pub spinup_s: f64,
    /// Dispersion: the standard deviation of the Gaussian spread about the gun axis, on each of
    /// two axes (GDD 15.4.1).
    #[schemars(extend("x-unit" = "mrad"), range(min = 0.0, max = 20.0))]
    pub dispersion_mrad: f64,
    /// Mass of one round.
    #[schemars(extend("x-unit" = "kg"), range(min = 0.001, max = 5.0))]
    pub round_mass_kg: f64,
    /// Drag coefficient of a round against Mach number.
    pub drag_cd: CurveFile,
    /// Rounds the weapon holds when full.
    #[schemars(range(min = 1, max = 5000))]
    pub rounds: u16,
    /// The round types it can load.
    #[schemars(length(min = 1, max = 4))]
    pub ammo_types: Vec<AmmoFile>,
    /// Age after which a round is removed.
    #[schemars(extend("x-unit" = "s"), range(min = 0.1, max = 30.0))]
    pub max_age_s: f64,
    /// Speed below which a round is removed.
    #[schemars(extend("x-unit" = "m/s"), range(min = 0.0, max = 1000.0))]
    pub min_speed_mps: f64,
}

/// One round type.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AmmoFile {
    /// Name within the weapon.
    #[schemars(length(min = 1))]
    pub id: String,
    /// Every n-th round is a tracer (0 for none).
    #[schemars(range(min = 0, max = 100))]
    pub tracer_every: u8,
}

/// A short-range missile (GDD 15.5 to 15.9).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MissileFile {
    /// The rocket motor.
    pub motor: MotorFile,
    /// Aerodynamics.
    pub aero: MissileAeroFile,
    /// Autopilot.
    pub autopilot: AutopilotFile,
    /// Guidance.
    pub guidance: GuidanceFile,
    /// Seeker.
    pub seeker: SeekerFile,
    /// Fuze.
    pub fuze: FuzeFile,
    /// Warhead.
    pub warhead: WarheadFile,
    /// Launch constraints.
    pub launch: LaunchFile,
    /// When the missile gives up.
    pub limits: MissileLimitsFile,
    /// Visibility to the eye.
    pub visual: MissileVisualFile,
}

/// A single-stage constant-thrust motor.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MotorFile {
    /// Thrust while the motor burns.
    #[schemars(extend("x-unit" = "N"), range(min = 100.0, max = 200_000.0))]
    pub thrust_n: f64,
    /// Burn time.
    #[schemars(extend("x-unit" = "s"), range(min = 0.1, max = 60.0))]
    pub burn_s: f64,
    /// Propellant mass.
    #[schemars(extend("x-unit" = "kg"), range(min = 0.1, max = 1000.0))]
    pub propellant_kg: f64,
}

/// A missile's aerodynamics.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MissileAeroFile {
    /// Reference area.
    #[schemars(extend("x-unit" = "m2"), range(min = 0.0005, max = 1.0))]
    pub s_ref_m2: f64,
    /// Zero-lift drag coefficient against Mach number while the motor burns.
    pub cd0_boost: CurveFile,
    /// Zero-lift drag coefficient against Mach number after burnout.
    pub cd0_coast: CurveFile,
    /// Largest normal-force coefficient.
    #[schemars(range(min = 1.0, max = 50.0))]
    pub cn_max: f64,
    /// Induced-drag factor.
    #[schemars(range(min = 0.0, max = 1.0))]
    pub k_induced: f64,
}

/// A missile's autopilot: a first-order lag with a lateral-acceleration limit.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AutopilotFile {
    /// Time constant.
    #[schemars(extend("x-unit" = "s"), range(min = 0.01, max = 5.0))]
    pub tau_s: f64,
    /// Largest lateral acceleration.
    #[schemars(extend("x-unit" = "g"), range(min = 1.0, max = 100.0))]
    pub g_max: f64,
    /// Speed below which the missile cannot manoeuvre.
    #[schemars(extend("x-unit" = "m/s"), range(min = 0.0, max = 500.0))]
    pub g_min_speed_mps: f64,
}

/// A missile's guidance.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GuidanceFile {
    /// The law.
    #[schemars(schema_with = "law_schema")]
    pub law: String,
    /// Navigation constant N'.
    #[schemars(range(min = 2.0, max = 6.0))]
    pub n_prime: f64,
}

/// A missile's seeker (GDD 15.7).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SeekerFile {
    /// The seeker type.
    #[serde(rename = "type")]
    #[schemars(schema_with = "seeker_schema")]
    pub kind: String,
    /// Field of view while caged at the boresight (full angle).
    #[schemars(extend("x-unit" = "deg"), range(min = 1.0, max = 60.0))]
    pub fov_deg: f64,
    /// Gimbal limit once locked (half angle).
    #[schemars(extend("x-unit" = "deg"), range(min = 1.0, max = 90.0))]
    pub gimbal_deg: f64,
    /// Range at which a target of intensity `ref_intensity` is detected.
    #[schemars(extend("x-unit" = "m"), range(min = 100.0, max = 50000.0))]
    pub lock_ref_range_m: f64,
    /// The intensity (relative to the Kestrel's afterburner) that `lock_ref_range_m` is quoted at.
    #[schemars(range(min = 0.01, max = 100.0))]
    pub ref_intensity: f64,
    /// Largest aspect angle, from dead astern, at which the seeker sees a target.
    #[schemars(extend("x-unit" = "deg"), range(min = 1.0, max = 180.0))]
    pub max_aspect_deg: f64,
    /// Standard deviation of the line-of-sight noise.
    #[schemars(extend("x-unit" = "deg"), range(min = 0.0, max = 2.0))]
    pub noise_deg: f64,
    /// Time of continuous tone before the seeker locks.
    #[schemars(extend("x-unit" = "s"), range(min = 0.0, max = 3.0))]
    pub lock_time_s: f64,
}

/// A missile's fuze.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FuzeFile {
    /// The fuze type.
    #[serde(rename = "type")]
    #[schemars(schema_with = "fuze_schema")]
    pub kind: String,
    /// Distance flown before the fuze arms.
    #[schemars(extend("x-unit" = "m"), range(min = 0.0, max = 5000.0))]
    pub arm_dist_m: f64,
    /// Distance at which it triggers.
    #[schemars(extend("x-unit" = "m"), range(min = 0.5, max = 50.0))]
    pub trigger_radius_m: f64,
}

/// A missile's warhead.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WarheadFile {
    /// The warhead type.
    #[schemars(schema_with = "warhead_schema")]
    pub kind: String,
    /// Explosive mass.
    #[schemars(extend("x-unit" = "kg"), range(min = 0.1, max = 500.0))]
    pub he_kg: f64,
    /// Distance beyond which it does no damage.
    #[schemars(extend("x-unit" = "m"), range(min = 0.5, max = 100.0))]
    pub lethal_radius_m: f64,
    /// Energy it delivers at zero distance.
    #[schemars(extend("x-unit" = "J"), range(min = 1000.0, max = 1.0e9))]
    pub damage_j: f64,
}

/// What a launch needs.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LaunchFile {
    /// Lowest speed of the launch aircraft.
    #[schemars(extend("x-unit" = "m/s"), range(min = 0.0, max = 500.0))]
    pub min_speed_mps: f64,
    /// Largest load factor of the launch aircraft.
    #[schemars(extend("x-unit" = "g"), range(min = 1.0, max = 20.0))]
    pub max_g: f64,
    /// Largest angle of attack of the launch aircraft.
    #[schemars(extend("x-unit" = "deg"), range(min = 1.0, max = 90.0))]
    pub max_alpha_deg: f64,
    /// Time after launch before the fuze may arm (the motor lights at launch).
    #[schemars(extend("x-unit" = "s"), range(min = 0.0, max = 5.0))]
    pub safe_sep_s: f64,
    /// Shortest range at which a launch is allowed.
    #[schemars(extend("x-unit" = "m"), range(min = 0.0, max = 5000.0))]
    pub min_range_m: f64,
}

/// When a missile gives up.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MissileLimitsFile {
    /// Longest flight.
    #[schemars(extend("x-unit" = "s"), range(min = 1.0, max = 300.0))]
    pub max_time_s: f64,
    /// Speed below which a coasting missile self-destructs.
    #[schemars(extend("x-unit" = "m/s"), range(min = 0.0, max = 500.0))]
    pub min_speed_mps: f64,
    /// Time after its target is destroyed before it self-destructs.
    #[schemars(extend("x-unit" = "s"), range(min = 0.0, max = 30.0))]
    pub target_lost_s: f64,
}

/// How visible a missile is to the eye.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MissileVisualFile {
    /// Effective size while the motor burns.
    #[schemars(extend("x-unit" = "m"), range(min = 0.01, max = 50.0))]
    pub l_eff_burning_m: f64,
    /// Effective size while coasting.
    #[schemars(extend("x-unit" = "m"), range(min = 0.01, max = 50.0))]
    pub l_eff_m: f64,
}

impl WeaponFile {
    /// The runtime definition; `None` if a name does not resolve (the checks report those first).
    #[must_use]
    pub fn to_def(&self) -> Option<WeaponDef> {
        Some(WeaponDef {
            name_key: self.name_key.clone(),
            class: WeaponClass::from_name(&self.class)?,
            mass_kg: self.mass_kg as f32,
            length_m: self.length_m as f32,
            diameter_m: self.diameter_m as f32,
            gun: self.gun.as_ref().map(GunFile::to_block),
            missile: match &self.missile {
                None => None,
                Some(m) => Some(m.to_block()?),
            },
        })
    }

    /// The authoring form of a baked definition.
    #[must_use]
    pub fn from_def(id: &str, d: &WeaponDef) -> Self {
        Self {
            schema: WEAPON_SCHEMA.to_owned(),
            id: id.to_owned(),
            name_key: d.name_key.clone(),
            class: d.class.name().to_owned(),
            mass_kg: f(d.mass_kg),
            length_m: f(d.length_m),
            diameter_m: f(d.diameter_m),
            gun: d.gun.as_ref().map(GunFile::from_block),
            missile: d.missile.as_ref().map(MissileFile::from_block),
        }
    }
}

impl GunFile {
    fn to_block(&self) -> GunBlock {
        GunBlock {
            caliber_mm: self.caliber_mm as f32,
            muzzle_mps: self.muzzle_mps as f32,
            rpm: self.rpm as f32,
            spinup_s: self.spinup_s as f32,
            dispersion_mrad: self.dispersion_mrad as f32,
            round_mass_kg: self.round_mass_kg as f32,
            drag_cd: self.drag_cd.to_curve(),
            rounds: self.rounds,
            ammo_types: self
                .ammo_types
                .iter()
                .map(|a| AmmoType {
                    id: a.id.clone(),
                    tracer_every: a.tracer_every,
                })
                .collect(),
            max_age_s: self.max_age_s as f32,
            min_speed_mps: self.min_speed_mps as f32,
        }
    }

    fn from_block(g: &GunBlock) -> Self {
        Self {
            caliber_mm: f(g.caliber_mm),
            muzzle_mps: f(g.muzzle_mps),
            rpm: f(g.rpm),
            spinup_s: f(g.spinup_s),
            dispersion_mrad: f(g.dispersion_mrad),
            round_mass_kg: f(g.round_mass_kg),
            drag_cd: CurveFile::from_curve("mach", &g.drag_cd),
            rounds: g.rounds,
            ammo_types: g
                .ammo_types
                .iter()
                .map(|a| AmmoFile {
                    id: a.id.clone(),
                    tracer_every: a.tracer_every,
                })
                .collect(),
            max_age_s: f(g.max_age_s),
            min_speed_mps: f(g.min_speed_mps),
        }
    }
}

impl MissileFile {
    fn to_block(&self) -> Option<MissileBlock> {
        let (m, a, p, g) = (&self.motor, &self.aero, &self.autopilot, &self.guidance);
        let (s, z, h) = (&self.seeker, &self.fuze, &self.warhead);
        let (l, x, v) = (&self.launch, &self.limits, &self.visual);
        Some(MissileBlock {
            motor: Motor {
                thrust_n: m.thrust_n as f32,
                burn_s: m.burn_s as f32,
                propellant_kg: m.propellant_kg as f32,
            },
            aero: MissileAero {
                s_ref_m2: a.s_ref_m2 as f32,
                cd0_boost: a.cd0_boost.to_curve(),
                cd0_coast: a.cd0_coast.to_curve(),
                cn_max: a.cn_max as f32,
                k_induced: a.k_induced as f32,
            },
            autopilot: Autopilot {
                tau_s: p.tau_s as f32,
                g_max: p.g_max as f32,
                g_min_speed_mps: p.g_min_speed_mps as f32,
            },
            guidance: Guidance {
                law: GuidanceLaw::from_name(&g.law)?,
                n_prime: g.n_prime as f32,
            },
            seeker: Seeker {
                kind: SeekerKind::from_name(&s.kind)?,
                fov_deg: s.fov_deg as f32,
                gimbal_deg: s.gimbal_deg as f32,
                lock_ref_range_m: s.lock_ref_range_m as f32,
                ref_intensity: s.ref_intensity as f32,
                max_aspect_deg: s.max_aspect_deg as f32,
                noise_deg: s.noise_deg as f32,
                lock_time_s: s.lock_time_s as f32,
            },
            fuze: Fuze {
                kind: FuzeKind::from_name(&z.kind)?,
                arm_dist_m: z.arm_dist_m as f32,
                trigger_radius_m: z.trigger_radius_m as f32,
            },
            warhead: Warhead {
                kind: WarheadKind::from_name(&h.kind)?,
                he_kg: h.he_kg as f32,
                lethal_radius_m: h.lethal_radius_m as f32,
                damage_j: h.damage_j as f32,
            },
            launch: Launch {
                min_speed_mps: l.min_speed_mps as f32,
                max_g: l.max_g as f32,
                max_alpha_deg: l.max_alpha_deg as f32,
                safe_sep_s: l.safe_sep_s as f32,
                min_range_m: l.min_range_m as f32,
            },
            limits: MissileLimits {
                max_time_s: x.max_time_s as f32,
                min_speed_mps: x.min_speed_mps as f32,
                target_lost_s: x.target_lost_s as f32,
            },
            visual: MissileVisual {
                l_eff_burning_m: v.l_eff_burning_m as f32,
                l_eff_m: v.l_eff_m as f32,
            },
        })
    }

    fn from_block(b: &MissileBlock) -> Self {
        let (m, a, p, g) = (&b.motor, &b.aero, &b.autopilot, &b.guidance);
        let (s, z, h) = (&b.seeker, &b.fuze, &b.warhead);
        let (l, x, v) = (&b.launch, &b.limits, &b.visual);
        Self {
            motor: MotorFile {
                thrust_n: f(m.thrust_n),
                burn_s: f(m.burn_s),
                propellant_kg: f(m.propellant_kg),
            },
            aero: MissileAeroFile {
                s_ref_m2: f(a.s_ref_m2),
                cd0_boost: CurveFile::from_curve("mach", &a.cd0_boost),
                cd0_coast: CurveFile::from_curve("mach", &a.cd0_coast),
                cn_max: f(a.cn_max),
                k_induced: f(a.k_induced),
            },
            autopilot: AutopilotFile {
                tau_s: f(p.tau_s),
                g_max: f(p.g_max),
                g_min_speed_mps: f(p.g_min_speed_mps),
            },
            guidance: GuidanceFile {
                law: g.law.name().to_owned(),
                n_prime: f(g.n_prime),
            },
            seeker: SeekerFile {
                kind: s.kind.name().to_owned(),
                fov_deg: f(s.fov_deg),
                gimbal_deg: f(s.gimbal_deg),
                lock_ref_range_m: f(s.lock_ref_range_m),
                ref_intensity: f(s.ref_intensity),
                max_aspect_deg: f(s.max_aspect_deg),
                noise_deg: f(s.noise_deg),
                lock_time_s: f(s.lock_time_s),
            },
            fuze: FuzeFile {
                kind: z.kind.name().to_owned(),
                arm_dist_m: f(z.arm_dist_m),
                trigger_radius_m: f(z.trigger_radius_m),
            },
            warhead: WarheadFile {
                kind: h.kind.name().to_owned(),
                he_kg: f(h.he_kg),
                lethal_radius_m: f(h.lethal_radius_m),
                damage_j: f(h.damage_j),
            },
            launch: LaunchFile {
                min_speed_mps: f(l.min_speed_mps),
                max_g: f(l.max_g),
                max_alpha_deg: f(l.max_alpha_deg),
                safe_sep_s: f(l.safe_sep_s),
                min_range_m: f(l.min_range_m),
            },
            limits: MissileLimitsFile {
                max_time_s: f(x.max_time_s),
                min_speed_mps: f(x.min_speed_mps),
                target_lost_s: f(x.target_lost_s),
            },
            visual: MissileVisualFile {
                l_eff_burning_m: f(v.l_eff_burning_m),
                l_eff_m: f(v.l_eff_m),
            },
        }
    }
}

// ---------------------------------------------------------------------------------------------
// ai_skill/1

/// One AI skill level (GDD 19.6, ADR-47).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SkillFile {
    /// Format tag, always `ai_skill/1`.
    #[schemars(extend("const" = "ai_skill/1"))]
    pub schema: String,
    /// `namespace:type/name`, with namespace the pack id and type `skl`.
    pub id: String,
    /// Key of the display name in the locale table.
    pub name_key: String,
    /// Position in the ladder, 1 for the lowest. Ranks in a pack are 1 to n, each once.
    #[schemars(range(min = 1, max = 16))]
    pub rank: u8,
    /// True if the pilot fires its gun.
    pub fires_gun: bool,
    /// Delay between something becoming visible and the pilot acting on it.
    #[schemars(extend("x-unit" = "s"), range(min = 0.0, max = 10.0))]
    pub perception_delay_s: f64,
    /// Probability per decision that the pilot notices an incoming missile it can see.
    #[schemars(range(min = 0.0, max = 1.0))]
    pub notice_prob: f64,
    /// Fraction of the aircraft's load-factor limit the pilot will pull.
    #[schemars(range(min = 0.0, max = 1.0))]
    pub g_frac: f64,
    /// Standard deviation of the pilot's gun aim error.
    #[schemars(extend("x-unit" = "mrad"), range(min = 0.0, max = 100.0))]
    pub gun_aim_sigma_mrad: f64,
    /// Delay before the pilot reacts to being fired at.
    #[schemars(extend("x-unit" = "s"), range(min = 0.0, max = 10.0))]
    pub defend_delay_s: f64,
    /// Standard deviation of the timing of a defensive break.
    #[schemars(extend("x-unit" = "s"), range(min = 0.0, max = 10.0))]
    pub break_timing_sigma_s: f64,
    /// Lowest height above the ground the pilot will fly at.
    #[schemars(extend("x-unit" = "m"), range(min = 0.0, max = 5000.0))]
    pub terrain_min_agl_m: f64,
    /// Shortest time the pilot keeps a decision before changing it.
    #[schemars(extend("x-unit" = "s"), range(min = 0.0, max = 30.0))]
    pub commitment_s: f64,
    /// Calibrated airspeed below which the pilot extends (0 for never).
    #[schemars(extend("x-unit" = "m/s"), range(min = 0.0, max = 500.0))]
    pub extend_below_cas_mps: f64,
    /// Calibrated airspeed at which the pilot stops extending.
    #[schemars(extend("x-unit" = "m/s"), range(min = 0.0, max = 500.0))]
    pub extend_until_cas_mps: f64,
    /// The tactics the pilot knows.
    #[schemars(schema_with = "tactic_schema")]
    pub tactics: Vec<String>,
}

impl SkillFile {
    /// The runtime definition; `None` if a tactic name is unknown (the checks report those first).
    #[must_use]
    pub fn to_def(&self) -> Option<SkillDef> {
        let mut tactics = 0_u8;
        for name in &self.tactics {
            tactics |= TACTICS.iter().find(|t| t.0 == name)?.1;
        }
        Some(SkillDef {
            name_key: self.name_key.clone(),
            rank: self.rank,
            fires_gun: self.fires_gun,
            perception_delay_s: self.perception_delay_s as f32,
            notice_prob: self.notice_prob as f32,
            g_frac: self.g_frac as f32,
            gun_aim_sigma_mrad: self.gun_aim_sigma_mrad as f32,
            defend_delay_s: self.defend_delay_s as f32,
            break_timing_sigma_s: self.break_timing_sigma_s as f32,
            terrain_min_agl_m: self.terrain_min_agl_m as f32,
            commitment_s: self.commitment_s as f32,
            extend_below_cas_mps: self.extend_below_cas_mps as f32,
            extend_until_cas_mps: self.extend_until_cas_mps as f32,
            tactics,
        })
    }

    /// The authoring form of a baked definition (tactics in the order [`TACTICS`] lists them).
    #[must_use]
    pub fn from_def(id: &str, d: &SkillDef) -> Self {
        Self {
            schema: SKILL_SCHEMA.to_owned(),
            id: id.to_owned(),
            name_key: d.name_key.clone(),
            rank: d.rank,
            fires_gun: d.fires_gun,
            perception_delay_s: f(d.perception_delay_s),
            notice_prob: f(d.notice_prob),
            g_frac: f(d.g_frac),
            gun_aim_sigma_mrad: f(d.gun_aim_sigma_mrad),
            defend_delay_s: f(d.defend_delay_s),
            break_timing_sigma_s: f(d.break_timing_sigma_s),
            terrain_min_agl_m: f(d.terrain_min_agl_m),
            commitment_s: f(d.commitment_s),
            extend_below_cas_mps: f(d.extend_below_cas_mps),
            extend_until_cas_mps: f(d.extend_until_cas_mps),
            tactics: TACTICS
                .iter()
                .filter(|t| d.tactics & t.1 != 0)
                .map(|t| t.0.to_owned())
                .collect(),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The additions to aircraft/1

/// The internal weapons of an aircraft.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InternalWeaponsFile {
    /// The internal gun.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gun: Option<GunInstallFile>,
}

/// An internal gun's installation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GunInstallFile {
    /// Id of the gun (`namespace:wpn/name`).
    #[serde(rename = "ref")]
    pub weapon_ref: String,
    /// Rounds loaded at the start of a sortie.
    #[schemars(range(min = 1, max = 5000))]
    pub rounds: u16,
    /// Muzzle position in body axes; the barrel points along the body's x axis.
    #[schemars(extend("x-unit" = "m"))]
    pub muzzle_m: [f64; 3],
    /// Angle between the barrel and the body's x axis in the pitch plane (nose up positive).
    #[schemars(extend("x-unit" = "mrad"), range(min = -100.0, max = 100.0))]
    pub harmonisation_mrad: f64,
}

/// What hangs on the aircraft.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LoadoutFile {
    /// Launch rails.
    #[schemars(length(min = 1, max = 8))]
    pub rails: Vec<RailFile>,
}

/// One launch rail.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RailFile {
    /// Hardpoint number, counted from 1.
    #[schemars(range(min = 1, max = 40))]
    pub station: u8,
    /// Position of the store's centre in body axes.
    #[schemars(extend("x-unit" = "m"))]
    pub pos_m: [f64; 3],
    /// Id of the store (`namespace:wpn/name`).
    pub store: String,
}

/// Signatures other aircraft can sense.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SignaturesFile {
    /// Infrared signature.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ir: Option<IrFile>,
    /// Effective size to the eye (GDD 16.6.2).
    #[serde(default)]
    #[schemars(extend("x-unit" = "m"), range(min = 0.0, max = 100.0))]
    pub visual_l_eff_m: f64,
}

/// Infrared intensities, relative to the Kestrel's afterburner (GDD C.11).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IrFile {
    /// Engine at idle.
    #[schemars(range(min = 0.0, max = 10.0))]
    pub engine_idle: f64,
    /// Engine at military power.
    #[schemars(range(min = 0.0, max = 10.0))]
    pub engine_mil: f64,
    /// Engine in full afterburner.
    #[schemars(range(min = 0.0, max = 10.0))]
    pub engine_ab: f64,
    /// Factor on the engine intensity seen from the side.
    #[schemars(range(min = 0.0, max = 1.0))]
    pub aspect_side: f64,
    /// Factor on the engine intensity seen from the front.
    #[schemars(range(min = 0.0, max = 1.0))]
    pub aspect_front: f64,
    /// Skin-heating intensity per Mach number squared.
    #[schemars(range(min = 0.0, max = 1.0))]
    pub skin_per_mach2: f64,
}

/// Damage points and hit volumes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DamageFile {
    /// Hit points when new.
    #[schemars(range(min = 1.0, max = 10000.0))]
    pub hp: f64,
    /// Energy that removes one hit point.
    #[schemars(extend("x-unit" = "J"), range(min = 1.0, max = 1.0e7))]
    pub joules_per_hp: f64,
    /// The hit volumes.
    #[schemars(length(min = 1, max = 16))]
    pub capsules: Vec<CapsuleFile>,
}

/// One hit volume: a capsule, a segment with a radius.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CapsuleFile {
    /// Name within the aircraft (`fuselage`, `wing_l`, ...).
    #[schemars(length(min = 1))]
    pub id: String,
    /// One end of the segment in body axes.
    #[schemars(extend("x-unit" = "m"))]
    pub a_m: [f64; 3],
    /// The other end.
    #[schemars(extend("x-unit" = "m"))]
    pub b_m: [f64; 3],
    /// Radius.
    #[schemars(extend("x-unit" = "m"), range(min = 0.05, max = 10.0))]
    pub radius_m: f64,
}

/// What an aircraft file's combat additions turn into in a definition.
pub(crate) type Armament = (
    Option<GunInstall>,
    Vec<Rail>,
    Option<IrSignature>,
    f32,
    Option<DamageDef>,
);

/// The runtime form of `a`'s combat additions. `None` if a reference does not resolve; the checks
/// and [`crate::bake`] report those first.
pub(crate) fn armament_to_def(
    a: &AircraftFile,
    resolve: &dyn Fn(&str) -> Option<ContentId>,
) -> Option<Armament> {
    let gun = match a.internal_weapons.as_ref().and_then(|w| w.gun.as_ref()) {
        None => None,
        Some(g) => Some(GunInstall {
            weapon: resolve(&g.weapon_ref)?,
            rounds: g.rounds,
            muzzle_m: s3(g.muzzle_m),
            harmonisation_mrad: g.harmonisation_mrad as f32,
        }),
    };
    let rails = match &a.loadout {
        None => Vec::new(),
        Some(l) => l
            .rails
            .iter()
            .map(|r| {
                Some(Rail {
                    station: r.station,
                    pos_m: s3(r.pos_m),
                    store: resolve(&r.store)?,
                })
            })
            .collect::<Option<Vec<_>>>()?,
    };
    let ir = a
        .signatures
        .as_ref()
        .and_then(|s| s.ir.as_ref())
        .map(|i| IrSignature {
            engine_idle: i.engine_idle as f32,
            engine_mil: i.engine_mil as f32,
            engine_ab: i.engine_ab as f32,
            aspect_side: i.aspect_side as f32,
            aspect_front: i.aspect_front as f32,
            skin_per_mach2: i.skin_per_mach2 as f32,
        });
    let visual = a
        .signatures
        .as_ref()
        .map_or(0.0, |s| s.visual_l_eff_m as f32);
    let damage = a.damage.as_ref().map(|d| DamageDef {
        hp: d.hp as f32,
        joules_per_hp: d.joules_per_hp as f32,
        capsules: d
            .capsules
            .iter()
            .map(|c| CapsuleDef {
                id: c.id.clone(),
                a_m: s3(c.a_m),
                b_m: s3(c.b_m),
                radius_m: c.radius_m as f32,
            })
            .collect(),
    });
    Some((gun, rails, ir, visual, damage))
}

/// The four optional aircraft sections of a baked definition.
pub(crate) type ArmamentFiles = (
    Option<InternalWeaponsFile>,
    Option<LoadoutFile>,
    Option<SignaturesFile>,
    Option<DamageFile>,
);

/// The authoring form of `d`'s combat additions. `name_of` maps a handle back to its id.
pub(crate) fn armament_from_def(
    d: &AircraftDef,
    name_of: &dyn Fn(ContentId) -> String,
) -> ArmamentFiles {
    let internal_weapons = d.gun.as_ref().map(|g| InternalWeaponsFile {
        gun: Some(GunInstallFile {
            weapon_ref: name_of(g.weapon),
            rounds: g.rounds,
            muzzle_m: f3(g.muzzle_m),
            harmonisation_mrad: f(g.harmonisation_mrad),
        }),
    });
    let loadout = (!d.rails.is_empty()).then(|| LoadoutFile {
        rails: d
            .rails
            .iter()
            .map(|r| RailFile {
                station: r.station,
                pos_m: f3(r.pos_m),
                store: name_of(r.store),
            })
            .collect(),
    });
    let signatures = (d.ir.is_some() || d.visual_l_eff_m != 0.0).then(|| SignaturesFile {
        ir: d.ir.map(|i| IrFile {
            engine_idle: f(i.engine_idle),
            engine_mil: f(i.engine_mil),
            engine_ab: f(i.engine_ab),
            aspect_side: f(i.aspect_side),
            aspect_front: f(i.aspect_front),
            skin_per_mach2: f(i.skin_per_mach2),
        }),
        visual_l_eff_m: f(d.visual_l_eff_m),
    });
    let damage = d.damage.as_ref().map(|x| DamageFile {
        hp: f(x.hp),
        joules_per_hp: f(x.joules_per_hp),
        capsules: x
            .capsules
            .iter()
            .map(|c| CapsuleFile {
                id: c.id.clone(),
                a_m: f3(c.a_m),
                b_m: f3(c.b_m),
                radius_m: f(c.radius_m),
            })
            .collect(),
    });
    (internal_weapons, loadout, signatures, damage)
}
