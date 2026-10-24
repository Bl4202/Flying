//! The authoring formats: strict serde types, which are also the source of the JSON Schemas.
//!
//! Every number carries its unit in its key (`_kg`, `_m`, `_deg`, ...) except the dimensionless
//! ones listed in [`crate::schema::DIMENSIONLESS`]. Unknown keys are errors, so a typo cannot be
//! silently ignored. Tables follow GDD Appendix A: a [`CurveFile`] is `(x, y)` pairs and a
//! [`GridFile`] is row-major `z[y][x]`; both name their axes, and the baker checks the names.
//!
//! The schemas stay at major version 1 while the game is pre-release and no saves exist (ADR-30).

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize};

use content::aero::{ConfigDeltas, Drag, Lateral, Lift, Pitch, PostStall};
use content::aircraft::{
    Actuator, AeroLimits, Airbrake, ContactPoint, EngineInstall, EnvelopePoint, Fcs, GearActuator,
    GearLeg, Geometry, Mass, NMaxByConfig, Tank,
};
use content::fcs::{
    ConfigGains, FullGains, GcasGains, GroundGains, MouseAimGains, NwsGains, OverspeedGains,
    PitchGains, RecoverGains, RollGains, YawGains,
};
use content::{
    AeroDef, AircraftDef, Category, ContentId, Curve, EngineDef, EngineKind, EnvelopeConfig, Era,
    FcsDef, Grid, Metric, Requirement,
};

/// Tag of the pack manifest format.
pub const PACK_SCHEMA: &str = "pack/1";
/// Tag of the aircraft format.
pub const AIRCRAFT_SCHEMA: &str = "aircraft/1";
/// Tag of the engine format.
pub const ENGINE_SCHEMA: &str = "engine/1";
/// Tag of the aero-table format.
pub const AERO_SCHEMA: &str = "aero/1";
/// Tag of the flight-control gains format.
pub const FCS_SCHEMA: &str = "fcs/1";

/// Axis names a table may use (each table field expects one; the baker checks which).
pub const AXES: &[&str] = &["mach", "alpha_deg", "deflection_deg", "qbar_kpa", "nz_g"];

/// `pack.json`: identity and compatibility of one content pack (GDD 62.4).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PackFile {
    /// Format tag, always `pack/1`.
    #[schemars(extend("const" = "pack/1"))]
    pub schema: String,
    /// Pack id and the namespace of every id inside it.
    #[schemars(length(min = 1))]
    pub id: String,
    /// Pack version (`major.minor.patch`).
    pub version: String,
    /// Range of game versions the pack works with.
    pub game: String,
    /// Licence of the pack's content.
    pub license: String,
    /// Who made it.
    pub authors: Vec<String>,
}

// ---------------------------------------------------------------------------------------------
// Tables.

/// `y(x)` as `(x, y)` pairs with a strictly increasing `x` (Appendix A `Table1D`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CurveFile {
    /// Name and unit of `x`.
    #[schemars(schema_with = "axis_schema")]
    pub axis: String,
    /// `(x, y)` pairs.
    #[schemars(length(min = 1))]
    pub points: Vec<[f64; 2]>,
}

/// `z(x, y)` stored row by row, `z[y][x]` (Appendix A `Table2D`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GridFile {
    /// Names and units of `x` and `y`.
    pub axes: [AxisName; 2],
    /// X grid, strictly increasing.
    #[schemars(length(min = 1))]
    pub x: Vec<f64>,
    /// Y grid, strictly increasing.
    #[schemars(length(min = 1))]
    pub y: Vec<f64>,
    /// One row of `x.len()` values per `y`.
    pub z: Vec<Vec<f64>>,
}

/// An axis name (a string from [`AXES`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AxisName(pub String);

impl JsonSchema for AxisName {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "AxisName".into()
    }
    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        axis_schema(generator)
    }
}

fn axis_schema(_: &mut SchemaGenerator) -> Schema {
    json_schema!({ "type": "string", "enum": AXES })
}

impl CurveFile {
    fn to_curve(&self) -> Curve {
        Curve {
            xs: self.points.iter().map(|p| p[0] as f32).collect(),
            ys: self.points.iter().map(|p| p[1] as f32).collect(),
        }
    }

    fn from_curve(axis: &str, c: &Curve) -> Self {
        Self {
            axis: axis.to_owned(),
            points: c
                .xs
                .iter()
                .zip(&c.ys)
                .map(|(x, y)| [f(*x), f(*y)])
                .collect(),
        }
    }
}

impl GridFile {
    fn to_grid(&self) -> Grid {
        Grid {
            xs: self.x.iter().map(|v| *v as f32).collect(),
            ys: self.y.iter().map(|v| *v as f32).collect(),
            zs: self.z.iter().flatten().map(|v| *v as f32).collect(),
        }
    }

    fn from_grid(axes: [&str; 2], g: &Grid) -> Self {
        Self {
            axes: [AxisName(axes[0].to_owned()), AxisName(axes[1].to_owned())],
            x: g.xs.iter().map(|v| f(*v)).collect(),
            y: g.ys.iter().map(|v| f(*v)).collect(),
            z: (0..g.ys.len())
                .map(|j| g.row(j).iter().map(|v| f(*v)).collect())
                .collect(),
        }
    }
}

/// The shortest decimal that reads back as the same `f32`: what the baker required of the input.
fn f(v: f32) -> f64 {
    v.to_string().parse::<f64>().unwrap_or(f64::from(v))
}

fn f3(v: [f32; 3]) -> [f64; 3] {
    [f(v[0]), f(v[1]), f(v[2])]
}

fn s3(v: [f64; 3]) -> [f32; 3] {
    [v[0] as f32, v[1] as f32, v[2] as f32]
}

// ---------------------------------------------------------------------------------------------
// aircraft/1

/// One aircraft (GDD 12.2, A.2, Appendix C).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AircraftFile {
    /// Format tag, always `aircraft/1`.
    #[schemars(extend("const" = "aircraft/1"))]
    pub schema: String,
    /// `namespace:type/name`, with namespace the pack id and type `ac`.
    pub id: String,
    /// Key of the display name in the locale table.
    pub name_key: String,
    /// Role.
    #[schemars(schema_with = "category_schema")]
    pub category: String,
    /// Technology generation.
    #[schemars(schema_with = "era_schema")]
    pub era: String,
    /// Crew count.
    #[schemars(range(min = 1, max = 16))]
    pub crew: u8,
    /// Hardpoint count.
    #[schemars(range(min = 0, max = 40))]
    pub hardpoints: u8,
    /// Mass properties.
    pub mass: MassFile,
    /// Reference geometry.
    pub geometry: GeometryFile,
    /// Engine installations.
    #[schemars(length(min = 1, max = 4))]
    pub engines: Vec<EngineInstallFile>,
    /// Aerodynamic references and operating limits.
    pub aero: AeroLimitsFile,
    /// Flight-control data.
    pub fcs: FcsFile,
    /// Authored performance targets (GDD 11.19).
    pub envelope: EnvelopeFile,
}

/// Mass properties (GDD 11.9).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MassFile {
    /// Empty mass.
    #[schemars(extend("x-unit" = "kg"), range(min = 1.0, max = 1.0e6))]
    pub empty_kg: f64,
    /// Internal fuel: the sum of the tank capacities.
    #[schemars(extend("x-unit" = "kg"), range(min = 1.0, max = 1.0e6))]
    pub internal_fuel_kg: f64,
    /// Maximum take-off mass.
    #[schemars(extend("x-unit" = "kg"), range(min = 1.0, max = 1.0e6))]
    pub max_takeoff_kg: f64,
    /// Maximum landing mass.
    #[schemars(extend("x-unit" = "kg"), range(min = 1.0, max = 1.0e6))]
    pub max_landing_kg: f64,
    /// Empty centre of gravity in body axes (x forward, y right, z down).
    #[schemars(extend("x-unit" = "m"))]
    pub cg_empty_m: [f64; 3],
    /// Empty roll, pitch and yaw inertia about the empty CG.
    #[schemars(extend("x-unit" = "kg m2"))]
    pub inertia_kgm2: [f64; 3],
    /// Empty product of inertia XZ (zero for a symmetric mass distribution).
    #[serde(default)]
    #[schemars(extend("x-unit" = "kg m2"))]
    pub inertia_xz_kgm2: f64,
    /// Fuel density (JP-8 is about 0.80).
    #[schemars(extend("x-unit" = "kg/L"), range(min = 0.5, max = 1.2))]
    pub fuel_density_kgpl: f64,
    /// Fuel tanks.
    #[schemars(length(min = 1, max = 16))]
    pub tanks: Vec<TankFile>,
}

/// One fuel tank.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TankFile {
    /// Name within the aircraft.
    #[schemars(length(min = 1))]
    pub id: String,
    /// Capacity.
    #[schemars(extend("x-unit" = "kg"), range(min = 1.0, max = 1.0e6))]
    pub capacity_kg: f64,
    /// Centroid in body axes.
    #[schemars(extend("x-unit" = "m"))]
    pub pos_m: [f64; 3],
    /// Feed order: lower feeds first; equal priorities feed together in proportion.
    #[schemars(range(min = 1, max = 16))]
    pub feed_priority: u8,
}

/// Reference geometry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GeometryFile {
    /// Wing reference area.
    #[schemars(extend("x-unit" = "m2"), range(min = 0.1, max = 1000.0))]
    pub wing_area_m2: f64,
    /// Wing span.
    #[schemars(extend("x-unit" = "m"), range(min = 0.1, max = 100.0))]
    pub span_m: f64,
    /// Mean aerodynamic chord.
    #[schemars(extend("x-unit" = "m"), range(min = 0.1, max = 50.0))]
    pub mac_m: f64,
    /// Aerodynamic moment reference point in body axes.
    #[schemars(extend("x-unit" = "m"))]
    pub ref_point_m: [f64; 3],
    /// Bounding box: length, span, height.
    #[schemars(extend("x-unit" = "m"))]
    pub bbox_m: [f64; 3],
    /// Landing-gear legs (GDD 11.13).
    #[schemars(length(max = 4))]
    pub gear: Vec<GearLegFile>,
    /// Airframe points that can scrape the ground.
    #[schemars(length(max = 8))]
    pub contacts: Vec<ContactFile>,
}

/// One landing-gear leg: a massless strut with a wheel at its end.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GearLegFile {
    /// Name within the aircraft (`nose`, `main_l`, ...).
    #[schemars(length(min = 1))]
    pub id: String,
    /// Wheel contact point with the leg fully extended, in body axes.
    #[schemars(extend("x-unit" = "m"))]
    pub pos_m: [f64; 3],
    /// Strut travel.
    #[schemars(extend("x-unit" = "m"), range(min = 0.05, max = 2.0))]
    pub stroke_m: f64,
    /// Spring rate.
    #[schemars(extend("x-unit" = "N/m"), range(min = 1.0e3, max = 5.0e6))]
    pub k_npm: f64,
    /// Damping while the strut compresses.
    #[schemars(extend("x-unit" = "N s/m"), range(min = 0.0, max = 5.0e5))]
    pub c_comp_nspm: f64,
    /// Damping while the strut extends (usually the larger).
    #[schemars(extend("x-unit" = "N s/m"), range(min = 0.0, max = 5.0e5))]
    pub c_ext_nspm: f64,
    /// Largest steering angle (0 if the leg does not steer).
    #[schemars(extend("x-unit" = "deg"), range(min = 0.0, max = 90.0))]
    pub steer_max_deg: f64,
    /// True if the wheel has a brake.
    pub brake: bool,
    /// True for a main leg.
    pub main: bool,
}

/// One airframe contact point.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ContactFile {
    /// Name within the aircraft (`nose_tip`, `tail`, `wing_l`, ...).
    #[schemars(length(min = 1))]
    pub id: String,
    /// Position in body axes.
    #[schemars(extend("x-unit" = "m"))]
    pub pos_m: [f64; 3],
}

/// One engine installation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EngineInstallFile {
    /// Id of the engine (`namespace:eng/name`).
    #[serde(rename = "ref")]
    pub engine_ref: String,
    /// Engines of this type at this position.
    #[schemars(range(min = 1, max = 8))]
    pub count: u8,
    /// Thrust point in body axes.
    #[schemars(extend("x-unit" = "m"))]
    pub position_m: [f64; 3],
}

/// Aerodynamic references and operating limits.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AeroLimitsFile {
    /// Id of the aero tables (`namespace:aero/name`).
    pub tables_ref: String,
    /// Highest Mach the tables are meant for.
    #[schemars(range(min = 0.1, max = 10.0))]
    pub max_mach: f64,
    /// Maximum operating Mach number.
    #[schemars(range(min = 0.1, max = 10.0))]
    pub mmo: f64,
    /// Maximum operating speed.
    #[schemars(extend("x-unit" = "m/s"), range(min = 10.0, max = 2000.0))]
    pub vmo_mps: f64,
    /// Service ceiling.
    #[schemars(extend("x-unit" = "m"), range(min = 100.0, max = 40000.0))]
    pub service_ceiling_m: f64,
    /// Structural positive load factor.
    #[schemars(extend("x-unit" = "g"), range(min = 1.0, max = 20.0))]
    pub n_max: f64,
    /// Structural negative load factor (below zero).
    #[schemars(extend("x-unit" = "g"), range(min = -20.0, max = -0.1))]
    pub n_min: f64,
    /// Positive load factors with the gear or the flaps out.
    pub n_max_by_config: NMaxByConfigFile,
    /// Gear-extended speed limit.
    #[schemars(extend("x-unit" = "m/s"), range(min = 10.0, max = 1000.0))]
    pub gear_limit_mps: f64,
    /// Flap-extended speed limit.
    #[schemars(extend("x-unit" = "m/s"), range(min = 10.0, max = 1000.0))]
    pub flap_limit_mps: f64,
    /// Static margin as a fraction of the mean aerodynamic chord (negative if unstable).
    #[schemars(extend("x-unit" = "fraction of MAC"), range(min = -0.5, max = 0.5))]
    pub static_margin_mac: f64,
}

/// Load-factor limits that depend on the configuration.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NMaxByConfigFile {
    /// Positive limit with the gear extended.
    #[schemars(extend("x-unit" = "g"), range(min = 1.0, max = 20.0))]
    pub gear: f64,
    /// Positive limit with the flaps out.
    #[schemars(extend("x-unit" = "g"), range(min = 1.0, max = 20.0))]
    pub flaps: f64,
}

/// Flight-control data (GDD 11.11): limits and actuators, and the gains by reference.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FcsFile {
    /// True if the airframe cannot be flown without stability augmentation.
    pub sas_required: bool,
    /// Id of the control-law gains (`namespace:fcs/name`).
    pub gains_ref: String,
    /// Limiter settings.
    pub limiters: LimitersFile,
    /// Control-surface actuators (GDD 11.7).
    pub actuators: ActuatorsFile,
}

/// Limiter settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LimitersFile {
    /// Angle-of-attack limit.
    #[schemars(extend("x-unit" = "deg"), range(min = 5.0, max = 90.0))]
    pub alpha_limit_deg: f64,
    /// Largest commanded load factor.
    #[schemars(extend("x-unit" = "g"), range(min = 1.0, max = 20.0))]
    pub n_cmd_max: f64,
    /// Largest commanded roll rate.
    #[schemars(extend("x-unit" = "deg/s"), range(min = 10.0, max = 720.0))]
    pub roll_rate_max_dps: f64,
}

/// The actuators of the standard surfaces.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ActuatorsFile {
    /// Stabilator (pitch).
    pub stab: ActuatorFile,
    /// Ailerons or flaperons (roll).
    pub aileron: ActuatorFile,
    /// Rudder (yaw).
    pub rudder: ActuatorFile,
    /// Leading-edge flaps.
    pub lef: ActuatorFile,
    /// Trailing-edge flaps.
    pub tef: ActuatorFile,
    /// Airbrake.
    pub airbrake: AirbrakeFile,
    /// Landing gear.
    pub gear: GearActuatorFile,
}

/// The landing-gear actuator.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GearActuatorFile {
    /// Time for the gear to travel from up to down (or back).
    #[schemars(extend("x-unit" = "s"), range(min = 0.5, max = 60.0))]
    pub transit_s: f64,
}

/// One surface actuator.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ActuatorFile {
    /// Travel, minimum then maximum.
    #[schemars(extend("x-unit" = "deg"))]
    pub range_deg: [f64; 2],
    /// Rate limit.
    #[schemars(extend("x-unit" = "deg/s"), range(min = 1.0, max = 1000.0))]
    pub rate_dps: f64,
    /// First-order time constant.
    #[schemars(extend("x-unit" = "s"), range(min = 0.001, max = 5.0))]
    pub tau_s: f64,
}

/// The airbrake actuator (moves as a fraction 0 to 1).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AirbrakeFile {
    /// Rate limit.
    #[schemars(extend("x-unit" = "1/s"), range(min = 0.01, max = 100.0))]
    pub rate_per_s: f64,
    /// First-order time constant.
    #[schemars(extend("x-unit" = "s"), range(min = 0.001, max = 5.0))]
    pub tau_s: f64,
}

/// The authored performance envelope (GDD 11.19, C.9).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EnvelopeFile {
    /// Target points.
    #[schemars(length(max = 64))]
    pub points: Vec<EnvelopePointFile>,
}

/// One performance target.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EnvelopePointFile {
    /// Name of the point.
    #[schemars(length(min = 1))]
    pub id: String,
    /// Aircraft mass.
    #[schemars(extend("x-unit" = "kg"), range(min = 1.0, max = 1.0e6))]
    pub mass_kg: f64,
    /// Height above MSL.
    #[schemars(extend("x-unit" = "m"), range(min = -500.0, max = 30000.0))]
    pub alt_m: f64,
    /// Flight Mach number; required for the metrics measured at a given speed (`n_sust`,
    /// `turn_dps`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0.0, max = 5.0))]
    pub mach: Option<f64>,
    /// Configuration.
    #[schemars(schema_with = "config_schema")]
    pub config: String,
    /// What is measured.
    #[schemars(schema_with = "metric_schema")]
    pub metric: String,
    /// Target, in the metric's unit (`max_mach`: Mach; `*_mps`: m/s; `turn_dps`: deg/s; ...).
    #[schemars(extend("x-unit" = "per metric"), range(min = 0.0))]
    pub value: f64,
    /// Tolerance.
    #[schemars(extend("x-unit" = "%"), range(min = 0.1, max = 50.0))]
    pub tol_pct: f64,
    /// A capability the point needs that the model may not have yet (ADR-31).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(schema_with = "requirement_schema")]
    pub requires: Option<String>,
}

fn names_schema(names: &[&str]) -> Schema {
    json_schema!({ "type": "string", "enum": names })
}

fn category_schema(_: &mut SchemaGenerator) -> Schema {
    names_schema(&Category::ALL.iter().map(|c| c.name()).collect::<Vec<_>>())
}

fn era_schema(_: &mut SchemaGenerator) -> Schema {
    names_schema(&Era::ALL.iter().map(|e| e.name()).collect::<Vec<_>>())
}

fn engine_kind_schema(_: &mut SchemaGenerator) -> Schema {
    names_schema(&EngineKind::ALL.iter().map(|e| e.name()).collect::<Vec<_>>())
}

fn config_schema(_: &mut SchemaGenerator) -> Schema {
    names_schema(
        &EnvelopeConfig::ALL
            .iter()
            .map(|e| e.name())
            .collect::<Vec<_>>(),
    )
}

fn metric_schema(_: &mut SchemaGenerator) -> Schema {
    names_schema(&Metric::ALL.iter().map(|e| e.name()).collect::<Vec<_>>())
}

fn requirement_schema(_: &mut SchemaGenerator) -> Schema {
    names_schema(
        &Requirement::ALL
            .iter()
            .map(|e| e.name())
            .collect::<Vec<_>>(),
    )
}

fn actuator(a: &ActuatorFile) -> Actuator {
    Actuator {
        range_deg: [a.range_deg[0] as f32, a.range_deg[1] as f32],
        rate_dps: a.rate_dps as f32,
        tau_s: a.tau_s as f32,
    }
}

fn actuator_file(a: &Actuator) -> ActuatorFile {
    ActuatorFile {
        range_deg: [f(a.range_deg[0]), f(a.range_deg[1])],
        rate_dps: f(a.rate_dps),
        tau_s: f(a.tau_s),
    }
}

impl EnvelopePointFile {
    fn to_point(&self) -> Option<EnvelopePoint> {
        Some(EnvelopePoint {
            id: self.id.clone(),
            mass_kg: self.mass_kg as f32,
            alt_m: self.alt_m as f32,
            mach: self.mach.map(|m| m as f32),
            config: EnvelopeConfig::from_name(&self.config)?,
            metric: Metric::from_name(&self.metric)?,
            value: self.value as f32,
            tol_pct: self.tol_pct as f32,
            requires: match &self.requires {
                None => None,
                Some(r) => Some(Requirement::from_name(r)?),
            },
        })
    }
}

impl GeometryFile {
    fn to_geometry(&self) -> Geometry {
        Geometry {
            wing_area_m2: self.wing_area_m2 as f32,
            span_m: self.span_m as f32,
            mac_m: self.mac_m as f32,
            ref_point_m: s3(self.ref_point_m),
            bbox_m: s3(self.bbox_m),
            gear: self
                .gear
                .iter()
                .map(|l| GearLeg {
                    id: l.id.clone(),
                    pos_m: s3(l.pos_m),
                    stroke_m: l.stroke_m as f32,
                    k_npm: l.k_npm as f32,
                    c_comp_nspm: l.c_comp_nspm as f32,
                    c_ext_nspm: l.c_ext_nspm as f32,
                    steer_max_deg: l.steer_max_deg as f32,
                    brake: l.brake,
                    main: l.main,
                })
                .collect(),
            contacts: self
                .contacts
                .iter()
                .map(|p| ContactPoint {
                    id: p.id.clone(),
                    pos_m: s3(p.pos_m),
                })
                .collect(),
        }
    }

    fn from_geometry(g: &Geometry) -> Self {
        Self {
            wing_area_m2: f(g.wing_area_m2),
            span_m: f(g.span_m),
            mac_m: f(g.mac_m),
            ref_point_m: f3(g.ref_point_m),
            bbox_m: f3(g.bbox_m),
            gear: g
                .gear
                .iter()
                .map(|l| GearLegFile {
                    id: l.id.clone(),
                    pos_m: f3(l.pos_m),
                    stroke_m: f(l.stroke_m),
                    k_npm: f(l.k_npm),
                    c_comp_nspm: f(l.c_comp_nspm),
                    c_ext_nspm: f(l.c_ext_nspm),
                    steer_max_deg: f(l.steer_max_deg),
                    brake: l.brake,
                    main: l.main,
                })
                .collect(),
            contacts: g
                .contacts
                .iter()
                .map(|p| ContactFile {
                    id: p.id.clone(),
                    pos_m: f3(p.pos_m),
                })
                .collect(),
        }
    }
}

impl AircraftFile {
    /// The runtime definition. `resolve` maps an entity id to its handle. `None` if a name or a
    /// reference does not resolve; [`crate::check`] and [`crate::bake`] report those with a
    /// pointer before this is called.
    #[must_use]
    pub fn to_def(&self, resolve: &dyn Fn(&str) -> Option<ContentId>) -> Option<AircraftDef> {
        let m = &self.mass;
        let g = &self.geometry;
        let a = &self.aero;
        let c = &self.fcs;
        let engines = self
            .engines
            .iter()
            .map(|e| {
                Some(EngineInstall {
                    engine: resolve(&e.engine_ref)?,
                    count: e.count,
                    position_m: s3(e.position_m),
                })
            })
            .collect::<Option<Vec<_>>>()?;
        let envelope = self
            .envelope
            .points
            .iter()
            .map(EnvelopePointFile::to_point)
            .collect::<Option<Vec<_>>>()?;
        Some(AircraftDef {
            name_key: self.name_key.clone(),
            category: Category::from_name(&self.category)?,
            era: Era::from_name(&self.era)?,
            crew: self.crew,
            hardpoints: self.hardpoints,
            mass: Mass {
                empty_kg: m.empty_kg as f32,
                internal_fuel_kg: m.internal_fuel_kg as f32,
                max_takeoff_kg: m.max_takeoff_kg as f32,
                max_landing_kg: m.max_landing_kg as f32,
                cg_empty_m: s3(m.cg_empty_m),
                inertia_kgm2: s3(m.inertia_kgm2),
                inertia_xz_kgm2: m.inertia_xz_kgm2 as f32,
                fuel_density_kgpl: m.fuel_density_kgpl as f32,
                tanks: m
                    .tanks
                    .iter()
                    .map(|t| Tank {
                        id: t.id.clone(),
                        capacity_kg: t.capacity_kg as f32,
                        pos_m: s3(t.pos_m),
                        feed_priority: t.feed_priority,
                    })
                    .collect(),
            },
            geometry: g.to_geometry(),
            engines,
            aero: AeroLimits {
                tables: resolve(&a.tables_ref)?,
                max_mach: a.max_mach as f32,
                mmo: a.mmo as f32,
                vmo_mps: a.vmo_mps as f32,
                service_ceiling_m: a.service_ceiling_m as f32,
                n_max: a.n_max as f32,
                n_min: a.n_min as f32,
                n_max_by_config: NMaxByConfig {
                    gear: a.n_max_by_config.gear as f32,
                    flaps: a.n_max_by_config.flaps as f32,
                },
                gear_limit_mps: a.gear_limit_mps as f32,
                flap_limit_mps: a.flap_limit_mps as f32,
                static_margin_mac: a.static_margin_mac as f32,
            },
            fcs: Fcs {
                sas_required: c.sas_required,
                alpha_limit_deg: c.limiters.alpha_limit_deg as f32,
                n_cmd_max: c.limiters.n_cmd_max as f32,
                roll_rate_max_dps: c.limiters.roll_rate_max_dps as f32,
                stab: actuator(&c.actuators.stab),
                aileron: actuator(&c.actuators.aileron),
                rudder: actuator(&c.actuators.rudder),
                lef: actuator(&c.actuators.lef),
                tef: actuator(&c.actuators.tef),
                airbrake: Airbrake {
                    rate_per_s: c.actuators.airbrake.rate_per_s as f32,
                    tau_s: c.actuators.airbrake.tau_s as f32,
                },
                gear: GearActuator {
                    transit_s: c.actuators.gear.transit_s as f32,
                },
                gains: resolve(&c.gains_ref)?,
            },
            envelope,
        })
    }

    /// The authoring form of a baked definition. `name_of` maps a handle back to its id.
    #[must_use]
    pub fn from_def(id: &str, d: &AircraftDef, name_of: &dyn Fn(ContentId) -> String) -> Self {
        let m = &d.mass;
        let g = &d.geometry;
        let a = &d.aero;
        let c = &d.fcs;
        Self {
            schema: AIRCRAFT_SCHEMA.to_owned(),
            id: id.to_owned(),
            name_key: d.name_key.clone(),
            category: d.category.name().to_owned(),
            era: d.era.name().to_owned(),
            crew: d.crew,
            hardpoints: d.hardpoints,
            mass: MassFile {
                empty_kg: f(m.empty_kg),
                internal_fuel_kg: f(m.internal_fuel_kg),
                max_takeoff_kg: f(m.max_takeoff_kg),
                max_landing_kg: f(m.max_landing_kg),
                cg_empty_m: f3(m.cg_empty_m),
                inertia_kgm2: f3(m.inertia_kgm2),
                inertia_xz_kgm2: f(m.inertia_xz_kgm2),
                fuel_density_kgpl: f(m.fuel_density_kgpl),
                tanks: m
                    .tanks
                    .iter()
                    .map(|t| TankFile {
                        id: t.id.clone(),
                        capacity_kg: f(t.capacity_kg),
                        pos_m: f3(t.pos_m),
                        feed_priority: t.feed_priority,
                    })
                    .collect(),
            },
            geometry: GeometryFile::from_geometry(g),
            engines: d
                .engines
                .iter()
                .map(|e| EngineInstallFile {
                    engine_ref: name_of(e.engine),
                    count: e.count,
                    position_m: f3(e.position_m),
                })
                .collect(),
            aero: AeroLimitsFile {
                tables_ref: name_of(a.tables),
                max_mach: f(a.max_mach),
                mmo: f(a.mmo),
                vmo_mps: f(a.vmo_mps),
                service_ceiling_m: f(a.service_ceiling_m),
                n_max: f(a.n_max),
                n_min: f(a.n_min),
                n_max_by_config: NMaxByConfigFile {
                    gear: f(a.n_max_by_config.gear),
                    flaps: f(a.n_max_by_config.flaps),
                },
                gear_limit_mps: f(a.gear_limit_mps),
                flap_limit_mps: f(a.flap_limit_mps),
                static_margin_mac: f(a.static_margin_mac),
            },
            fcs: FcsFile {
                sas_required: c.sas_required,
                gains_ref: name_of(c.gains),
                limiters: LimitersFile {
                    alpha_limit_deg: f(c.alpha_limit_deg),
                    n_cmd_max: f(c.n_cmd_max),
                    roll_rate_max_dps: f(c.roll_rate_max_dps),
                },
                actuators: ActuatorsFile {
                    stab: actuator_file(&c.stab),
                    aileron: actuator_file(&c.aileron),
                    rudder: actuator_file(&c.rudder),
                    lef: actuator_file(&c.lef),
                    tef: actuator_file(&c.tef),
                    airbrake: AirbrakeFile {
                        rate_per_s: f(c.airbrake.rate_per_s),
                        tau_s: f(c.airbrake.tau_s),
                    },
                    gear: GearActuatorFile {
                        transit_s: f(c.gear.transit_s),
                    },
                },
            },
            envelope: EnvelopeFile {
                points: d
                    .envelope
                    .iter()
                    .map(|p| EnvelopePointFile {
                        id: p.id.clone(),
                        mass_kg: f(p.mass_kg),
                        alt_m: f(p.alt_m),
                        mach: p.mach.map(f),
                        config: p.config.name().to_owned(),
                        metric: p.metric.name().to_owned(),
                        value: f(p.value),
                        tol_pct: f(p.tol_pct),
                        requires: p.requires.map(|r| r.name().to_owned()),
                    })
                    .collect(),
            },
        }
    }
}

// ---------------------------------------------------------------------------------------------
// engine/1

/// One engine type (GDD 11.8, A.3, C.3). Thrust and fuel flow are per engine.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EngineFile {
    /// Format tag, always `engine/1`.
    #[schemars(extend("const" = "engine/1"))]
    pub schema: String,
    /// `namespace:eng/name`.
    pub id: String,
    /// Key of the display name in the locale table.
    pub name_key: String,
    /// Engine type.
    #[schemars(schema_with = "engine_kind_schema")]
    pub kind: String,
    /// Thrust deck.
    pub thrust: ThrustFile,
    /// Fuel consumption.
    pub tsfc: TsfcFile,
    /// Spool and afterburner dynamics.
    pub dynamics: DynamicsFile,
    /// Inlet distortion at high angle of attack.
    pub inlet: InletFile,
    /// Operating limits.
    pub limits: EngineLimitsFile,
}

/// Thrust deck: `F = F_SL (rho / rho0)^n f(M)`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ThrustFile {
    /// Sea-level static thrust at military (dry) power.
    #[schemars(extend("x-unit" = "N"), range(min = 1.0, max = 1.0e6))]
    pub sl_dry_n: f64,
    /// Sea-level static thrust in full afterburner (equal to dry if there is none).
    #[schemars(extend("x-unit" = "N"), range(min = 1.0, max = 1.0e6))]
    pub sl_ab_n: f64,
    /// Exponent of the density lapse.
    #[schemars(range(min = 0.1, max = 2.0))]
    pub density_exp: f64,
    /// Dry thrust factor against Mach.
    pub mach_factor_dry: CurveFile,
    /// Afterburner thrust factor against Mach.
    pub mach_factor_ab: CurveFile,
    /// Idle thrust as a fraction of military thrust.
    #[schemars(range(min = 0.0, max = 0.5))]
    pub idle_fraction: f64,
}

/// Thrust-specific fuel consumption at sea-level static conditions.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TsfcFile {
    /// Dry.
    #[schemars(extend("x-unit" = "kg/(N s)"), range(min = 1.0e-6, max = 1.0e-3))]
    pub dry_kg_per_ns: f64,
    /// Full afterburner.
    #[schemars(extend("x-unit" = "kg/(N s)"), range(min = 1.0e-6, max = 1.0e-3))]
    pub ab_kg_per_ns: f64,
}

/// Spool and afterburner dynamics.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DynamicsFile {
    /// Spool-up time constant.
    #[schemars(extend("x-unit" = "s"), range(min = 0.05, max = 30.0))]
    pub spool_up_s: f64,
    /// Spool-down time constant.
    #[schemars(extend("x-unit" = "s"), range(min = 0.05, max = 30.0))]
    pub spool_down_s: f64,
    /// Afterburner light-off delay.
    #[schemars(extend("x-unit" = "s"), range(min = 0.0, max = 10.0))]
    pub ab_lightoff_s: f64,
    /// Time constant of the afterburner level rising once lit.
    #[schemars(extend("x-unit" = "s"), range(min = 0.0, max = 10.0))]
    pub ab_stage_s: f64,
    /// Time constant of the afterburner level falling when it is throttled back or cut.
    #[schemars(extend("x-unit" = "s"), range(min = 0.0, max = 10.0))]
    pub ab_cutoff_s: f64,
    /// Afterburner stages (0 for none).
    #[schemars(range(min = 0, max = 8))]
    pub ab_stages: u8,
}

/// Inlet distortion: thrust is scaled by `1 - 0.5 s^2`, `s = clamp((|alpha| - start) / span)`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InletFile {
    /// Angle of attack where the loss begins.
    #[schemars(extend("x-unit" = "deg"), range(min = 0.0, max = 90.0))]
    pub alpha_start_deg: f64,
    /// Span over which the loss reaches its maximum.
    #[schemars(extend("x-unit" = "deg"), range(min = 1.0, max = 90.0))]
    pub alpha_span_deg: f64,
}

/// Engine operating limits.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EngineLimitsFile {
    /// Angle of attack above which the compressor stalls at low speed.
    #[schemars(extend("x-unit" = "deg"), range(min = 10.0, max = 90.0))]
    pub compressor_stall_alpha_deg: f64,
    /// Lowest airspeed for a windmill relight.
    #[schemars(extend("x-unit" = "m/s"), range(min = 0.0, max = 1000.0))]
    pub windmill_relight_mps: f64,
}

impl EngineFile {
    /// The runtime definition. `None` if `kind` is not a known name.
    #[must_use]
    pub fn to_def(&self) -> Option<EngineDef> {
        let t = &self.thrust;
        let d = &self.dynamics;
        Some(EngineDef {
            name_key: self.name_key.clone(),
            kind: EngineKind::from_name(&self.kind)?,
            sl_dry_n: t.sl_dry_n as f32,
            sl_ab_n: t.sl_ab_n as f32,
            density_exp: t.density_exp as f32,
            mach_factor_dry: t.mach_factor_dry.to_curve(),
            mach_factor_ab: t.mach_factor_ab.to_curve(),
            idle_fraction: t.idle_fraction as f32,
            tsfc_dry_kg_per_ns: self.tsfc.dry_kg_per_ns as f32,
            tsfc_ab_kg_per_ns: self.tsfc.ab_kg_per_ns as f32,
            spool_up_s: d.spool_up_s as f32,
            spool_down_s: d.spool_down_s as f32,
            ab_lightoff_s: d.ab_lightoff_s as f32,
            ab_stage_s: d.ab_stage_s as f32,
            ab_cutoff_s: d.ab_cutoff_s as f32,
            ab_stages: d.ab_stages,
            inlet_alpha_start_deg: self.inlet.alpha_start_deg as f32,
            inlet_alpha_span_deg: self.inlet.alpha_span_deg as f32,
            compressor_stall_alpha_deg: self.limits.compressor_stall_alpha_deg as f32,
            windmill_relight_mps: self.limits.windmill_relight_mps as f32,
        })
    }

    /// The authoring form of a baked definition.
    #[must_use]
    pub fn from_def(id: &str, e: &EngineDef) -> Self {
        Self {
            schema: ENGINE_SCHEMA.to_owned(),
            id: id.to_owned(),
            name_key: e.name_key.clone(),
            kind: e.kind.name().to_owned(),
            thrust: ThrustFile {
                sl_dry_n: f(e.sl_dry_n),
                sl_ab_n: f(e.sl_ab_n),
                density_exp: f(e.density_exp),
                mach_factor_dry: CurveFile::from_curve("mach", &e.mach_factor_dry),
                mach_factor_ab: CurveFile::from_curve("mach", &e.mach_factor_ab),
                idle_fraction: f(e.idle_fraction),
            },
            tsfc: TsfcFile {
                dry_kg_per_ns: f(e.tsfc_dry_kg_per_ns),
                ab_kg_per_ns: f(e.tsfc_ab_kg_per_ns),
            },
            dynamics: DynamicsFile {
                spool_up_s: f(e.spool_up_s),
                spool_down_s: f(e.spool_down_s),
                ab_lightoff_s: f(e.ab_lightoff_s),
                ab_stage_s: f(e.ab_stage_s),
                ab_cutoff_s: f(e.ab_cutoff_s),
                ab_stages: e.ab_stages,
            },
            inlet: InletFile {
                alpha_start_deg: f(e.inlet_alpha_start_deg),
                alpha_span_deg: f(e.inlet_alpha_span_deg),
            },
            limits: EngineLimitsFile {
                compressor_stall_alpha_deg: f(e.compressor_stall_alpha_deg),
                windmill_relight_mps: f(e.windmill_relight_mps),
            },
        }
    }
}

// ---------------------------------------------------------------------------------------------
// aero/1

/// One airframe's aerodynamic tables (GDD 11.6, C.4, C.5).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AeroFile {
    /// Format tag, always `aero/1`.
    #[schemars(extend("const" = "aero/1"))]
    pub schema: String,
    /// `namespace:aero/name`.
    pub id: String,
    /// Lift.
    pub lift: LiftFile,
    /// Drag.
    pub drag: DragFile,
    /// Pitching moment.
    pub pitch: PitchFile,
    /// Lateral and directional derivatives.
    pub lateral: LateralFile,
    /// Blend to a flat plate beyond the tables (GDD 11.6.3, ADR-30).
    pub post_stall: PostStallFile,
    /// Control power against deflection, `eta(deflection_deg)`.
    pub control_effectiveness: CurveFile,
    /// Configuration increments.
    pub config: ConfigFile,
    /// Moment reference point, fraction of MAC from its leading edge.
    #[schemars(extend("x-unit" = "fraction of MAC"), range(min = 0.0, max = 1.0))]
    pub x_ref_mac: f64,
    /// Ground-effect lift factor `k_g`.
    #[schemars(range(min = 0.0, max = 1.0))]
    pub ground_effect_k: f64,
}

/// Lift.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LiftFile {
    /// `C_L,base(alpha_deg, mach)`, clean, through the stall and beyond.
    pub cl_base: GridFile,
    /// Trimmed whole-aircraft maximum lift coefficient against Mach.
    pub cl_max: CurveFile,
    /// Lift-curve slope against Mach.
    pub cl_alpha_per_rad: CurveFile,
    /// `C_L0` of the induced-drag term.
    #[schemars(range(min = -1.0, max = 1.0))]
    pub cl0: f64,
    /// Lift per radian of elevator against Mach.
    pub cl_de_per_rad: CurveFile,
}

/// Drag.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DragFile {
    /// Zero-lift drag against Mach.
    pub cd0: CurveFile,
    /// Wave-drag rise against Mach.
    pub cd_wave: CurveFile,
    /// Induced-drag factor against Mach.
    pub k_induced: CurveFile,
}

/// Pitching moment about the reference point.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PitchFile {
    /// `C_m0(alpha_deg, mach)`, elevator neutral.
    pub cm_0: GridFile,
    /// Moment per radian of elevator against Mach.
    pub cm_de_per_rad: CurveFile,
    /// Pitch damping.
    #[schemars(range(min = -100.0, max = 0.0))]
    pub cm_q_per_rad: f64,
    /// Lag-of-downwash damping.
    #[schemars(range(min = -100.0, max = 0.0))]
    pub cm_alpha_dot_per_rad: f64,
}

/// Lateral and directional derivatives (GDD C.5), each against `|alpha_deg|` (ADR-38).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LateralFile {
    /// Side force from sideslip.
    pub cy_beta_per_rad: CurveFile,
    /// Rolling moment from sideslip.
    pub cl_beta_per_rad: CurveFile,
    /// Yawing moment from sideslip.
    pub cn_beta_per_rad: CurveFile,
    /// Roll damping.
    pub cl_p_per_rad: CurveFile,
    /// Rolling moment from yaw rate.
    pub cl_r_per_rad: CurveFile,
    /// Yawing moment from roll rate.
    pub cn_p_per_rad: CurveFile,
    /// Yaw damping.
    pub cn_r_per_rad: CurveFile,
    /// Rolling moment per radian of aileron.
    pub cl_da_per_rad: CurveFile,
    /// Yawing moment per radian of aileron.
    pub cn_da_per_rad: CurveFile,
    /// Rolling moment per radian of rudder.
    pub cl_dr_per_rad: CurveFile,
    /// Yawing moment per radian of rudder.
    pub cn_dr_per_rad: CurveFile,
    /// Side force per radian of rudder.
    pub cy_dr_per_rad: CurveFile,
}

/// The flat-plate blend beyond the tables.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PostStallFile {
    /// Angle of attack where the blend starts, against Mach.
    pub blend_start_deg: CurveFile,
    /// Angle of attack where the blend is complete, against Mach.
    pub blend_end_deg: CurveFile,
    /// Flat-plate normal-force coefficient at 90 degrees.
    #[schemars(range(min = 0.1, max = 3.0))]
    pub cd90: f64,
    /// Flat-plate aerodynamic centre, fraction of MAC from its leading edge.
    #[schemars(extend("x-unit" = "fraction of MAC"), range(min = 0.0, max = 1.0))]
    pub x_ac_post_mac: f64,
}

/// Configuration increments (GDD C.4).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfigFile {
    /// Drag of the extended gear.
    #[schemars(range(min = 0.0, max = 0.5))]
    pub gear_cd0: f64,
    /// Drag of take-off flaps.
    #[schemars(range(min = 0.0, max = 0.5))]
    pub flap_to_cd0: f64,
    /// Drag of landing flaps.
    #[schemars(range(min = 0.0, max = 0.5))]
    pub flap_ld_cd0: f64,
    /// Drag of the fully open airbrake.
    #[schemars(range(min = 0.0, max = 0.5))]
    pub brake_cd0: f64,
    /// Lift of take-off flaps.
    #[schemars(range(min = 0.0, max = 2.0))]
    pub flap_to_cl: f64,
    /// Lift of landing flaps.
    #[schemars(range(min = 0.0, max = 2.0))]
    pub flap_ld_cl: f64,
}

/// The axis each aero table field must use: `(JSON pointer, axes)`. One axis for a curve, two
/// (x then y) for a grid.
pub const AERO_AXES: &[(&str, &[&str])] = &[
    ("/lift/cl_base", &["alpha_deg", "mach"]),
    ("/lift/cl_max", &["mach"]),
    ("/lift/cl_alpha_per_rad", &["mach"]),
    ("/lift/cl_de_per_rad", &["mach"]),
    ("/drag/cd0", &["mach"]),
    ("/drag/cd_wave", &["mach"]),
    ("/drag/k_induced", &["mach"]),
    ("/pitch/cm_0", &["alpha_deg", "mach"]),
    ("/pitch/cm_de_per_rad", &["mach"]),
    ("/lateral/cy_beta_per_rad", &["alpha_deg"]),
    ("/lateral/cl_beta_per_rad", &["alpha_deg"]),
    ("/lateral/cn_beta_per_rad", &["alpha_deg"]),
    ("/lateral/cl_p_per_rad", &["alpha_deg"]),
    ("/lateral/cl_r_per_rad", &["alpha_deg"]),
    ("/lateral/cn_p_per_rad", &["alpha_deg"]),
    ("/lateral/cn_r_per_rad", &["alpha_deg"]),
    ("/lateral/cl_da_per_rad", &["alpha_deg"]),
    ("/lateral/cn_da_per_rad", &["alpha_deg"]),
    ("/lateral/cl_dr_per_rad", &["alpha_deg"]),
    ("/lateral/cn_dr_per_rad", &["alpha_deg"]),
    ("/lateral/cy_dr_per_rad", &["alpha_deg"]),
    ("/post_stall/blend_start_deg", &["mach"]),
    ("/post_stall/blend_end_deg", &["mach"]),
    ("/control_effectiveness", &["deflection_deg"]),
];

/// The axis each engine table field must use.
pub const ENGINE_AXES: &[(&str, &[&str])] = &[
    ("/thrust/mach_factor_dry", &["mach"]),
    ("/thrust/mach_factor_ab", &["mach"]),
];

impl AeroFile {
    /// The runtime definition.
    #[must_use]
    pub fn to_def(&self) -> AeroDef {
        let l = &self.lift;
        let d = &self.drag;
        let p = &self.pitch;
        let t = &self.lateral;
        let s = &self.post_stall;
        let c = &self.config;
        AeroDef {
            lift: Lift {
                cl_base: l.cl_base.to_grid(),
                cl_max: l.cl_max.to_curve(),
                cl_alpha_per_rad: l.cl_alpha_per_rad.to_curve(),
                cl0: l.cl0 as f32,
                cl_de_per_rad: l.cl_de_per_rad.to_curve(),
            },
            drag: Drag {
                cd0: d.cd0.to_curve(),
                cd_wave: d.cd_wave.to_curve(),
                k_induced: d.k_induced.to_curve(),
            },
            pitch: Pitch {
                cm_0: p.cm_0.to_grid(),
                cm_de_per_rad: p.cm_de_per_rad.to_curve(),
                cm_q_per_rad: p.cm_q_per_rad as f32,
                cm_alpha_dot_per_rad: p.cm_alpha_dot_per_rad as f32,
            },
            lateral: Lateral {
                cy_beta_per_rad: t.cy_beta_per_rad.to_curve(),
                cl_beta_per_rad: t.cl_beta_per_rad.to_curve(),
                cn_beta_per_rad: t.cn_beta_per_rad.to_curve(),
                cl_p_per_rad: t.cl_p_per_rad.to_curve(),
                cl_r_per_rad: t.cl_r_per_rad.to_curve(),
                cn_p_per_rad: t.cn_p_per_rad.to_curve(),
                cn_r_per_rad: t.cn_r_per_rad.to_curve(),
                cl_da_per_rad: t.cl_da_per_rad.to_curve(),
                cn_da_per_rad: t.cn_da_per_rad.to_curve(),
                cl_dr_per_rad: t.cl_dr_per_rad.to_curve(),
                cn_dr_per_rad: t.cn_dr_per_rad.to_curve(),
                cy_dr_per_rad: t.cy_dr_per_rad.to_curve(),
            },
            post_stall: PostStall {
                blend_start_deg: s.blend_start_deg.to_curve(),
                blend_end_deg: s.blend_end_deg.to_curve(),
                cd90: s.cd90 as f32,
                x_ac_post_mac: s.x_ac_post_mac as f32,
            },
            control_effectiveness: self.control_effectiveness.to_curve(),
            config: ConfigDeltas {
                gear_cd0: c.gear_cd0 as f32,
                flap_to_cd0: c.flap_to_cd0 as f32,
                flap_ld_cd0: c.flap_ld_cd0 as f32,
                brake_cd0: c.brake_cd0 as f32,
                flap_to_cl: c.flap_to_cl as f32,
                flap_ld_cl: c.flap_ld_cl as f32,
            },
            x_ref_mac: self.x_ref_mac as f32,
            ground_effect_k: self.ground_effect_k as f32,
        }
    }

    /// The authoring form of a baked definition.
    #[must_use]
    pub fn from_def(id: &str, a: &AeroDef) -> Self {
        let l = &a.lift;
        let d = &a.drag;
        let p = &a.pitch;
        let t = &a.lateral;
        let s = &a.post_stall;
        let c = &a.config;
        let ad = ["alpha_deg", "mach"];
        Self {
            schema: AERO_SCHEMA.to_owned(),
            id: id.to_owned(),
            lift: LiftFile {
                cl_base: GridFile::from_grid(ad, &l.cl_base),
                cl_max: CurveFile::from_curve("mach", &l.cl_max),
                cl_alpha_per_rad: CurveFile::from_curve("mach", &l.cl_alpha_per_rad),
                cl0: f(l.cl0),
                cl_de_per_rad: CurveFile::from_curve("mach", &l.cl_de_per_rad),
            },
            drag: DragFile {
                cd0: CurveFile::from_curve("mach", &d.cd0),
                cd_wave: CurveFile::from_curve("mach", &d.cd_wave),
                k_induced: CurveFile::from_curve("mach", &d.k_induced),
            },
            pitch: PitchFile {
                cm_0: GridFile::from_grid(ad, &p.cm_0),
                cm_de_per_rad: CurveFile::from_curve("mach", &p.cm_de_per_rad),
                cm_q_per_rad: f(p.cm_q_per_rad),
                cm_alpha_dot_per_rad: f(p.cm_alpha_dot_per_rad),
            },
            lateral: LateralFile {
                cy_beta_per_rad: CurveFile::from_curve("alpha_deg", &t.cy_beta_per_rad),
                cl_beta_per_rad: CurveFile::from_curve("alpha_deg", &t.cl_beta_per_rad),
                cn_beta_per_rad: CurveFile::from_curve("alpha_deg", &t.cn_beta_per_rad),
                cl_p_per_rad: CurveFile::from_curve("alpha_deg", &t.cl_p_per_rad),
                cl_r_per_rad: CurveFile::from_curve("alpha_deg", &t.cl_r_per_rad),
                cn_p_per_rad: CurveFile::from_curve("alpha_deg", &t.cn_p_per_rad),
                cn_r_per_rad: CurveFile::from_curve("alpha_deg", &t.cn_r_per_rad),
                cl_da_per_rad: CurveFile::from_curve("alpha_deg", &t.cl_da_per_rad),
                cn_da_per_rad: CurveFile::from_curve("alpha_deg", &t.cn_da_per_rad),
                cl_dr_per_rad: CurveFile::from_curve("alpha_deg", &t.cl_dr_per_rad),
                cn_dr_per_rad: CurveFile::from_curve("alpha_deg", &t.cn_dr_per_rad),
                cy_dr_per_rad: CurveFile::from_curve("alpha_deg", &t.cy_dr_per_rad),
            },
            post_stall: PostStallFile {
                blend_start_deg: CurveFile::from_curve("mach", &s.blend_start_deg),
                blend_end_deg: CurveFile::from_curve("mach", &s.blend_end_deg),
                cd90: f(s.cd90),
                x_ac_post_mac: f(s.x_ac_post_mac),
            },
            control_effectiveness: CurveFile::from_curve(
                "deflection_deg",
                &a.control_effectiveness,
            ),
            config: ConfigFile {
                gear_cd0: f(c.gear_cd0),
                flap_to_cd0: f(c.flap_to_cd0),
                flap_ld_cd0: f(c.flap_ld_cd0),
                brake_cd0: f(c.brake_cd0),
                flap_to_cl: f(c.flap_to_cl),
                flap_ld_cl: f(c.flap_ld_cl),
            },
            x_ref_mac: f(a.x_ref_mac),
            ground_effect_k: f(a.ground_effect_k),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// fcs/1

/// The control-law gains of one airframe (GDD 11.11, A.2, ADR-34).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FcsGainsFile {
    /// Format tag, always `fcs/1`.
    #[schemars(extend("const" = "fcs/1"))]
    pub schema: String,
    /// `namespace:fcs/name`.
    pub id: String,
    /// Key of the display name in the locale table.
    pub name_key: String,
    /// Pitch axis and the angle-of-attack limiter.
    pub pitch: PitchGainsFile,
    /// Roll axis.
    pub roll: RollGainsFile,
    /// Yaw axis.
    pub yaw: YawGainsFile,
    /// The Full law.
    pub full: FullGainsFile,
    /// The overspeed soft wall.
    pub overspeed: OverspeedGainsFile,
    /// Configuration limits.
    pub config: ConfigGainsFile,
    /// Ground mode.
    pub ground: GroundGainsFile,
    /// Time over which an assist switch blends the surface command.
    #[schemars(extend("x-unit" = "s"), range(min = 0.0, max = 5.0))]
    pub assist_blend_s: f64,
    /// Mouse-aim controller.
    pub mouse_aim: MouseAimGainsFile,
    /// The recovery button.
    pub recover: RecoverGainsFile,
    /// Automatic ground-collision avoidance.
    pub gcas: GcasGainsFile,
}

/// Pitch axis gains.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PitchGainsFile {
    /// Pitch-rate damping gain of the inner loop.
    #[schemars(extend("x-unit" = "1/s"), range(min = 0.0, max = 50.0))]
    pub k_q_per_s: f64,
    /// Bandwidth of the load-factor loop.
    #[schemars(extend("x-unit" = "1/s"), range(min = 0.0, max = 20.0))]
    pub k_n_per_s: f64,
    /// Integral gain on the load-factor error, relative to the bandwidth.
    #[schemars(extend("x-unit" = "1/s"), range(min = 0.0, max = 20.0))]
    pub k_ni_per_s: f64,
    /// Time constant of the command prefilter.
    #[schemars(extend("x-unit" = "s"), range(min = 0.0, max = 2.0))]
    pub tau_cmd_s: f64,
    /// Width of the angle-of-attack limiter band below the limit.
    #[schemars(extend("x-unit" = "deg"), range(min = 0.5, max = 20.0))]
    pub alpha_band_deg: f64,
    /// Angle-of-attack feedback gain inside the band.
    #[schemars(extend("x-unit" = "1/s"), range(min = 0.0, max = 50.0))]
    pub k_alpha_per_s: f64,
    /// Most negative angle of attack the limiter allows.
    #[schemars(extend("x-unit" = "deg"), range(min = -60.0, max = 0.0))]
    pub alpha_min_deg: f64,
    /// Largest commanded pitch rate of the Reduced law against dynamic pressure.
    pub q_max_dps: CurveFile,
}

/// Roll axis gains.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RollGainsFile {
    /// Roll-rate damping gain.
    #[schemars(extend("x-unit" = "1/s"), range(min = 0.0, max = 50.0))]
    pub k_p_per_s: f64,
    /// Integral gain on the roll-rate error.
    #[schemars(extend("x-unit" = "1/s"), range(min = 0.0, max = 20.0))]
    pub k_pi_per_s: f64,
    /// Time constant of the command prefilter.
    #[schemars(extend("x-unit" = "s"), range(min = 0.0, max = 2.0))]
    pub tau_cmd_s: f64,
    /// Fraction of the maximum roll rate available against dynamic pressure.
    pub p_max_qbar: CurveFile,
    /// Fraction of the maximum roll rate available against angle of attack.
    pub p_max_alpha: CurveFile,
    /// Fraction of the maximum roll rate available against load factor.
    pub p_max_nz: CurveFile,
}

/// Yaw axis gains.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct YawGainsFile {
    /// Sideslip feedback gain.
    #[schemars(extend("x-unit" = "1/s"), range(min = 0.0, max = 50.0))]
    pub k_beta_per_s: f64,
    /// Yaw-rate feedback gain.
    #[schemars(extend("x-unit" = "1/s"), range(min = 0.0, max = 50.0))]
    pub k_r_per_s: f64,
    /// Largest sideslip the pedals command, against dynamic pressure.
    pub beta_max_deg: CurveFile,
    /// Time constant of the Reduced-law yaw damper.
    #[schemars(extend("x-unit" = "s"), range(min = 0.01, max = 10.0))]
    pub yaw_damper_s: f64,
    /// Washout time constant of the yaw damper.
    #[schemars(extend("x-unit" = "s"), range(min = 0.01, max = 20.0))]
    pub washout_s: f64,
}

/// The Full law.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FullGainsFile {
    /// Largest commanded flight-path rate.
    #[schemars(extend("x-unit" = "deg/s"), range(min = 1.0, max = 60.0))]
    pub gamma_rate_max_dps: f64,
    /// Flight-path hold gain.
    #[schemars(extend("x-unit" = "1/s"), range(min = 0.0, max = 20.0))]
    pub k_gamma_per_s: f64,
    /// Largest commanded bank angle.
    #[schemars(extend("x-unit" = "deg"), range(min = 10.0, max = 90.0))]
    pub bank_max_deg: f64,
    /// Bank-angle gain.
    #[schemars(extend("x-unit" = "1/s"), range(min = 0.0, max = 20.0))]
    pub k_bank_per_s: f64,
    /// How much tighter than the Standard limit the angle-of-attack limit is.
    #[schemars(extend("x-unit" = "deg"), range(min = 0.0, max = 10.0))]
    pub alpha_margin_deg: f64,
}

/// The overspeed soft wall.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OverspeedGainsFile {
    /// Fraction of `vmo` and `mmo` where the wall begins.
    #[schemars(range(min = 0.5, max = 1.0))]
    pub onset: f64,
    /// Pull-up command added per unit of speed ratio past the onset.
    #[schemars(extend("x-unit" = "g per unit ratio"), range(min = 0.0, max = 500.0))]
    pub k_pull: f64,
}

/// Configuration limits.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfigGainsFile {
    /// Negative load limit with the gear or the flaps out.
    #[schemars(extend("x-unit" = "g"), range(min = -20.0, max = 0.0))]
    pub n_min_config: f64,
}

/// Nose-wheel steering schedule.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NwsGainsFile {
    /// Below this ground speed the full range is available.
    #[schemars(extend("x-unit" = "m/s"), range(min = 0.0, max = 200.0))]
    pub low_speed_mps: f64,
    /// Above this ground speed only the small range is available.
    #[schemars(extend("x-unit" = "m/s"), range(min = 0.0, max = 200.0))]
    pub high_speed_mps: f64,
    /// Full range.
    #[schemars(extend("x-unit" = "deg"), range(min = 0.0, max = 90.0))]
    pub low_deg: f64,
    /// Small range.
    #[schemars(extend("x-unit" = "deg"), range(min = 0.0, max = 90.0))]
    pub high_deg: f64,
}

/// Ground mode gains.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GroundGainsFile {
    /// Weight-on-wheels debounce.
    #[schemars(extend("x-unit" = "s"), range(min = 0.0, max = 2.0))]
    pub wow_debounce_s: f64,
    /// Blend from the ground law to the flight law after lift-off.
    #[schemars(extend("x-unit" = "s"), range(min = 0.0, max = 10.0))]
    pub liftoff_blend_s: f64,
    /// Blend from the flight law to the ground law after touchdown.
    #[schemars(extend("x-unit" = "s"), range(min = 0.0, max = 10.0))]
    pub touchdown_blend_s: f64,
    /// Pitch-rate damping of the ground law.
    #[schemars(extend("x-unit" = "1/s"), range(min = 0.0, max = 5.0))]
    pub k_q_ground_s: f64,
    /// Nose-wheel steering schedule.
    pub nws: NwsGainsFile,
}

/// Mouse-aim gains (GDD 58.4).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MouseAimGainsFile {
    /// Bank command per unit of heading error.
    #[schemars(range(min = 0.0, max = 10.0))]
    pub k_bank: f64,
    /// Pitch command per radian of elevation error.
    #[schemars(extend("x-unit" = "1/rad"), range(min = 0.0, max = 20.0))]
    pub k_pitch_per_rad: f64,
    /// Aim error below which no input is given.
    #[schemars(extend("x-unit" = "deg"), range(min = 0.0, max = 10.0))]
    pub deadzone_deg: f64,
    /// Stick ramp, full scale per second.
    #[schemars(extend("x-unit" = "1/s"), range(min = 0.1, max = 50.0))]
    pub ramp_per_s: f64,
    /// Bank command per unit of bank error when the aim is far (the near value is `k_bank`).
    #[schemars(range(min = 0.0, max = 10.0))]
    pub k_bank_far: f64,
    /// How far ahead along the rate it closes at the nose is looked at.
    #[schemars(extend("x-unit" = "s"), range(min = 0.0, max = 1.0))]
    pub lead_s: f64,
    /// The largest turn rate toward the aim asked for.
    #[schemars(extend("x-unit" = "deg/s"), range(min = 1.0, max = 90.0))]
    pub rate_cap_dps: f64,
    /// The largest pull the aim asks for, as a fraction of the pitch stick.
    #[schemars(range(min = 0.1, max = 1.0))]
    pub pitch_stick_max: f64,
    /// Roll command taken off per unit of roll rate, as a fraction of the largest roll rate.
    #[schemars(range(min = 0.0, max = 5.0))]
    pub k_roll_rate: f64,
}

/// The recovery button.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecoverGainsFile {
    /// Load factor of the recovery pull-up.
    #[schemars(extend("x-unit" = "g"), range(min = 1.0, max = 12.0))]
    pub n_pull: f64,
    /// Flight-path angle the recovery aims for.
    #[schemars(extend("x-unit" = "deg"), range(min = 0.0, max = 45.0))]
    pub gamma_deg: f64,
}

/// Automatic ground-collision avoidance.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GcasGainsFile {
    /// Load factor of the avoidance pull-up.
    #[schemars(extend("x-unit" = "g"), range(min = 1.0, max = 12.0))]
    pub n_pull: f64,
    /// Terrain clearance below which it triggers.
    #[schemars(extend("x-unit" = "m"), range(min = 0.0, max = 1000.0))]
    pub buffer_m: f64,
    /// Look-ahead.
    #[schemars(extend("x-unit" = "s"), range(min = 1.0, max = 30.0))]
    pub horizon_s: f64,
    /// Look-ahead step.
    #[schemars(extend("x-unit" = "s"), range(min = 0.01, max = 1.0))]
    pub step_s: f64,
}

/// The axis each fcs table field must use: `(JSON pointer, axes)`.
pub const FCS_AXES: &[(&str, &[&str])] = &[
    ("/pitch/q_max_dps", &["qbar_kpa"]),
    ("/roll/p_max_qbar", &["qbar_kpa"]),
    ("/roll/p_max_alpha", &["alpha_deg"]),
    ("/roll/p_max_nz", &["nz_g"]),
    ("/yaw/beta_max_deg", &["qbar_kpa"]),
];

impl FcsGainsFile {
    /// The runtime definition.
    #[must_use]
    pub fn to_def(&self) -> FcsDef {
        let (p, r, y, fu) = (&self.pitch, &self.roll, &self.yaw, &self.full);
        let (g, m) = (&self.ground, &self.mouse_aim);
        FcsDef {
            name_key: self.name_key.clone(),
            pitch: PitchGains {
                k_q_per_s: p.k_q_per_s as f32,
                k_n_per_s: p.k_n_per_s as f32,
                k_ni_per_s: p.k_ni_per_s as f32,
                tau_cmd_s: p.tau_cmd_s as f32,
                alpha_band_deg: p.alpha_band_deg as f32,
                k_alpha_per_s: p.k_alpha_per_s as f32,
                alpha_min_deg: p.alpha_min_deg as f32,
                q_max_dps: p.q_max_dps.to_curve(),
            },
            roll: RollGains {
                k_p_per_s: r.k_p_per_s as f32,
                k_pi_per_s: r.k_pi_per_s as f32,
                tau_cmd_s: r.tau_cmd_s as f32,
                p_max_qbar: r.p_max_qbar.to_curve(),
                p_max_alpha: r.p_max_alpha.to_curve(),
                p_max_nz: r.p_max_nz.to_curve(),
            },
            yaw: YawGains {
                k_beta_per_s: y.k_beta_per_s as f32,
                k_r_per_s: y.k_r_per_s as f32,
                yaw_damper_s: y.yaw_damper_s as f32,
                washout_s: y.washout_s as f32,
                beta_max_deg: y.beta_max_deg.to_curve(),
            },
            full: FullGains {
                gamma_rate_max_dps: fu.gamma_rate_max_dps as f32,
                k_gamma_per_s: fu.k_gamma_per_s as f32,
                bank_max_deg: fu.bank_max_deg as f32,
                k_bank_per_s: fu.k_bank_per_s as f32,
                alpha_margin_deg: fu.alpha_margin_deg as f32,
            },
            overspeed: OverspeedGains {
                onset: self.overspeed.onset as f32,
                k_pull: self.overspeed.k_pull as f32,
            },
            config: ConfigGains {
                n_min_config: self.config.n_min_config as f32,
            },
            ground: GroundGains {
                wow_debounce_s: g.wow_debounce_s as f32,
                liftoff_blend_s: g.liftoff_blend_s as f32,
                touchdown_blend_s: g.touchdown_blend_s as f32,
                k_q_ground_s: g.k_q_ground_s as f32,
                nws: NwsGains {
                    low_speed_mps: g.nws.low_speed_mps as f32,
                    high_speed_mps: g.nws.high_speed_mps as f32,
                    low_deg: g.nws.low_deg as f32,
                    high_deg: g.nws.high_deg as f32,
                },
            },
            assist_blend_s: self.assist_blend_s as f32,
            mouse_aim: MouseAimGains {
                k_bank: m.k_bank as f32,
                k_pitch_per_rad: m.k_pitch_per_rad as f32,
                deadzone_deg: m.deadzone_deg as f32,
                ramp_per_s: m.ramp_per_s as f32,
                k_bank_far: m.k_bank_far as f32,
                lead_s: m.lead_s as f32,
                rate_cap_dps: m.rate_cap_dps as f32,
                pitch_stick_max: m.pitch_stick_max as f32,
                k_roll_rate: m.k_roll_rate as f32,
            },
            recover: RecoverGains {
                n_pull: self.recover.n_pull as f32,
                gamma_deg: self.recover.gamma_deg as f32,
            },
            gcas: GcasGains {
                n_pull: self.gcas.n_pull as f32,
                buffer_m: self.gcas.buffer_m as f32,
                horizon_s: self.gcas.horizon_s as f32,
                step_s: self.gcas.step_s as f32,
            },
        }
    }

    /// The authoring form of a baked definition.
    #[must_use]
    pub fn from_def(id: &str, d: &FcsDef) -> Self {
        let (p, r, y, fu) = (&d.pitch, &d.roll, &d.yaw, &d.full);
        let (g, m) = (&d.ground, &d.mouse_aim);
        Self {
            schema: FCS_SCHEMA.to_owned(),
            id: id.to_owned(),
            name_key: d.name_key.clone(),
            pitch: PitchGainsFile {
                k_q_per_s: f(p.k_q_per_s),
                k_n_per_s: f(p.k_n_per_s),
                k_ni_per_s: f(p.k_ni_per_s),
                tau_cmd_s: f(p.tau_cmd_s),
                alpha_band_deg: f(p.alpha_band_deg),
                k_alpha_per_s: f(p.k_alpha_per_s),
                alpha_min_deg: f(p.alpha_min_deg),
                q_max_dps: CurveFile::from_curve("qbar_kpa", &p.q_max_dps),
            },
            roll: RollGainsFile {
                k_p_per_s: f(r.k_p_per_s),
                k_pi_per_s: f(r.k_pi_per_s),
                tau_cmd_s: f(r.tau_cmd_s),
                p_max_qbar: CurveFile::from_curve("qbar_kpa", &r.p_max_qbar),
                p_max_alpha: CurveFile::from_curve("alpha_deg", &r.p_max_alpha),
                p_max_nz: CurveFile::from_curve("nz_g", &r.p_max_nz),
            },
            yaw: YawGainsFile {
                k_beta_per_s: f(y.k_beta_per_s),
                k_r_per_s: f(y.k_r_per_s),
                beta_max_deg: CurveFile::from_curve("qbar_kpa", &y.beta_max_deg),
                yaw_damper_s: f(y.yaw_damper_s),
                washout_s: f(y.washout_s),
            },
            full: FullGainsFile {
                gamma_rate_max_dps: f(fu.gamma_rate_max_dps),
                k_gamma_per_s: f(fu.k_gamma_per_s),
                bank_max_deg: f(fu.bank_max_deg),
                k_bank_per_s: f(fu.k_bank_per_s),
                alpha_margin_deg: f(fu.alpha_margin_deg),
            },
            overspeed: OverspeedGainsFile {
                onset: f(d.overspeed.onset),
                k_pull: f(d.overspeed.k_pull),
            },
            config: ConfigGainsFile {
                n_min_config: f(d.config.n_min_config),
            },
            ground: GroundGainsFile {
                wow_debounce_s: f(g.wow_debounce_s),
                liftoff_blend_s: f(g.liftoff_blend_s),
                touchdown_blend_s: f(g.touchdown_blend_s),
                k_q_ground_s: f(g.k_q_ground_s),
                nws: NwsGainsFile {
                    low_speed_mps: f(g.nws.low_speed_mps),
                    high_speed_mps: f(g.nws.high_speed_mps),
                    low_deg: f(g.nws.low_deg),
                    high_deg: f(g.nws.high_deg),
                },
            },
            assist_blend_s: f(d.assist_blend_s),
            mouse_aim: MouseAimGainsFile {
                k_bank: f(m.k_bank),
                k_pitch_per_rad: f(m.k_pitch_per_rad),
                deadzone_deg: f(m.deadzone_deg),
                ramp_per_s: f(m.ramp_per_s),
                k_bank_far: f(m.k_bank_far),
                lead_s: f(m.lead_s),
                rate_cap_dps: f(m.rate_cap_dps),
                pitch_stick_max: f(m.pitch_stick_max),
                k_roll_rate: f(m.k_roll_rate),
            },
            recover: RecoverGainsFile {
                n_pull: f(d.recover.n_pull),
                gamma_deg: f(d.recover.gamma_deg),
            },
            gcas: GcasGainsFile {
                n_pull: f(d.gcas.n_pull),
                buffer_m: f(d.gcas.buffer_m),
                horizon_s: f(d.gcas.horizon_s),
                step_s: f(d.gcas.step_s),
            },
        }
    }
}
