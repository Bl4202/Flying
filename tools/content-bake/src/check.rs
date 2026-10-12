//! Loading and validating one authoring file (GDD 62.6).
//!
//! Order matters for useful messages: syntax, then structure (types, required and unknown keys,
//! with the path serde reached), then numeric ranges taken from the generated JSON Schema, then
//! `f32` exactness, then cross-field physics and references. A value that already has an
//! error is not blamed again by the checks that depend on it.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::diag::{Diagnostic, pointer};
use crate::model::{AIRCRAFT_SCHEMA, AircraftFile, PACK_SCHEMA, PackFile};
use content::{AircraftDef, Category, Era};

/// Standard gravity, m/s².
const G: f64 = 9.806_65;
/// Wing loading outside this range (kg/m² at reference weight) is a data error (GDD 62.6).
const WING_LOADING_KG_M2: (f64, f64) = (100.0, 900.0);
/// Thrust-to-weight outside this range is a data error (GDD 62.6).
const THRUST_TO_WEIGHT: (f64, f64) = (0.3, 2.5);
/// Fraction of internal fuel in the reference weight (GDD 13.4).
const REFERENCE_FUEL_FRACTION: f64 = 0.6;

/// Collects diagnostics for one file.
struct Ctx<'a> {
    file: &'a str,
    out: Vec<Diagnostic>,
    flagged: BTreeSet<String>,
}

impl Ctx<'_> {
    fn err(&mut self, ptr: &str, message: impl Into<String>, hint: impl Into<String>) {
        self.flagged.insert(ptr.to_owned());
        self.out
            .push(Diagnostic::new(self.file, ptr, message, hint));
    }

    /// True if none of the pointers has been flagged yet.
    fn clean(&self, pointers: &[&str]) -> bool {
        pointers.iter().all(|p| !self.flagged.contains(*p))
    }
}

/// Reads `text` as JSON and returns the value, or a syntax diagnostic.
fn parse_json(file: &str, text: &str) -> Result<Value, Vec<Diagnostic>> {
    serde_json::from_str(text).map_err(|e| {
        vec![Diagnostic::new(
            file,
            "",
            format!("not valid JSON: {e}"),
            format!(
                "fix the syntax near line {}, column {}",
                e.line(),
                e.column()
            ),
        )]
    })
}

/// Deserialises with path tracking and turns failures into diagnostics.
fn decode<T: serde::de::DeserializeOwned>(file: &str, value: &Value) -> Result<T, Vec<Diagnostic>> {
    serde_path_to_error::deserialize::<_, T>(value).map_err(|e| {
        use serde_path_to_error::Segment;
        let mut segments: Vec<String> = e
            .path()
            .iter()
            .map(|s| match s {
                Segment::Seq { index } => index.to_string(),
                Segment::Map { key } => key.clone(),
                Segment::Enum { variant } => variant.clone(),
                Segment::Unknown => "?".to_owned(),
            })
            .collect();
        let msg = e.inner().to_string();
        // A missing key is reported against its parent, so name the key itself. An unknown key
        // is already the last path segment.
        let (what, hint) =
            if let Some(key) = quoted(&msg).filter(|_| msg.starts_with("unknown field")) {
                if segments.last().map(String::as_str) != Some(key) {
                    segments.push(key.to_owned());
                }
                (
                    format!("unknown key `{key}`"),
                    format!("remove it or fix the spelling; {}", expected_list(&msg)),
                )
            } else if let Some(key) = quoted(&msg).filter(|_| msg.starts_with("missing field")) {
                segments.push(key.to_owned());
                (
                    format!("required key `{key}` is missing"),
                    format!("add `{key}`; see the schema for its unit and range"),
                )
            } else {
                (
                    msg.split(" at line ").next().unwrap_or(&msg).to_owned(),
                    "use a value of the type the schema declares".to_owned(),
                )
            };
        let refs: Vec<&str> = segments.iter().map(String::as_str).collect();
        vec![Diagnostic::new(file, pointer(&refs), what, hint)]
    })
}

/// The first backtick-quoted word of a serde message.
fn quoted(msg: &str) -> Option<&str> {
    let start = msg.find('`')? + 1;
    let len = msg[start..].find('`')?;
    Some(&msg[start..start + len])
}

fn expected_list(msg: &str) -> String {
    msg.split_once(", expected ").map_or_else(
        || "see the schema for the allowed keys".to_owned(),
        |(_, rest)| {
            format!(
                "allowed keys: {}",
                rest.strip_prefix("one of ").unwrap_or(rest)
            )
        },
    )
}

/// Loads `pack.json`.
///
/// # Errors
/// Diagnostics for syntax, structure and the pack's own rules.
pub fn load_pack(file: &str, text: &str) -> Result<PackFile, Vec<Diagnostic>> {
    let value = parse_json(file, text)?;
    let pack: PackFile = decode(file, &value)?;
    let mut c = Ctx {
        file,
        out: Vec::new(),
        flagged: BTreeSet::new(),
    };
    if pack.schema != PACK_SCHEMA {
        c.err(
            "/schema",
            format!("`{}` is not a pack manifest", pack.schema),
            format!("set it to `{PACK_SCHEMA}`"),
        );
    }
    if !is_slug(&pack.id) {
        c.err(
            "/id",
            format!("`{}` is not a valid pack id", pack.id),
            "use lower-case letters, digits and `_`, starting with a letter",
        );
    }
    if !is_semver(&pack.version) {
        c.err(
            "/version",
            format!("`{}` is not a version", pack.version),
            "use `major.minor.patch`, for example `0.0.2`",
        );
    }
    if pack.authors.is_empty() {
        c.err("/authors", "no authors listed", "name at least one author");
    }
    if c.out.is_empty() {
        Ok(pack)
    } else {
        Err(c.out)
    }
}

/// Loads the default locale table (`{ key: text }`).
///
/// # Errors
/// Diagnostics for syntax, structure and empty texts.
pub fn load_locale(file: &str, text: &str) -> Result<BTreeMap<String, String>, Vec<Diagnostic>> {
    let value = parse_json(file, text)?;
    let table: BTreeMap<String, String> = decode(file, &value)?;
    let out: Vec<Diagnostic> = table
        .iter()
        .filter(|(_, v)| v.trim().is_empty())
        .map(|(k, _)| {
            Diagnostic::new(
                file,
                pointer(&[k]),
                "the text is empty",
                "write the display text, or remove the key",
            )
        })
        .collect();
    if out.is_empty() { Ok(table) } else { Err(out) }
}

/// A validated aircraft file and the definition packed from it.
#[derive(Debug)]
pub struct Loaded {
    /// The authoring form.
    pub file: AircraftFile,
    /// The runtime form.
    pub def: AircraftDef,
}

/// Loads and validates one aircraft file. `locale` is the default locale table.
///
/// # Errors
/// Every problem found, each with its pointer.
pub fn load_aircraft(
    file: &str,
    text: &str,
    pack_id: &str,
    locale: &BTreeMap<String, String>,
) -> Result<Loaded, Vec<Diagnostic>> {
    let value = parse_json(file, text)?;
    let aircraft: AircraftFile = decode(file, &value)?;
    let mut c = Ctx {
        file,
        out: Vec::new(),
        flagged: BTreeSet::new(),
    };
    let schema = crate::schema::aircraft_schema();
    check_ranges(&mut c, &schema, &schema, &value, "");
    check_f32_exact(&mut c, &value, "");
    check_semantics(&mut c, &aircraft, pack_id, locale);
    if !c.out.is_empty() {
        return Err(c.out);
    }
    // The semantic checks have vouched for the names, so this only fails on a baker bug.
    let def = aircraft.to_def().ok_or_else(|| {
        vec![Diagnostic::new(
            file,
            "",
            "category or era could not be resolved after validation",
            "this is a baker bug; report it",
        )]
    })?;
    Ok(Loaded {
        file: aircraft,
        def,
    })
}

// ---------------------------------------------------------------------------------------------
// Numeric ranges from the schema.

/// Resolves a local `$ref` such as `#/$defs/Mass`.
fn resolve<'a>(root: &'a Value, schema: &'a Value) -> &'a Value {
    schema
        .get("$ref")
        .and_then(Value::as_str)
        .and_then(|r| r.strip_prefix('#'))
        .and_then(|path| root.pointer(path))
        .unwrap_or(schema)
}

/// Checks `minimum`/`maximum` of every numeric value the schema describes.
fn check_ranges(c: &mut Ctx<'_>, root: &Value, schema: &Value, value: &Value, at: &str) {
    let schema = resolve(root, schema);
    match value {
        Value::Object(map) => {
            let Some(props) = schema.get("properties").and_then(Value::as_object) else {
                return;
            };
            for (key, v) in map {
                if let Some(s) = props.get(key) {
                    let child = format!("{at}{}", pointer(&[key]));
                    check_ranges(c, root, s, v, &child);
                }
            }
        }
        Value::Array(items) => {
            if let Some(s) = schema.get("items") {
                for (i, v) in items.iter().enumerate() {
                    check_ranges(c, root, s, v, &format!("{at}/{i}"));
                }
            }
        }
        Value::Number(n) => {
            let Some(x) = n.as_f64() else { return };
            let unit = schema.get("x-unit").and_then(Value::as_str).unwrap_or("");
            let (lo, hi) = (
                schema.get("minimum").and_then(Value::as_f64),
                schema.get("maximum").and_then(Value::as_f64),
            );
            let out_of_range = lo.is_some_and(|m| x < m) || hi.is_some_and(|m| x > m);
            if out_of_range {
                let bounds = match (lo, hi) {
                    (Some(l), Some(h)) => format!("between {l} and {h}"),
                    (Some(l), None) => format!("at least {l}"),
                    (None, Some(h)) => format!("at most {h}"),
                    (None, None) => unreachable!("out of range without a bound"),
                };
                c.err(
                    at,
                    format!("{x} is outside the allowed range"),
                    format!("use a value {bounds} {unit}").trim_end().to_owned(),
                );
            }
        }
        _ => {}
    }
}

/// Every number is stored as `f32`; refuse any that would be rounded on the way in.
fn check_f32_exact(c: &mut Ctx<'_>, value: &Value, at: &str) {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                check_f32_exact(c, v, &format!("{at}{}", pointer(&[k])));
            }
        }
        Value::Array(items) => {
            for (i, v) in items.iter().enumerate() {
                check_f32_exact(c, v, &format!("{at}/{i}"));
            }
        }
        Value::Number(n) => {
            if let Some(x) = n.as_f64() {
                let stored = x as f32;
                let back = stored.to_string().parse::<f64>().ok();
                if back != Some(x) && c.clean(&[at]) {
                    c.err(
                        at,
                        format!("{x} has more precision than a pack stores (single precision)"),
                        format!("write {stored} instead"),
                    );
                }
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------------------------
// Cross-field checks.

fn check_semantics(
    c: &mut Ctx<'_>,
    a: &AircraftFile,
    pack_id: &str,
    locale: &BTreeMap<String, String>,
) {
    check_identity(c, a, pack_id, locale);
    check_mass_and_geometry(c, a);
    check_inertia(c, a);
    check_propulsion_and_aero(c, a);
}

/// Mass at which wing loading and thrust-to-weight are judged: empty plus 60 % of the fuel.
fn reference_kg(a: &AircraftFile) -> f64 {
    a.mass.empty_kg + REFERENCE_FUEL_FRACTION * a.mass.internal_fuel_kg
}

/// True if the masses the derived checks rely on have no error of their own.
fn masses_ok(c: &Ctx<'_>) -> bool {
    c.clean(&["/mass/empty_kg", "/mass/internal_fuel_kg"])
}

/// Tags, ids, names and references.
fn check_identity(
    c: &mut Ctx<'_>,
    a: &AircraftFile,
    pack_id: &str,
    locale: &BTreeMap<String, String>,
) {
    if a.schema != AIRCRAFT_SCHEMA {
        c.err(
            "/schema",
            format!("`{}` is not an aircraft file", a.schema),
            format!("set it to `{AIRCRAFT_SCHEMA}`"),
        );
    }
    if let Err(hint) = check_id(&a.id, pack_id, "ac") {
        c.err(
            "/id",
            format!("`{}` is not a valid aircraft id", a.id),
            hint,
        );
    }
    if Category::from_name(&a.category).is_none() {
        let names: Vec<&str> = Category::ALL.iter().map(|x| x.name()).collect();
        c.err(
            "/category",
            format!("`{}` is not a known category", a.category),
            format!("use one of: {}", names.join(", ")),
        );
    }
    if Era::from_name(&a.era).is_none() {
        let names: Vec<&str> = Era::ALL.iter().map(|x| x.name()).collect();
        c.err(
            "/era",
            format!("`{}` is not a known era", a.era),
            format!("use one of: {}", names.join(", ")),
        );
    }
    if !locale.contains_key(&a.name_key) {
        c.err(
            "/name_key",
            format!("locale key `{}` is not in the default locale", a.name_key),
            format!(
                "add \"{}\" to locales/en.json, or use an existing key",
                a.name_key
            ),
        );
    }
}

/// Masses and wing geometry, and the wing loading they imply.
fn check_mass_and_geometry(c: &mut Ctx<'_>, a: &AircraftFile) {
    let m = &a.mass;
    if c.clean(&[
        "/mass/empty_kg",
        "/mass/internal_fuel_kg",
        "/mass/max_takeoff_kg",
    ]) && m.max_takeoff_kg < m.empty_kg + m.internal_fuel_kg
    {
        c.err(
            "/mass/max_takeoff_kg",
            format!(
                "maximum take-off mass {} kg is below empty mass plus internal fuel ({} kg)",
                m.max_takeoff_kg,
                m.empty_kg + m.internal_fuel_kg
            ),
            "raise max_takeoff_kg, or correct the empty or fuel mass",
        );
    }

    let g = &a.geometry;
    if c.clean(&["/geometry/span_m", "/geometry/mac_m"]) && g.span_m <= g.mac_m {
        c.err(
            "/geometry/span_m",
            format!(
                "span {} m is not larger than the chord {} m",
                g.span_m, g.mac_m
            ),
            "check span_m and mac_m; span is the full wingtip-to-wingtip width",
        );
    }

    let reference_kg = reference_kg(a);
    if masses_ok(c) && c.clean(&["/geometry/wing_area_m2"]) {
        let wl = reference_kg / g.wing_area_m2;
        if !(WING_LOADING_KG_M2.0..=WING_LOADING_KG_M2.1).contains(&wl) {
            c.err(
                "/geometry/wing_area_m2",
                format!(
                    "wing loading is {wl:.0} kg/m2 at reference weight ({reference_kg:.0} kg), outside {}-{}",
                    WING_LOADING_KG_M2.0, WING_LOADING_KG_M2.1
                ),
                "check wing_area_m2 and the masses",
            );
        }
    }
}

/// Thrust, thrust-to-weight and the stability data.
fn check_propulsion_and_aero(c: &mut Ctx<'_>, a: &AircraftFile) {
    let e = &a.engines;
    if c.clean(&[
        "/engines/thrust_military_n",
        "/engines/thrust_afterburner_n",
    ]) && e.thrust_afterburner_n < e.thrust_military_n
    {
        c.err(
            "/engines/thrust_afterburner_n",
            "afterburner thrust is below military thrust",
            "set it equal to thrust_military_n for an engine with no afterburner",
        );
    }
    if masses_ok(c)
        && c.clean(&[
            "/engines/thrust_afterburner_n",
            "/engines/thrust_military_n",
        ])
    {
        let t_w = e.thrust_afterburner_n.max(e.thrust_military_n) / (reference_kg(a) * G);
        if !(THRUST_TO_WEIGHT.0..=THRUST_TO_WEIGHT.1).contains(&t_w) {
            c.err(
                "/engines/thrust_afterburner_n",
                format!(
                    "thrust-to-weight is {t_w:.2} at reference weight, outside {}-{}",
                    THRUST_TO_WEIGHT.0, THRUST_TO_WEIGHT.1
                ),
                "thrust is for all engines together; check it and the masses",
            );
        }
    }

    if a.aero.static_margin_mac < 0.0 && !a.aero.sas_required {
        c.err(
            "/aero/sas_required",
            "the static margin is negative, so the airframe is unstable without augmentation",
            "set sas_required to true, or correct static_margin_mac",
        );
    }
}

fn check_inertia(c: &mut Ctx<'_>, a: &AircraftFile) {
    let [xx, yy, zz] = a.mass.inertia_kgm2;
    let mut positive = true;
    for (i, v) in [xx, yy, zz].into_iter().enumerate() {
        if v <= 0.0 {
            positive = false;
            c.err(
                &format!("/mass/inertia_kgm2/{i}"),
                format!("inertia {v} kg m2 is not positive"),
                "inertia must be greater than zero",
            );
        }
    }
    if !positive {
        return;
    }
    // A rigid body's principal moments cannot violate the triangle inequalities.
    if xx + yy < zz || yy + zz < xx || xx + zz < yy {
        c.err(
            "/mass/inertia_kgm2",
            format!("inertia ({xx}, {yy}, {zz}) cannot belong to a rigid body"),
            "the sum of any two moments must be at least the third; check the order [roll, pitch, yaw]",
        );
        return;
    }
    let xz = a.mass.inertia_xz_kgm2;
    if xz * xz >= xx * zz {
        c.err(
            "/mass/inertia_xz_kgm2",
            format!("the product of inertia {xz} is too large for the inertia tensor to be positive definite"),
            "its square must be smaller than inertia_kgm2[0] * inertia_kgm2[2]",
        );
    }
}

/// `namespace:type/name`, each part lower-case letters, digits, `_` or `-`, starting with a letter.
fn check_id(id: &str, pack_id: &str, kind: &str) -> Result<(), String> {
    let shape = "ids look like `core:ac/kestrel` (namespace:type/name; lower-case letters, digits, `_` and `-`)";
    let (ns, rest) = id.split_once(':').ok_or_else(|| shape.to_owned())?;
    let (ty, name) = rest.split_once('/').ok_or_else(|| shape.to_owned())?;
    if ![ns, ty, name].iter().all(|s| is_segment(s)) {
        return Err(shape.to_owned());
    }
    if ns != pack_id {
        return Err(format!("the namespace must be the pack id `{pack_id}`"));
    }
    if ty != kind {
        return Err(format!("aircraft ids use the type `{kind}`"));
    }
    Ok(())
}

fn is_segment(s: &str) -> bool {
    let mut chars = s.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

fn is_slug(s: &str) -> bool {
    is_segment(s) && !s.contains('-')
}

fn is_semver(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_follow_the_pattern() {
        assert!(check_id("core:ac/kestrel", "core", "ac").is_ok());
        assert!(check_id("core:ac/aim-ir_2", "core", "ac").is_ok());
        for bad in [
            "kestrel",
            "core:kestrel",
            "core:ac/",
            "Core:ac/x",
            "core:ac/Kes",
            "core:ac/1x",
            "core:ac/a b",
        ] {
            assert!(check_id(bad, "core", "ac").is_err(), "{bad}");
        }
        assert!(
            check_id("other:ac/x", "core", "ac")
                .unwrap_err()
                .contains("pack id")
        );
        assert!(
            check_id("core:wpn/x", "core", "ac")
                .unwrap_err()
                .contains("type")
        );
    }

    #[test]
    fn versions_and_slugs() {
        assert!(is_semver("0.0.2") && is_semver("10.20.30"));
        assert!(!is_semver("1.2") && !is_semver("1.2.x") && !is_semver("1..3"));
        assert!(is_slug("core") && is_slug("my_pack") && !is_slug("my-pack") && !is_slug("Core"));
    }

    #[test]
    fn serde_messages_yield_the_key() {
        assert_eq!(
            quoted("unknown field `foo`, expected one of `a`, `b`"),
            Some("foo")
        );
        assert_eq!(quoted("missing field `mac_m`"), Some("mac_m"));
        assert_eq!(quoted("no ticks"), None);
        assert!(expected_list("unknown field `x`, expected one of `a`").contains("`a`"));
    }
}
