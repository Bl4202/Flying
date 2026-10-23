//! JSON Schemas generated from the model types, and the lint that keeps their keys honest.

use std::fs;
use std::path::Path;
use std::sync::OnceLock;

use schemars::schema_for;
use serde_json::Value;

use crate::model::{AeroFile, AircraftFile, EngineFile, FcsGainsFile, PackFile};

/// Key suffixes that say what a number is measured in (GDD 62.5, Appendix A).
pub const UNIT_SUFFIXES: &[&str] = &[
    "_kg",
    "_m",
    "_m2",
    "_n",
    "_mps",
    "_kgm2",
    "_per_rad",
    "_mac",
    "_deg",
    "_dps",
    "_s",
    "_pct",
    "_kgpl",
    "_kg_per_ns",
    "_npm",
    "_nspm",
    "_per_s",
    "_per_g",
];

/// Numeric keys that have no unit. Table grids (`x`, `y`) name their units in `axes`; an envelope
/// point's `value` takes the unit of its `metric`.
pub const DIMENSIONLESS: &[&str] = &[
    "crew",
    "hardpoints",
    "count",
    "feed_priority",
    "max_mach",
    "mmo",
    "n_max",
    "n_min",
    "n_cmd_max",
    "gear",
    "flaps",
    "n_min_config",
    "n_pull",
    "k_n",
    "k_bank",
    "k_pull",
    "onset",
    "value",
    "density_exp",
    "idle_fraction",
    "ab_stages",
    "cl0",
    "cd90",
    "ground_effect_k",
    "gear_cd0",
    "flap_to_cd0",
    "flap_ld_cd0",
    "brake_cd0",
    "flap_to_cl",
    "flap_ld_cl",
    "x",
    "y",
];

fn render_schema<T: schemars::JsonSchema>() -> Value {
    serde_json::to_value(schema_for!(T)).expect("schema is JSON")
}

/// The schema of `aircraft/*.json`.
///
/// # Panics
/// Never in practice: the derived schema is always representable as JSON.
#[must_use]
pub fn aircraft_schema() -> Value {
    static CACHE: OnceLock<Value> = OnceLock::new();
    CACHE.get_or_init(render_schema::<AircraftFile>).clone()
}

/// The schema of `engines/*.json`.
///
/// # Panics
/// Never in practice.
#[must_use]
pub fn engine_schema() -> Value {
    static CACHE: OnceLock<Value> = OnceLock::new();
    CACHE.get_or_init(render_schema::<EngineFile>).clone()
}

/// The schema of `aero/*.json`.
///
/// # Panics
/// Never in practice.
#[must_use]
pub fn aero_schema() -> Value {
    static CACHE: OnceLock<Value> = OnceLock::new();
    CACHE.get_or_init(render_schema::<AeroFile>).clone()
}

/// The schema of `fcs/*.json`.
///
/// # Panics
/// Never in practice.
#[must_use]
pub fn fcs_schema() -> Value {
    static CACHE: OnceLock<Value> = OnceLock::new();
    CACHE.get_or_init(render_schema::<FcsGainsFile>).clone()
}

/// The schema of `pack.json`.
///
/// # Panics
/// Never in practice: the derived schema is always representable as JSON.
#[must_use]
pub fn pack_schema() -> Value {
    render_schema::<PackFile>()
}

/// `(file name, schema)` for every schema this tool owns.
#[must_use]
pub fn all() -> Vec<(&'static str, Value)> {
    vec![
        ("aero.schema.json", aero_schema()),
        ("aircraft.schema.json", aircraft_schema()),
        ("engine.schema.json", engine_schema()),
        ("fcs.schema.json", fcs_schema()),
        ("pack.schema.json", pack_schema()),
    ]
}

/// The bytes of a schema file: pretty-printed with a trailing newline.
///
/// # Panics
/// Never for a schema from this module; a `Value` always serialises.
#[must_use]
pub fn render(schema: &Value) -> String {
    let mut s = serde_json::to_string_pretty(schema).expect("schema is JSON");
    s.push('\n');
    s
}

/// Numeric properties whose key names no unit and is not on the dimensionless list.
#[must_use]
pub fn lint_units(schema: &Value) -> Vec<String> {
    let mut problems = Vec::new();
    walk(schema, "", &mut problems);
    problems
}

fn walk(node: &Value, at: &str, problems: &mut Vec<String>) {
    let Value::Object(map) = node else { return };
    if let Some(Value::Object(props)) = map.get("properties") {
        for (key, prop) in props {
            if is_numeric(prop)
                && !DIMENSIONLESS.contains(&key.as_str())
                && !UNIT_SUFFIXES.iter().any(|s| key.ends_with(s))
            {
                problems.push(format!(
                    "{at}/{key}: numeric key has no unit suffix ({}) and is not dimensionless",
                    UNIT_SUFFIXES.join(", ")
                ));
            }
        }
    }
    for (k, v) in map {
        match v {
            Value::Object(_) => walk(v, &format!("{at}/{k}"), problems),
            Value::Array(items) => {
                for (i, item) in items.iter().enumerate() {
                    walk(item, &format!("{at}/{k}/{i}"), problems);
                }
            }
            _ => {}
        }
    }
}

fn is_numeric(prop: &Value) -> bool {
    let numeric =
        |t: Option<&Value>| matches!(t.and_then(Value::as_str), Some("number" | "integer"));
    match prop.get("type").and_then(Value::as_str) {
        Some("array") => numeric(prop.get("items").and_then(|i| i.get("type"))),
        _ => numeric(prop.get("type")),
    }
}

/// Writes every schema into `dir`.
///
/// # Errors
/// A message naming the file that could not be written.
pub fn write(dir: &Path) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for (name, schema) in all() {
        let path = dir.join(name);
        fs::write(&path, render(&schema)).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(())
}

/// The files in `dir` that differ from what [`write`] would produce.
#[must_use]
pub fn stale(dir: &Path) -> Vec<String> {
    all()
        .into_iter()
        .filter(|(name, schema)| {
            fs::read_to_string(dir.join(name)).ok().as_deref() != Some(render(schema).as_str())
        })
        .map(|(name, _)| dir.join(name).display().to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_numeric_key_in_the_schemas_names_its_unit() {
        for (name, schema) in all() {
            assert_eq!(lint_units(&schema), Vec::<String>::new(), "{name}");
        }
    }

    #[test]
    fn the_lint_catches_a_bare_number() {
        let bad = serde_json::json!({
            "properties": {
                "mass": { "type": "number" },
                "empty_kg": { "type": "number" },
                "inertia": { "type": "array", "items": { "type": "number" } },
                "nested": { "properties": { "span": { "type": "integer" } } },
                "label": { "type": "string" },
                "count": { "type": "integer" }
            }
        });
        let problems = lint_units(&bad);
        assert_eq!(problems.len(), 3, "{problems:?}");
        assert!(problems.iter().any(|p| p.contains("/mass")));
        assert!(problems.iter().any(|p| p.contains("/inertia")));
        assert!(problems.iter().any(|p| p.contains("/span")));
    }

    #[test]
    fn the_aircraft_schema_is_strict_and_documents_units() {
        let s = aircraft_schema();
        assert_eq!(s["additionalProperties"], false);
        assert_eq!(s["properties"]["category"]["enum"][0], "light_fighter");
        let mass = &s["$defs"]["MassFile"];
        assert_eq!(mass["additionalProperties"], false);
        assert_eq!(mass["properties"]["empty_kg"]["x-unit"], "kg");
        assert!(mass["properties"]["empty_kg"]["minimum"].is_number());
        assert_eq!(s["properties"]["schema"]["const"], "aircraft/1");
    }

    #[test]
    fn the_table_schemas_name_their_axes() {
        let s = aero_schema();
        let axes = &s["$defs"]["AxisName"]["enum"];
        assert_eq!(axes, &serde_json::json!(crate::model::AXES));
        assert_eq!(s["properties"]["schema"]["const"], "aero/1");
        assert_eq!(engine_schema()["properties"]["schema"]["const"], "engine/1");
    }

    #[test]
    fn rendering_is_stable() {
        assert_eq!(render(&aircraft_schema()), render(&aircraft_schema()));
        assert!(render(&pack_schema()).ends_with("}\n"));
    }
}
