//! The world bundle (ADR-51 D4, ADR-54): `content/world/` → `world.json` (`world-bundle/1`).
//!
//! The lints, beyond the schemas' types and ranges:
//! - a region's id is a slug and is its file's name; every id and `menu_order` is unique;
//! - its cells are z5 cells (x < 64, y < 32), without duplicates;
//! - its battlefield lies inside its cells: the centre plus half the battlefield's size (half the
//!   larger edge of a rect, the radius of a circle, the larger half-axis of an ellipse), checked at
//!   nine points;
//! - its spawn is inside its battlefield, and `spawn_level` is 10 or 12;
//! - `name_key` and `preview_key` are in the world locale;
//! - a descriptor's id is a slug, its `terms_checked` is an ISO date, its `url` is https and its
//!   notices are not empty.
//!
//! The bundle holds the regions (by `menu_order`), the sources (by id) and the strings. Its keys are
//! sorted, so the same files always give the same bytes.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::sync::OnceLock;

use serde_json::{Value, json};
use simmath::dmath;

use crate::bake::{hex, sha256};
use crate::check::{Ctx, is_slug, load, load_locale};
use crate::diag::Diagnostic;
use crate::world_model::{
    GEO_SOURCE_SCHEMA, GeoSourceFile, REGION_SCHEMA, RegionFile, ShapeFile, WORLD_BUNDLE_SCHEMA,
};

/// The degrees of a z5 cell's edge (180° / 2⁵). // DATA: ADR-54
const CELL_DEG: f64 = 5.625;
/// Metres per degree of latitude, for the lints' local geometry. // ALGO: mean value, lints only
const M_PER_DEG_LAT: f64 = 111_132.0;
/// Metres per degree of longitude at the equator. // ALGO: lints only
const M_PER_DEG_LON: f64 = 111_320.0;

/// The baked world bundle.
#[derive(Clone, Debug)]
pub struct WorldBaked {
    /// `world.json`'s bytes.
    pub json: Vec<u8>,
    /// Their SHA-256.
    pub sha256: [u8; 32],
    /// Regions baked.
    pub regions: usize,
    /// Descriptors baked.
    pub sources: usize,
}

/// The schema of `regions/*.json`.
///
/// # Panics
/// Never in practice: the derived schema is always representable as JSON.
#[must_use]
pub fn region_schema() -> Value {
    static CACHE: OnceLock<Value> = OnceLock::new();
    CACHE
        .get_or_init(|| {
            serde_json::to_value(schemars::schema_for!(RegionFile)).expect("schema is JSON")
        })
        .clone()
}

/// The schema of `sources/*.json`.
///
/// # Panics
/// Never in practice.
#[must_use]
pub fn geo_source_schema() -> Value {
    static CACHE: OnceLock<Value> = OnceLock::new();
    CACHE
        .get_or_init(|| {
            serde_json::to_value(schemars::schema_for!(GeoSourceFile)).expect("schema is JSON")
        })
        .clone()
}

fn cell_contains(cells: &[[u32; 2]], lat: f64, lon: f64) -> bool {
    cells.iter().any(|&[x, y]| {
        let (west, north) = (
            -180.0 + CELL_DEG * f64::from(x),
            90.0 - CELL_DEG * f64::from(y),
        );
        lon >= west && lon <= west + CELL_DEG && lat <= north && lat >= north - CELL_DEG
    })
}

/// A point's local (east, north) metres from (`lat0`, `lon0`), equirectangular.
fn local_m(lat0: f64, lon0: f64, lat: f64, lon: f64) -> (f64, f64) {
    let mut dlon = lon - lon0;
    if dlon > 180.0 {
        dlon -= 360.0;
    } else if dlon < -180.0 {
        dlon += 360.0;
    }
    (
        dlon * M_PER_DEG_LON * dmath::cos(lat0.to_radians()),
        (lat - lat0) * M_PER_DEG_LAT,
    )
}

/// (centre, half-size in metres) of a shape.
fn extent(s: &ShapeFile) -> ([f64; 2], f64) {
    match s {
        ShapeFile::Rect {
            center_deg,
            width_m,
            height_m,
            rotation_deg,
        } => {
            let half = if *rotation_deg == 0.0 {
                0.5 * width_m.max(*height_m)
            } else {
                0.5 * (width_m * width_m + height_m * height_m).sqrt()
            };
            (*center_deg, half)
        }
        ShapeFile::Circle {
            center_deg,
            radius_m,
        } => (*center_deg, *radius_m),
        ShapeFile::Ellipse {
            center_deg,
            a_m,
            b_m,
            ..
        } => (*center_deg, a_m.max(*b_m)),
        ShapeFile::Capsule {
            a_deg,
            b_deg,
            radius_m,
        } => {
            let mid = [
                f64::midpoint(a_deg[0], b_deg[0]),
                f64::midpoint(a_deg[1], b_deg[1]),
            ];
            let (e, n) = local_m(mid[0], mid[1], a_deg[0], a_deg[1]);
            (mid, (e * e + n * n).sqrt() + radius_m)
        }
        ShapeFile::Polygon { vertices_deg } => {
            let n = vertices_deg.len().max(1) as f64;
            let c = [
                vertices_deg.iter().map(|v| v[0]).sum::<f64>() / n,
                vertices_deg.iter().map(|v| v[1]).sum::<f64>() / n,
            ];
            let r = vertices_deg
                .iter()
                .map(|v| {
                    let (e, nn) = local_m(c[0], c[1], v[0], v[1]);
                    (e * e + nn * nn).sqrt()
                })
                .fold(0.0, f64::max);
            (c, r)
        }
    }
}

/// Whether (`lat`, `lon`) is inside the shape (local geometry; the lints only).
fn shape_contains(s: &ShapeFile, lat: f64, lon: f64) -> bool {
    let rot = |e: f64, n: f64, deg: f64| {
        let (sn, cs) = (dmath::sin(-deg.to_radians()), dmath::cos(-deg.to_radians()));
        (e * cs + n * sn, -e * sn + n * cs)
    };
    match s {
        ShapeFile::Rect {
            center_deg,
            width_m,
            height_m,
            rotation_deg,
        } => {
            let (e, n) = local_m(center_deg[0], center_deg[1], lat, lon);
            let (x, y) = rot(e, n, *rotation_deg);
            x.abs() <= 0.5 * width_m && y.abs() <= 0.5 * height_m
        }
        ShapeFile::Circle {
            center_deg,
            radius_m,
        } => {
            let (e, n) = local_m(center_deg[0], center_deg[1], lat, lon);
            e * e + n * n <= radius_m * radius_m
        }
        ShapeFile::Ellipse {
            center_deg,
            a_m,
            b_m,
            rotation_deg,
        } => {
            let (e, n) = local_m(center_deg[0], center_deg[1], lat, lon);
            let (x, y) = rot(e, n, *rotation_deg);
            (x / a_m) * (x / a_m) + (y / b_m) * (y / b_m) <= 1.0
        }
        ShapeFile::Capsule {
            a_deg,
            b_deg,
            radius_m,
        } => {
            let (ae, an) = local_m(lat, lon, a_deg[0], a_deg[1]);
            let (be, bn) = local_m(lat, lon, b_deg[0], b_deg[1]);
            let (dx, dy) = (be - ae, bn - an);
            let len2 = dx * dx + dy * dy;
            let t = if len2 > 0.0 {
                (-(ae * dx + an * dy) / len2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let (px, py) = (ae + t * dx, an + t * dy);
            px * px + py * py <= radius_m * radius_m
        }
        ShapeFile::Polygon { vertices_deg } => {
            let mut inside = false;
            let n = vertices_deg.len();
            for i in 0..n {
                let (a, b) = (vertices_deg[i], vertices_deg[(i + 1) % n]);
                if (a[0] > lat) != (b[0] > lat)
                    && lon < a[1] + (lat - a[0]) * (b[1] - a[1]) / (b[0] - a[0])
                {
                    inside = !inside;
                }
            }
            inside
        }
    }
}

fn check_region(c: &mut Ctx<'_>, r: &RegionFile, stem: &str, locale: &BTreeMap<String, String>) {
    if r.schema != REGION_SCHEMA {
        c.err(
            "/schema",
            format!("schema `{}` is not {REGION_SCHEMA}", r.schema),
            format!("write \"{REGION_SCHEMA}\""),
        );
    }
    if !is_slug(&r.id) || r.id != stem {
        c.err(
            "/id",
            format!("id `{}` is not the file's name `{stem}` as a slug", r.id),
            format!("use \"{stem}\" (lower case, digits, underscores)"),
        );
    }
    for (ptr, key) in [("/name_key", &r.name_key), ("/preview_key", &r.preview_key)] {
        if !locale.contains_key(key) {
            c.err(
                ptr,
                format!("locale key `{key}` is not in locales/en.json"),
                format!("add \"{key}\" to content/world/locales/en.json"),
            );
        }
    }
    let mut seen = BTreeSet::new();
    for (i, &[x, y]) in r.cells.iter().enumerate() {
        if x >= 64 || y >= 32 {
            c.err(
                &format!("/cells/{i}"),
                format!("[{x}, {y}] is not a z5 cell"),
                "use 0 ≤ x < 64 and 0 ≤ y < 32",
            );
        } else if !seen.insert((x, y)) {
            c.err(
                &format!("/cells/{i}"),
                format!("[{x}, {y}] is listed twice"),
                "remove the duplicate",
            );
        }
    }
    let b = &r.battlefield;
    if !matches!(b.spawn_level, 10 | 12) {
        c.err(
            "/battlefield/spawn_level",
            format!("{} is not 10 or 12", b.spawn_level),
            "use 12 for a region, 10 elsewhere",
        );
    }
    let (centre, half) = extent(&b.shape);
    let dlat = half / M_PER_DEG_LAT;
    let dlon = half / (M_PER_DEG_LON * dmath::cos(centre[0].to_radians()));
    let corners_in = [-1.0, 0.0, 1.0].iter().all(|&i| {
        [-1.0, 0.0, 1.0]
            .iter()
            .all(|&j| cell_contains(&r.cells, centre[0] + i * dlat, centre[1] + j * dlon))
    });
    if !corners_in {
        c.err("/battlefield/shape", format!("the battlefield (centre {}, {}; {:.0} km from it) is not inside the region's cells", centre[0], centre[1], half / 1e3), "move it, shrink it, or add the cells it reaches");
    }
    if !shape_contains(&b.shape, r.spawn.lat_deg, r.spawn.lon_deg) {
        c.err(
            "/spawn",
            "the spawn is outside the battlefield",
            "put the spawn inside the battlefield's shape",
        );
    }
}

/// Whether `s` is `YYYY-MM-DD` with a plausible month and day.
fn is_iso_date(s: &str) -> bool {
    let b = s.as_bytes();
    let digits = |r: core::ops::Range<usize>| b[r].iter().all(u8::is_ascii_digit);
    if b.len() != 10
        || b[4] != b'-'
        || b[7] != b'-'
        || !digits(0..4)
        || !digits(5..7)
        || !digits(8..10)
    {
        return false;
    }
    let (m, d) = (
        u32::from(b[5] - b'0') * 10 + u32::from(b[6] - b'0'),
        u32::from(b[8] - b'0') * 10 + u32::from(b[9] - b'0'),
    );
    (1..=12).contains(&m) && (1..=31).contains(&d)
}

fn check_source(c: &mut Ctx<'_>, d: &GeoSourceFile) {
    if d.schema != GEO_SOURCE_SCHEMA {
        c.err(
            "/schema",
            format!("schema `{}` is not {GEO_SOURCE_SCHEMA}", d.schema),
            format!("write \"{GEO_SOURCE_SCHEMA}\""),
        );
    }
    if !is_slug(&d.id) {
        c.err(
            "/id",
            format!("id `{}` is not a slug", d.id),
            "use lower case, digits and underscores",
        );
    }
    if !is_iso_date(&d.terms_checked) {
        c.err(
            "/terms_checked",
            format!("`{}` is not a date YYYY-MM-DD", d.terms_checked),
            "write the day the terms were read, e.g. 2026-11-13",
        );
    }
    if !d.url.starts_with("https://") {
        c.err(
            "/url",
            "the terms' URL is not https",
            "link the licence page over https",
        );
    }
    for (i, n) in d.required_notices.iter().enumerate() {
        if n.trim().is_empty() {
            c.err(
                &format!("/required_notices/{i}"),
                "the notice is empty",
                "copy the notice word for word from the licence, or remove it",
            );
        }
    }
    for (ptr, s) in [
        ("/name", &d.name),
        ("/licence", &d.licence),
        ("/attribution", &d.attribution),
    ] {
        if s.trim().is_empty() {
            c.err(ptr, "empty", "fill it from the licence text");
        }
    }
}

/// The `*.json` files of `dir/sub`, sorted, as (relative name, text).
fn read_json_files(dir: &Path, sub: &str) -> Result<Vec<(String, String)>, Vec<Diagnostic>> {
    let path = dir.join(sub);
    let io = |rel: &str, e: &std::io::Error| {
        vec![Diagnostic::new(
            rel,
            "",
            format!("cannot read: {e}"),
            format!("check {}", dir.join(rel).display()),
        )]
    };
    let mut names: Vec<String> = fs::read_dir(&path)
        .map_err(|e| io(sub, &e))?
        .filter_map(Result::ok)
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| Path::new(n).extension().is_some_and(|x| x == "json"))
        .collect();
    names.sort();
    names
        .iter()
        .map(|n| {
            let rel = format!("{sub}/{n}");
            fs::read_to_string(dir.join(&rel))
                .map(|t| (rel.clone(), t))
                .map_err(|e| io(&rel, &e))
        })
        .collect()
}

/// Loads, checks and bakes a world directory.
///
/// # Errors
/// Every problem found, each with its file and pointer.
///
/// # Panics
/// Never in practice: the bundle is always representable as JSON.
pub fn bake_world_dir(dir: &Path) -> Result<WorldBaked, Vec<Diagnostic>> {
    let locale_text = fs::read_to_string(dir.join("locales/en.json")).map_err(|e| {
        vec![Diagnostic::new(
            "locales/en.json",
            "",
            format!("cannot read: {e}"),
            "add content/world/locales/en.json",
        )]
    })?;
    let locale = load_locale("locales/en.json", &locale_text)?;
    let mut problems = Vec::new();
    let mut regions: Vec<RegionFile> = Vec::new();
    for (file, text) in read_json_files(dir, "regions")? {
        let stem = Path::new(&file)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();
        match load::<RegionFile>(&file, &text, &region_schema(), |c, r| {
            check_region(c, r, &stem, &locale);
        }) {
            Ok(r) => regions.push(r),
            Err(mut p) => problems.append(&mut p),
        }
    }
    let mut sources: Vec<GeoSourceFile> = Vec::new();
    for (file, text) in read_json_files(dir, "sources")? {
        match load::<GeoSourceFile>(&file, &text, &geo_source_schema(), check_source) {
            Ok(d) => sources.push(d),
            Err(mut p) => problems.append(&mut p),
        }
    }
    let mut orders = BTreeSet::new();
    for r in &regions {
        if !orders.insert(r.menu_order) {
            problems.push(Diagnostic::new(
                &format!("regions/{}.json", r.id),
                "/menu_order",
                format!("menu_order {} is used twice", r.menu_order),
                "give each region its own place",
            ));
        }
    }
    let mut ids = BTreeSet::new();
    for d in &sources {
        if !ids.insert(d.id.clone()) {
            problems.push(Diagnostic::new(
                "sources",
                "/id",
                format!("descriptor id `{}` is used twice", d.id),
                "rename one",
            ));
        }
    }
    if regions.is_empty() || sources.is_empty() {
        problems.push(Diagnostic::new(
            "",
            "",
            "the world bundle needs at least one region and one source",
            "add content/world/regions/*.json and sources/*.json",
        ));
    }
    if !problems.is_empty() {
        return Err(problems);
    }
    regions.sort_by_key(|r| r.menu_order);
    sources.sort_by(|a, b| a.id.cmp(&b.id));
    let bundle = json!({ "schema": WORLD_BUNDLE_SCHEMA, "regions": regions, "sources": sources, "strings": locale });
    // `Value` keeps its keys sorted, so the same files always give the same bytes.
    let json = serde_json::to_vec(&bundle).expect("the bundle is JSON");
    Ok(WorldBaked {
        sha256: sha256(&json),
        json,
        regions: regions.len(),
        sources: sources.len(),
    })
}

/// The bundle's entry in `content-manifest.json`.
#[must_use]
pub fn manifest_entry(w: &WorldBaked, file: &str) -> Value {
    json!({ "file": file, "bytes": w.json.len(), "sha256": hex(&w.sha256), "entities": { "region": w.regions, "geo_source": w.sources } })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world_dir() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/world")
    }

    #[test]
    fn the_shipped_world_bakes() {
        let w = bake_world_dir(&world_dir()).unwrap_or_else(|p| {
            panic!(
                "{}",
                p.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        });
        assert_eq!((w.regions, w.sources), (6, 1));
        let v: Value = serde_json::from_slice(&w.json).unwrap();
        assert_eq!(v["schema"], WORLD_BUNDLE_SCHEMA);
        let ids: Vec<&str> = v["regions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["id"].as_str().unwrap())
            .collect();
        assert_eq!(
            ids,
            ["alps", "nevada", "plains", "normandy", "fjords", "tokyo"]
        );
        assert_eq!(v["sources"][0]["id"], "copernicus_dem");
        assert_eq!(v["strings"]["region.alps.name"], "Swiss Alps");
        assert_eq!(
            bake_world_dir(&world_dir()).unwrap().json,
            w.json,
            "deterministic"
        );
    }

    fn region_text(edit: impl Fn(&mut Value)) -> String {
        let mut v: Value = serde_json::from_str(
            &fs::read_to_string(world_dir().join("regions/alps.json")).unwrap(),
        )
        .unwrap();
        edit(&mut v);
        v.to_string()
    }

    fn lint(text: &str) -> Vec<String> {
        let locale = load_locale(
            "l",
            &fs::read_to_string(world_dir().join("locales/en.json")).unwrap(),
        )
        .unwrap();
        match load::<RegionFile>("regions/alps.json", text, &region_schema(), |c, r| {
            check_region(c, r, "alps", &locale);
        }) {
            Ok(_) => vec![],
            Err(p) => p
                .iter()
                .map(|d| format!("{} {}", d.pointer, d.message))
                .collect(),
        }
    }

    #[test]
    fn each_region_lint_fails_on_a_bad_file() {
        assert_eq!(lint(&region_text(|_| {})), Vec::<String>::new());
        let bad = lint(&region_text(|v| v["cells"] = json!([[64, 7]])));
        assert!(bad.iter().any(|p| p.contains("/cells/0")), "{bad:?}");
        let bad = lint(&region_text(|v| {
            v["battlefield"]["shape"]["center_deg"] = json!([45.2, 8.0]);
        }));
        assert!(
            bad.iter().any(|p| p.contains("not inside the region")),
            "{bad:?}"
        );
        let bad = lint(&region_text(|v| v["spawn"]["lat_deg"] = json!(48.0)));
        assert!(bad.iter().any(|p| p.contains("/spawn")), "{bad:?}");
        let bad = lint(&region_text(|v| {
            v["name_key"] = json!("region.nowhere.name");
        }));
        assert!(bad.iter().any(|p| p.contains("/name_key")), "{bad:?}");
        let bad = lint(&region_text(|v| v["id"] = json!("Alps")));
        assert!(bad.iter().any(|p| p.contains("/id")), "{bad:?}");
        let bad = lint(&region_text(|v| {
            v["battlefield"]["spawn_level"] = json!(11);
        }));
        assert!(bad.iter().any(|p| p.contains("spawn_level")), "{bad:?}");
        let bad = lint(&region_text(|v| v["battlefield"]["countdown_s"] = json!(1)));
        assert!(!bad.is_empty(), "the schema's range");
        let bad = lint(&region_text(|v| {
            v["spawn"]["tas_mps"] = json!(200.000_000_1);
        }));
        assert!(!bad.is_empty(), "f32 exactness");
    }

    #[test]
    fn each_source_lint_fails_on_a_bad_file() {
        let good = fs::read_to_string(world_dir().join("sources/copernicus_dem.json")).unwrap();
        let run = |edit: &dyn Fn(&mut Value)| {
            let mut v: Value = serde_json::from_str(&good).unwrap();
            edit(&mut v);
            load::<GeoSourceFile>(
                "sources/x.json",
                &v.to_string(),
                &geo_source_schema(),
                check_source,
            )
            .err()
            .map(|p| p.iter().map(|d| d.pointer.clone()).collect::<Vec<_>>())
        };
        assert_eq!(run(&|_| {}), None);
        assert_eq!(
            run(&|v| v["terms_checked"] = json!("13/11/2026")),
            Some(vec!["/terms_checked".to_string()])
        );
        assert_eq!(
            run(&|v| v["url"] = json!("http://x")),
            Some(vec!["/url".to_string()])
        );
        assert_eq!(
            run(&|v| v["required_notices"] = json!(["  "])),
            Some(vec!["/required_notices/0".to_string()])
        );
        assert!(run(&|v| v["extra"] = json!(1)).is_some(), "unknown keys");
        assert!(is_iso_date("2026-11-13") && !is_iso_date("2026-13-01"));
    }
}
