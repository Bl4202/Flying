//! Loading and validating one authoring file (GDD 62.6).
//!
//! Order matters for useful messages: syntax, then structure (types, required and unknown keys,
//! with the path serde reached), then numeric ranges taken from the generated JSON Schema, then
//! `f32` exactness, then cross-field physics and references. A value that already has an
//! error is not blamed again by the checks that depend on it.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::diag::{Diagnostic, pointer};
use crate::model::{
    AERO_AXES, AERO_SCHEMA, AIRCRAFT_SCHEMA, AeroFile, AircraftFile, CurveFile, ENGINE_AXES,
    ENGINE_SCHEMA, EngineFile, GridFile, PACK_SCHEMA, PackFile,
};
use content::{AeroDef, Category, EngineDef, EngineKind, EnvelopeConfig, Era, Metric, Requirement};

/// Wing loading outside this range (kg/m² at reference weight) is a data error (GDD 62.6).
const WING_LOADING_KG_M2: (f64, f64) = (100.0, 900.0);
/// Fraction of internal fuel in the reference weight (GDD 13.4).
pub const REFERENCE_FUEL_FRACTION: f64 = 0.6;
/// The angle-of-attack range every two-dimensional aero table must cover, degrees (GDD 11.6.2).
const ALPHA_COVERAGE_DEG: (f64, f64) = (-30.0, 90.0);
/// The Mach range every two-dimensional aero table must cover (GDD 11.6.2).
const MACH_COVERAGE: (f64, f64) = (0.0, 3.0);
/// A row's lift peak may exceed the trimmed `C_L,max` by this factor at most: the table is the
/// untrimmed lift, raised to make up for the elevator's trim loss, which grows with the static
/// margin to about 15 % supersonic (ADR-30).
const CL_PEAK_RATIO: (f64, f64) = (1.0, 1.2);
/// The post-stall blend must start at least this far past the lift peak, degrees (ADR-30).
const BLEND_CLEARANCE_DEG: f64 = 5.0;
/// An interior table point that is a local extremum and sits further than this fraction of the
/// row's span from the line through its neighbours is a spike (GDD 11.6.2).
const SPIKE_FRACTION: f64 = 0.5;

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

/// Parses `text`, decodes it into `T`, checks numeric ranges against `schema` and `f32`
/// exactness, then runs `semantics`. Returns the decoded value or every problem found.
fn load<T: serde::de::DeserializeOwned>(
    file: &str,
    text: &str,
    schema: &Value,
    semantics: impl FnOnce(&mut Ctx<'_>, &T),
) -> Result<T, Vec<Diagnostic>> {
    let value = parse_json(file, text)?;
    let typed: T = decode(file, &value)?;
    let mut c = Ctx {
        file,
        out: Vec::new(),
        flagged: BTreeSet::new(),
    };
    check_ranges(&mut c, schema, schema, &value, "");
    check_f32_exact(&mut c, &value, "");
    semantics(&mut c, &typed);
    if c.out.is_empty() {
        Ok(typed)
    } else {
        Err(c.out)
    }
}

/// Loads and validates one aircraft file. `locale` is the default locale table. References to
/// engines and aero tables are checked for shape here and resolved by [`crate::bake`].
///
/// # Errors
/// Every problem found, each with its pointer.
pub fn load_aircraft(
    file: &str,
    text: &str,
    pack_id: &str,
    locale: &BTreeMap<String, String>,
) -> Result<AircraftFile, Vec<Diagnostic>> {
    load(file, text, &crate::schema::aircraft_schema(), |c, a| {
        check_aircraft(c, a, pack_id, locale);
    })
}

/// A validated engine file and its runtime form.
#[derive(Debug)]
pub struct LoadedEngine {
    /// The authoring form.
    pub file: EngineFile,
    /// The runtime form.
    pub def: EngineDef,
}

/// Loads and validates one engine file.
///
/// # Errors
/// Every problem found, each with its pointer.
pub fn load_engine(
    file: &str,
    text: &str,
    pack_id: &str,
    locale: &BTreeMap<String, String>,
) -> Result<LoadedEngine, Vec<Diagnostic>> {
    let e = load(file, text, &crate::schema::engine_schema(), |c, e| {
        check_engine(c, e, pack_id, locale);
    })?;
    let def = e.to_def().ok_or_else(|| baker_bug(file))?;
    Ok(LoadedEngine { file: e, def })
}

/// A validated aero file and its runtime form.
#[derive(Debug)]
pub struct LoadedAero {
    /// The authoring form.
    pub file: AeroFile,
    /// The runtime form.
    pub def: AeroDef,
}

/// Loads and validates one aero-table file.
///
/// # Errors
/// Every problem found, each with its pointer.
pub fn load_aero(file: &str, text: &str, pack_id: &str) -> Result<LoadedAero, Vec<Diagnostic>> {
    let a = load(file, text, &crate::schema::aero_schema(), |c, a| {
        check_aero(c, a, pack_id);
    })?;
    let def = a.to_def();
    Ok(LoadedAero { file: a, def })
}

/// The diagnostic for a name that validated but then failed to convert.
#[must_use]
pub fn baker_bug(file: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::new(
        file,
        "",
        "a name could not be resolved after validation",
        "this is a baker bug; report it",
    )]
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
// Aircraft.

fn check_aircraft(
    c: &mut Ctx<'_>,
    a: &AircraftFile,
    pack_id: &str,
    locale: &BTreeMap<String, String>,
) {
    check_identity(c, a, pack_id, locale);
    check_mass_and_geometry(c, a);
    check_tanks(c, a);
    check_inertia(c, a);
    check_references_and_fcs(c, a, pack_id);
    check_envelope(c, a);
}

/// Mass at which wing loading and thrust-to-weight are judged: empty plus 60 % of the fuel.
#[must_use]
pub fn reference_kg(a: &AircraftFile) -> f64 {
    a.mass.empty_kg + REFERENCE_FUEL_FRACTION * a.mass.internal_fuel_kg
}

/// True if the masses the derived checks rely on have no error of their own.
fn masses_ok(c: &Ctx<'_>) -> bool {
    c.clean(&["/mass/empty_kg", "/mass/internal_fuel_kg"])
}

fn check_names<T>(
    c: &mut Ctx<'_>,
    ptr: &str,
    name: &str,
    what: &str,
    all: &[T],
    lookup: fn(&str) -> Option<T>,
    name_of: fn(&T) -> &'static str,
) {
    if lookup(name).is_none() {
        let names: Vec<&str> = all.iter().map(name_of).collect();
        c.err(
            ptr,
            format!("`{name}` is not a known {what}"),
            format!("use one of: {}", names.join(", ")),
        );
    }
}

fn check_locale_key(c: &mut Ctx<'_>, key: &str, locale: &BTreeMap<String, String>) {
    if !locale.contains_key(key) {
        c.err(
            "/name_key",
            format!("locale key `{key}` is not in the default locale"),
            format!("add \"{key}\" to locales/en.json, or use an existing key"),
        );
    }
}

/// Tags, ids, names and the locale key.
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
    check_names(
        c,
        "/category",
        &a.category,
        "category",
        Category::ALL,
        Category::from_name,
        |x| x.name(),
    );
    check_names(c, "/era", &a.era, "era", Era::ALL, Era::from_name, |x| {
        x.name()
    });
    check_locale_key(c, &a.name_key, locale);
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
    if c.clean(&[
        "/mass/empty_kg",
        "/mass/max_takeoff_kg",
        "/mass/max_landing_kg",
    ]) && !(m.empty_kg..=m.max_takeoff_kg).contains(&m.max_landing_kg)
    {
        c.err(
            "/mass/max_landing_kg",
            format!(
                "maximum landing mass {} kg is not between the empty mass {} kg and the maximum take-off mass {} kg",
                m.max_landing_kg, m.empty_kg, m.max_takeoff_kg
            ),
            "check max_landing_kg",
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

/// Tank names are unique and the capacities add up to the internal fuel.
fn check_tanks(c: &mut Ctx<'_>, a: &AircraftFile) {
    let tanks = &a.mass.tanks;
    let mut seen = BTreeSet::new();
    for (i, t) in tanks.iter().enumerate() {
        if !seen.insert(t.id.as_str()) {
            c.err(
                &format!("/mass/tanks/{i}/id"),
                format!("tank `{}` is listed twice", t.id),
                "give each tank its own id",
            );
        }
    }
    let sum: f64 = tanks.iter().map(|t| t.capacity_kg).sum();
    if c.clean(&["/mass/internal_fuel_kg"])
        && (sum - a.mass.internal_fuel_kg).abs() > 1e-3 * a.mass.internal_fuel_kg
    {
        c.err(
            "/mass/tanks",
            format!(
                "the tanks hold {sum} kg but internal_fuel_kg is {} kg",
                a.mass.internal_fuel_kg
            ),
            "make the tank capacities add up to internal_fuel_kg",
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

/// The shape of the references (they are resolved by the bake), actuator travel and SAS.
fn check_references_and_fcs(c: &mut Ctx<'_>, a: &AircraftFile, pack_id: &str) {
    for (i, e) in a.engines.iter().enumerate() {
        if let Err(hint) = check_id(&e.engine_ref, pack_id, "eng") {
            c.err(
                &format!("/engines/{i}/ref"),
                format!("`{}` is not a valid engine id", e.engine_ref),
                hint,
            );
        }
    }
    if let Err(hint) = check_id(&a.aero.tables_ref, pack_id, "aero") {
        c.err(
            "/aero/tables_ref",
            format!("`{}` is not a valid aero-table id", a.aero.tables_ref),
            hint,
        );
    }
    let acts = &a.fcs.actuators;
    for (name, act) in [
        ("stab", &acts.stab),
        ("aileron", &acts.aileron),
        ("rudder", &acts.rudder),
        ("lef", &acts.lef),
        ("tef", &acts.tef),
    ] {
        if act.range_deg[0] >= act.range_deg[1] {
            c.err(
                &format!("/fcs/actuators/{name}/range_deg"),
                format!(
                    "travel [{}, {}] is empty",
                    act.range_deg[0], act.range_deg[1]
                ),
                "write the minimum first, then a larger maximum",
            );
        }
    }
    if a.aero.static_margin_mac < 0.0 && !a.fcs.sas_required {
        c.err(
            "/fcs/sas_required",
            "the static margin is negative, so the airframe is unstable without augmentation",
            "set sas_required to true, or correct static_margin_mac",
        );
    }
}

/// Envelope points: unique ids, known names, masses the aircraft can have.
fn check_envelope(c: &mut Ctx<'_>, a: &AircraftFile) {
    let mut seen = BTreeSet::new();
    for (i, p) in a.envelope.points.iter().enumerate() {
        let at = format!("/envelope/points/{i}");
        if !seen.insert(p.id.as_str()) {
            c.err(
                &format!("{at}/id"),
                format!("envelope point `{}` is listed twice", p.id),
                "give each point its own id",
            );
        }
        check_names(
            c,
            &format!("{at}/config"),
            &p.config,
            "configuration",
            EnvelopeConfig::ALL,
            EnvelopeConfig::from_name,
            |x| x.name(),
        );
        check_names(
            c,
            &format!("{at}/metric"),
            &p.metric,
            "metric",
            Metric::ALL,
            Metric::from_name,
            |x| x.name(),
        );
        if let Some(r) = &p.requires {
            check_names(
                c,
                &format!("{at}/requires"),
                r,
                "requirement",
                Requirement::ALL,
                Requirement::from_name,
                |x| x.name(),
            );
        }
        let ptr = format!("{at}/mass_kg");
        if c.clean(&["/mass/empty_kg", "/mass/max_takeoff_kg", ptr.as_str()])
            && !(a.mass.empty_kg..=a.mass.max_takeoff_kg).contains(&p.mass_kg)
        {
            c.err(
                &ptr,
                format!(
                    "{} kg is outside the aircraft's mass range {}-{} kg",
                    p.mass_kg, a.mass.empty_kg, a.mass.max_takeoff_kg
                ),
                "use a mass between the empty mass and the maximum take-off mass",
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Tables.

/// A table field of a file, with its pointer.
enum TableField<'a> {
    Curve(&'a CurveFile),
    Grid(&'a GridFile),
}

/// Linear interpolation of a curve's points, clamped at the ends.
#[must_use]
pub fn eval_curve(points: &[[f64; 2]], x: f64) -> f64 {
    let n = points.len();
    if n == 0 {
        return f64::NAN;
    }
    if x <= points[0][0] {
        return points[0][1];
    }
    if x >= points[n - 1][0] {
        return points[n - 1][1];
    }
    let i = points.partition_point(|p| p[0] <= x) - 1;
    let (a, b) = (points[i], points[i + 1]);
    a[1] + (b[1] - a[1]) * (x - a[0]) / (b[0] - a[0])
}

fn increasing(xs: impl Iterator<Item = f64>) -> Option<usize> {
    let mut prev = f64::NEG_INFINITY;
    for (i, x) in xs.enumerate() {
        if x <= prev {
            return Some(i);
        }
        prev = x;
    }
    None
}

/// The index of an interior point that sticks out of its row (GDD 11.6.2): a strict local
/// extremum further than [`SPIKE_FRACTION`] of the row's span from the line through its
/// neighbours. Second differences relative to the span, rather than neighbour ratios, so that
/// zero crossings are not flagged.
fn spike(xs: &[f64], ys: &[f64]) -> Option<usize> {
    let (lo, hi) = ys
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), &v| {
            (l.min(v), h.max(v))
        });
    let span = hi - lo;
    if span.is_nan() || span <= 0.0 {
        return None;
    }
    (1..ys.len().saturating_sub(1)).find(|&i| {
        let line =
            ys[i - 1] + (ys[i + 1] - ys[i - 1]) * (xs[i] - xs[i - 1]) / (xs[i + 1] - xs[i - 1]);
        let extremum =
            (ys[i] > ys[i - 1] && ys[i] > ys[i + 1]) || (ys[i] < ys[i - 1] && ys[i] < ys[i + 1]);
        extremum && (ys[i] - line).abs() > SPIKE_FRACTION * span
    })
}

/// Axis names, shape, ordering and spikes of one table. Returns true if the table is sound
/// enough for the checks that read its values.
fn check_table(c: &mut Ctx<'_>, ptr: &str, field: &TableField<'_>, axes: &[&str]) -> bool {
    match field {
        TableField::Curve(t) => {
            if axes != [t.axis.as_str()] {
                c.err(
                    &format!("{ptr}/axis"),
                    format!(
                        "this table is against `{}`, not `{}`",
                        axes.join(", "),
                        t.axis
                    ),
                    format!("set axis to `{}`", axes.join(", ")),
                );
                return false;
            }
            if let Some(i) = increasing(t.points.iter().map(|p| p[0])) {
                c.err(
                    &format!("{ptr}/points/{i}"),
                    "the x values are not strictly increasing",
                    "sort the points by x and remove duplicates",
                );
                return false;
            }
            let xs: Vec<f64> = t.points.iter().map(|p| p[0]).collect();
            let ys: Vec<f64> = t.points.iter().map(|p| p[1]).collect();
            if let Some(i) = spike(&xs, &ys) {
                c.err(
                    &format!("{ptr}/points/{i}"),
                    format!("{} sticks out of the curve (a spike)", ys[i]),
                    "check the value for a typo",
                );
                return false;
            }
            true
        }
        TableField::Grid(t) => {
            let names = [t.axes[0].0.as_str(), t.axes[1].0.as_str()];
            if axes != names {
                c.err(
                    &format!("{ptr}/axes"),
                    format!(
                        "this table is against `{}`, not `{}`",
                        axes.join(", "),
                        names.join(", ")
                    ),
                    format!("set axes to [\"{}\"]", axes.join("\", \"")),
                );
                return false;
            }
            for (axis, values) in [("x", &t.x), ("y", &t.y)] {
                if let Some(i) = increasing(values.iter().copied()) {
                    c.err(
                        &format!("{ptr}/{axis}/{i}"),
                        format!("the {axis} grid is not strictly increasing"),
                        "sort the grid and remove duplicates",
                    );
                    return false;
                }
            }
            if t.z.len() != t.y.len() {
                c.err(
                    &format!("{ptr}/z"),
                    format!("{} rows for {} y values", t.z.len(), t.y.len()),
                    "write one row per y value",
                );
                return false;
            }
            for (j, row) in t.z.iter().enumerate() {
                if row.len() != t.x.len() {
                    c.err(
                        &format!("{ptr}/z/{j}"),
                        format!("{} values for {} x values", row.len(), t.x.len()),
                        "write one value per x value in every row",
                    );
                    return false;
                }
                if let Some(i) = spike(&t.x, row) {
                    c.err(
                        &format!("{ptr}/z/{j}/{i}"),
                        format!("{} sticks out of its row (a spike)", row[i]),
                        "check the value for a typo",
                    );
                    return false;
                }
            }
            true
        }
    }
}

/// Every point of a curve satisfies `ok`.
fn curve_values(c: &mut Ctx<'_>, ptr: &str, t: &CurveFile, ok: fn(f64) -> bool, need: &str) {
    if let Some(i) = t.points.iter().position(|p| !ok(p[1])) {
        c.err(
            &format!("{ptr}/points/{i}"),
            format!("{} is not {need}", t.points[i][1]),
            format!("every value of this table must be {need}"),
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Engines.

fn check_engine(c: &mut Ctx<'_>, e: &EngineFile, pack_id: &str, locale: &BTreeMap<String, String>) {
    if e.schema != ENGINE_SCHEMA {
        c.err(
            "/schema",
            format!("`{}` is not an engine file", e.schema),
            format!("set it to `{ENGINE_SCHEMA}`"),
        );
    }
    if let Err(hint) = check_id(&e.id, pack_id, "eng") {
        c.err("/id", format!("`{}` is not a valid engine id", e.id), hint);
    }
    check_names(
        c,
        "/kind",
        &e.kind,
        "engine kind",
        EngineKind::ALL,
        EngineKind::from_name,
        |x| x.name(),
    );
    check_locale_key(c, &e.name_key, locale);

    for (ptr, axes) in ENGINE_AXES {
        let t = if ptr.ends_with("dry") {
            &e.thrust.mach_factor_dry
        } else {
            &e.thrust.mach_factor_ab
        };
        if check_table(c, ptr, &TableField::Curve(t), axes) {
            curve_values(c, ptr, t, |v| v > 0.0, "positive");
        }
    }

    let t = &e.thrust;
    if c.clean(&["/thrust/sl_dry_n", "/thrust/sl_ab_n"]) {
        if t.sl_ab_n < t.sl_dry_n {
            c.err(
                "/thrust/sl_ab_n",
                "afterburner thrust is below military thrust",
                "set it equal to sl_dry_n for an engine with no afterburner",
            );
        } else if (e.dynamics.ab_stages == 0) != (t.sl_ab_n <= t.sl_dry_n) {
            // Afterburner thrust is not below dry here, so `<=` means "equal".
            c.err(
                "/dynamics/ab_stages",
                format!(
                    "{} afterburner stages, but afterburner thrust {} the dry thrust",
                    e.dynamics.ab_stages,
                    if t.sl_ab_n <= t.sl_dry_n {
                        "equals"
                    } else {
                        "exceeds"
                    }
                ),
                "use 0 stages exactly when there is no afterburner (sl_ab_n equal to sl_dry_n)",
            );
        }
    }
    if c.clean(&["/tsfc/dry_kg_per_ns", "/tsfc/ab_kg_per_ns"])
        && e.tsfc.ab_kg_per_ns < e.tsfc.dry_kg_per_ns
    {
        c.err(
            "/tsfc/ab_kg_per_ns",
            "afterburner TSFC is below dry TSFC",
            "an afterburner burns more fuel per newton; check both values",
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Aero tables.

fn aero_tables(a: &AeroFile) -> Vec<(&'static str, &'static [&'static str], TableField<'_>)> {
    AERO_AXES
        .iter()
        .map(|&(ptr, axes)| {
            let field = match ptr {
                "/lift/cl_base" => TableField::Grid(&a.lift.cl_base),
                "/lift/cl_max" => TableField::Curve(&a.lift.cl_max),
                "/lift/cl_alpha_per_rad" => TableField::Curve(&a.lift.cl_alpha_per_rad),
                "/lift/cl_de_per_rad" => TableField::Curve(&a.lift.cl_de_per_rad),
                "/drag/cd0" => TableField::Curve(&a.drag.cd0),
                "/drag/cd_wave" => TableField::Curve(&a.drag.cd_wave),
                "/drag/k_induced" => TableField::Curve(&a.drag.k_induced),
                "/pitch/cm_0" => TableField::Grid(&a.pitch.cm_0),
                "/pitch/cm_de_per_rad" => TableField::Curve(&a.pitch.cm_de_per_rad),
                "/post_stall/blend_start_deg" => TableField::Curve(&a.post_stall.blend_start_deg),
                "/post_stall/blend_end_deg" => TableField::Curve(&a.post_stall.blend_end_deg),
                "/control_effectiveness" => TableField::Curve(&a.control_effectiveness),
                other => unreachable!("AERO_AXES lists `{other}` but aero_tables does not"),
            };
            (ptr, axes, field)
        })
        .collect()
}

fn check_aero(c: &mut Ctx<'_>, a: &AeroFile, pack_id: &str) {
    if a.schema != AERO_SCHEMA {
        c.err(
            "/schema",
            format!("`{}` is not an aero-table file", a.schema),
            format!("set it to `{AERO_SCHEMA}`"),
        );
    }
    if let Err(hint) = check_id(&a.id, pack_id, "aero") {
        c.err(
            "/id",
            format!("`{}` is not a valid aero-table id", a.id),
            hint,
        );
    }
    let mut sound = true;
    for (ptr, axes, field) in aero_tables(a) {
        sound &= check_table(c, ptr, &field, axes);
    }
    if !sound {
        return;
    }

    for (ptr, t) in [
        ("/lift/cl_base", &a.lift.cl_base),
        ("/pitch/cm_0", &a.pitch.cm_0),
    ] {
        let (x0, x1) = (t.x[0], t.x[t.x.len() - 1]);
        if x0 > ALPHA_COVERAGE_DEG.0 || x1 < ALPHA_COVERAGE_DEG.1 {
            c.err(
                &format!("{ptr}/x"),
                format!(
                    "alpha covers {x0} to {x1} deg, not {} to {}",
                    ALPHA_COVERAGE_DEG.0, ALPHA_COVERAGE_DEG.1
                ),
                "extend the alpha grid (GDD 11.6.2)",
            );
        }
        let (y0, y1) = (t.y[0], t.y[t.y.len() - 1]);
        if y0 > MACH_COVERAGE.0 || y1 < MACH_COVERAGE.1 {
            c.err(
                &format!("{ptr}/y"),
                format!(
                    "Mach covers {y0} to {y1}, not {} to {}",
                    MACH_COVERAGE.0, MACH_COVERAGE.1
                ),
                "extend the Mach grid (GDD 11.6.2)",
            );
        }
    }

    curve_values(c, "/drag/cd0", &a.drag.cd0, |v| v > 0.0, "positive");
    curve_values(
        c,
        "/drag/cd_wave",
        &a.drag.cd_wave,
        |v| v >= 0.0,
        "zero or positive",
    );
    curve_values(
        c,
        "/drag/k_induced",
        &a.drag.k_induced,
        |v| v > 0.0,
        "positive",
    );
    curve_values(c, "/lift/cl_max", &a.lift.cl_max, |v| v > 0.0, "positive");
    curve_values(
        c,
        "/lift/cl_alpha_per_rad",
        &a.lift.cl_alpha_per_rad,
        |v| v > 0.0,
        "positive",
    );
    curve_values(
        c,
        "/control_effectiveness",
        &a.control_effectiveness,
        |v| v > 0.0 && v <= 1.0,
        "in (0, 1]",
    );

    check_lift_peaks_and_blend(c, a);
}

/// Each Mach row of `C_L,base` peaks at the trimmed `C_L,max` raised by at most the trim loss,
/// and the post-stall blend starts clear of the peak (ADR-30).
fn check_lift_peaks_and_blend(c: &mut Ctx<'_>, a: &AeroFile) {
    let t = &a.lift.cl_base;
    for (j, (row, &mach)) in t.z.iter().zip(&t.y).enumerate() {
        let (i_peak, peak) =
            row.iter()
                .copied()
                .enumerate()
                .fold((0, f64::NEG_INFINITY), |best, (i, v)| {
                    if v > best.1 { (i, v) } else { best }
                });
        let cl_max = eval_curve(&a.lift.cl_max.points, mach);
        let ratio = peak / cl_max;
        if !(CL_PEAK_RATIO.0..=CL_PEAK_RATIO.1).contains(&ratio) {
            c.err(
                &format!("/lift/cl_base/z/{j}/{i_peak}"),
                format!(
                    "at Mach {mach} the lift peaks at {peak}, {ratio:.3} times the trimmed C_L,max of {cl_max}"
                ),
                format!(
                    "the untrimmed peak may exceed cl_max by the elevator's trim loss only ({}-{} times)",
                    CL_PEAK_RATIO.0, CL_PEAK_RATIO.1
                ),
            );
        }
        let alpha_peak = t.x[i_peak];
        let start = eval_curve(&a.post_stall.blend_start_deg.points, mach);
        let end = eval_curve(&a.post_stall.blend_end_deg.points, mach);
        if start < alpha_peak + BLEND_CLEARANCE_DEG {
            c.err(
                "/post_stall/blend_start_deg",
                format!(
                    "at Mach {mach} the blend starts at {start} deg, within {BLEND_CLEARANCE_DEG} deg of the lift peak at {alpha_peak} deg"
                ),
                "start the flat-plate blend past the stall; the stall itself belongs in cl_base (ADR-30)",
            );
            return;
        }
        if end <= start {
            c.err(
                "/post_stall/blend_end_deg",
                format!(
                    "at Mach {mach} the blend ends at {end} deg, not after its start at {start} deg"
                ),
                "make blend_end_deg larger than blend_start_deg",
            );
            return;
        }
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
        return Err(format!(
            "this kind of entity uses the type `{kind}` in its id"
        ));
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
