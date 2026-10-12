//! The authoring formats: strict serde types, which are also the source of the JSON Schemas.
//!
//! Every number carries its unit in its key (`_kg`, `_m`, ...) except the few dimensionless ones
//! listed in [`crate::schema::DIMENSIONLESS`]. Unknown keys are errors, so a typo cannot be
//! silently ignored.

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize};

use content::{AircraftDef, Category, Era};

/// Tag of the pack manifest format.
pub const PACK_SCHEMA: &str = "pack/1";
/// Tag of the aircraft format.
pub const AIRCRAFT_SCHEMA: &str = "aircraft/1";

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

/// One aircraft (GDD 62.2, Appendix C).
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
    pub mass: Mass,
    /// Wing geometry.
    pub geometry: Geometry,
    /// Propulsion.
    pub engines: Engines,
    /// Aerodynamic reference data.
    pub aero: Aero,
    /// Structural and operating limits.
    pub limits: Limits,
}

/// Mass properties.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Mass {
    /// Empty mass.
    #[schemars(extend("x-unit" = "kg"), range(min = 1.0, max = 1.0e6))]
    pub empty_kg: f64,
    /// Internal fuel.
    #[schemars(extend("x-unit" = "kg"), range(min = 1.0, max = 1.0e6))]
    pub internal_fuel_kg: f64,
    /// Maximum take-off mass.
    #[schemars(extend("x-unit" = "kg"), range(min = 1.0, max = 1.0e6))]
    pub max_takeoff_kg: f64,
    /// Roll, pitch and yaw inertia at reference weight.
    #[schemars(extend("x-unit" = "kg m2"))]
    pub inertia_kgm2: [f64; 3],
    /// Product of inertia XZ (zero for a symmetric airframe).
    #[serde(default)]
    #[schemars(extend("x-unit" = "kg m2"))]
    pub inertia_xz_kgm2: f64,
}

/// Wing geometry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Geometry {
    /// Wing reference area.
    #[schemars(extend("x-unit" = "m2"), range(min = 0.1, max = 1000.0))]
    pub wing_area_m2: f64,
    /// Wing span.
    #[schemars(extend("x-unit" = "m"), range(min = 0.1, max = 100.0))]
    pub span_m: f64,
    /// Mean aerodynamic chord.
    #[schemars(extend("x-unit" = "m"), range(min = 0.1, max = 50.0))]
    pub mac_m: f64,
}

/// Propulsion.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Engines {
    /// Number of engines.
    #[schemars(range(min = 1, max = 8))]
    pub count: u8,
    /// Sea-level static thrust at military power, all engines.
    #[schemars(extend("x-unit" = "N"), range(min = 1.0, max = 5.0e6))]
    pub thrust_military_n: f64,
    /// Sea-level static thrust with afterburner, all engines (equal to military if none).
    #[schemars(extend("x-unit" = "N"), range(min = 1.0, max = 5.0e6))]
    pub thrust_afterburner_n: f64,
}

/// Aerodynamic reference data.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Aero {
    /// Zero-lift drag coefficient.
    #[schemars(range(min = 0.001, max = 0.5))]
    pub cd0: f64,
    /// Induced-drag factor K.
    #[schemars(range(min = 0.001, max = 1.0))]
    pub induced_k: f64,
    /// Lift-curve slope.
    #[schemars(extend("x-unit" = "1/rad"), range(min = 0.5, max = 10.0))]
    pub cl_alpha_per_rad: f64,
    /// Clean maximum lift coefficient at low Mach.
    #[schemars(range(min = 0.1, max = 5.0))]
    pub cl_max: f64,
    /// Static margin as a fraction of the mean aerodynamic chord (negative if unstable).
    #[schemars(extend("x-unit" = "fraction of MAC"), range(min = -0.5, max = 0.5))]
    pub static_margin_mac: f64,
    /// True if the airframe cannot be flown without stability augmentation.
    pub sas_required: bool,
}

/// Structural and operating limits.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    /// Structural positive load factor.
    #[schemars(extend("x-unit" = "g"), range(min = 1.0, max = 20.0))]
    pub n_max: f64,
    /// Structural negative load factor (below zero).
    #[schemars(extend("x-unit" = "g"), range(min = -20.0, max = -0.1))]
    pub n_min: f64,
    /// Maximum operating Mach number.
    #[schemars(range(min = 0.1, max = 10.0))]
    pub mmo: f64,
    /// Maximum operating speed.
    #[schemars(extend("x-unit" = "m/s"), range(min = 10.0, max = 2000.0))]
    pub vmo_ms: f64,
    /// Service ceiling.
    #[schemars(extend("x-unit" = "m"), range(min = 100.0, max = 40000.0))]
    pub service_ceiling_m: f64,
}

fn category_schema(_: &mut SchemaGenerator) -> Schema {
    let names: Vec<&str> = Category::ALL.iter().map(|c| c.name()).collect();
    json_schema!({ "type": "string", "enum": names })
}

fn era_schema(_: &mut SchemaGenerator) -> Schema {
    let names: Vec<&str> = Era::ALL.iter().map(|e| e.name()).collect();
    json_schema!({ "type": "string", "enum": names })
}

impl AircraftFile {
    /// The authoring form of a baked definition. Floats come back as the shortest decimal that
    /// reads back as the same `f32`, which is what the baker required of the input.
    #[must_use]
    pub fn from_def(id: &str, d: &AircraftDef) -> Self {
        let f = |v: f32| v.to_string().parse::<f64>().unwrap_or(f64::from(v));
        Self {
            schema: AIRCRAFT_SCHEMA.to_owned(),
            id: id.to_owned(),
            name_key: d.name_key.clone(),
            category: d.category.name().to_owned(),
            era: d.era.name().to_owned(),
            crew: d.crew,
            hardpoints: d.hardpoints,
            mass: Mass {
                empty_kg: f(d.empty_kg),
                internal_fuel_kg: f(d.internal_fuel_kg),
                max_takeoff_kg: f(d.max_takeoff_kg),
                inertia_kgm2: [
                    f(d.inertia_xx_kgm2),
                    f(d.inertia_yy_kgm2),
                    f(d.inertia_zz_kgm2),
                ],
                inertia_xz_kgm2: f(d.inertia_xz_kgm2),
            },
            geometry: Geometry {
                wing_area_m2: f(d.wing_area_m2),
                span_m: f(d.span_m),
                mac_m: f(d.mac_m),
            },
            engines: Engines {
                count: d.engine_count,
                thrust_military_n: f(d.thrust_mil_n),
                thrust_afterburner_n: f(d.thrust_ab_n),
            },
            aero: Aero {
                cd0: f(d.cd0),
                induced_k: f(d.induced_k),
                cl_alpha_per_rad: f(d.cl_alpha_per_rad),
                cl_max: f(d.cl_max),
                static_margin_mac: f(d.static_margin_mac),
                sas_required: d.sas_required,
            },
            limits: Limits {
                n_max: f(d.n_max),
                n_min: f(d.n_min),
                mmo: f(d.mmo),
                vmo_ms: f(d.vmo_ms),
                service_ceiling_m: f(d.service_ceiling_m),
            },
        }
    }

    /// The runtime definition. `None` if `category` or `era` is not a known name; the checks in
    /// [`crate::check`] report those with a pointer before this is called.
    #[must_use]
    pub fn to_def(&self) -> Option<AircraftDef> {
        Some(AircraftDef {
            name_key: self.name_key.clone(),
            category: Category::from_name(&self.category)?,
            era: Era::from_name(&self.era)?,
            crew: self.crew,
            engine_count: self.engines.count,
            hardpoints: self.hardpoints,
            sas_required: self.aero.sas_required,
            empty_kg: self.mass.empty_kg as f32,
            internal_fuel_kg: self.mass.internal_fuel_kg as f32,
            max_takeoff_kg: self.mass.max_takeoff_kg as f32,
            wing_area_m2: self.geometry.wing_area_m2 as f32,
            span_m: self.geometry.span_m as f32,
            mac_m: self.geometry.mac_m as f32,
            inertia_xx_kgm2: self.mass.inertia_kgm2[0] as f32,
            inertia_yy_kgm2: self.mass.inertia_kgm2[1] as f32,
            inertia_zz_kgm2: self.mass.inertia_kgm2[2] as f32,
            inertia_xz_kgm2: self.mass.inertia_xz_kgm2 as f32,
            n_max: self.limits.n_max as f32,
            n_min: self.limits.n_min as f32,
            mmo: self.limits.mmo as f32,
            vmo_ms: self.limits.vmo_ms as f32,
            service_ceiling_m: self.limits.service_ceiling_m as f32,
            thrust_mil_n: self.engines.thrust_military_n as f32,
            thrust_ab_n: self.engines.thrust_afterburner_n as f32,
            cd0: self.aero.cd0 as f32,
            induced_k: self.aero.induced_k as f32,
            cl_alpha_per_rad: self.aero.cl_alpha_per_rad as f32,
            cl_max: self.aero.cl_max as f32,
            static_margin_mac: self.aero.static_margin_mac as f32,
        })
    }
}
