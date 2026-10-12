//! JSON Schemas generated from the model types, and the lint that keeps their keys honest.

use std::fs;
use std::path::Path;
use std::sync::OnceLock;

use schemars::schema_for;
use serde_json::Value;

use crate::model::{AircraftFile, PackFile};

/// Key suffixes that say what a number is measured in (GDD 62.5).
pub const UNIT_SUFFIXES: &[&str] = &["_kg", "_m", "_m2", "_n", "_ms", "_kgm2", "_per_rad", "_mac"];

/// Numeric keys that have no unit.
pub const DIMENSIONLESS: &[&str] = &[
    "crew",
    "hardpoints",
    "count",
    "cd0",
    "induced_k",
    "cl_max",
    "n_max",
    "n_min",
    "mmo",
];

/// The schema of `aircraft/*.json`.
///
/// # Panics
/// Never in practice: the derived schema is always representable as JSON.
#[must_use]
pub fn aircraft_schema() -> Value {
    static CACHE: OnceLock<Value> = OnceLock::new();
    CACHE
        .get_or_init(|| serde_json::to_value(schema_for!(AircraftFile)).expect("schema is JSON"))
        .clone()
}

/// The schema of `pack.json`.
///
/// # Panics
/// Never in practice: the derived schema is always representable as JSON.
#[must_use]
pub fn pack_schema() -> Value {
    serde_json::to_value(schema_for!(PackFile)).expect("schema is JSON")
}

/// `(file name, schema)` for every schema this tool owns.
#[must_use]
pub fn all() -> Vec<(&'static str, Value)> {
    vec![
        ("aircraft.schema.json", aircraft_schema()),
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
        let mass = &s["$defs"]["Mass"];
        assert_eq!(mass["additionalProperties"], false);
        assert_eq!(mass["properties"]["empty_kg"]["x-unit"], "kg");
        assert!(mass["properties"]["empty_kg"]["minimum"].is_number());
        assert_eq!(s["properties"]["schema"]["const"], "aircraft/1");
    }

    #[test]
    fn rendering_is_stable() {
        assert_eq!(render(&aircraft_schema()), render(&aircraft_schema()));
        assert!(render(&pack_schema()).ends_with("}\n"));
    }
}
