//! The bake pipeline: load, validate, resolve, pack (GDD 62.2).

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::check::{
    LoadedAero, LoadedEngine, baker_bug, load_aero, load_aircraft, load_engine, load_locale,
    load_pack, reference_kg,
};
use crate::diag::{Diagnostic, pointer};
use crate::model::{AeroFile, AircraftFile, EngineFile, PackFile};
use content::{ContentId, Entry};

/// Standard gravity, m/s² (thrust-to-weight).
const G: f64 = 9.806_65;
/// Thrust-to-weight outside this range is a data error (GDD 62.6).
const THRUST_TO_WEIGHT: (f64, f64) = (0.3, 2.5);

/// The text of every input file of one pack, with the names to report errors against.
pub struct Sources {
    /// `(name, text)` of `pack.json`.
    pub pack: (String, String),
    /// `(name, text)` of `locales/en.json`.
    pub locale: (String, String),
    /// `(name, text)` of each `aircraft/*.json`.
    pub aircraft: Vec<(String, String)>,
    /// `(name, text)` of each `engines/*.json`.
    pub engines: Vec<(String, String)>,
    /// `(name, text)` of each `aero/*.json`.
    pub aero: Vec<(String, String)>,
}

/// A successful bake.
#[derive(Debug)]
pub struct Baked {
    /// The pack manifest.
    pub pack: PackFile,
    /// The `.cpack` bytes.
    pub cpack: Vec<u8>,
    /// The aircraft that went in, sorted by id.
    pub aircraft: Vec<AircraftFile>,
    /// The engines, sorted by id.
    pub engines: Vec<EngineFile>,
    /// The aero tables, sorted by id.
    pub aero: Vec<AeroFile>,
}

/// One loaded entity, before ids are turned into handles.
#[allow(
    clippy::large_enum_variant,
    reason = "a handful per pack, built once; boxing buys nothing"
)]
enum Loaded {
    Aircraft(AircraftFile),
    Engine(LoadedEngine),
    Aero(LoadedAero),
}

/// Validates and packs `src`. Collects every diagnostic it can before giving up.
///
/// # Errors
/// All diagnostics found, in file order.
pub fn bake_sources(src: &Sources) -> Result<Baked, Vec<Diagnostic>> {
    let mut problems = Vec::new();
    let pack = keep(load_pack(&src.pack.0, &src.pack.1), &mut problems);
    let locale = keep(load_locale(&src.locale.0, &src.locale.1), &mut problems);
    let (Some(pack), Some(locale)) = (pack, locale) else {
        return Err(problems);
    };

    // Every entity, by id; the order of this map is the order of the pack's handles.
    let mut by_id: BTreeMap<String, (String, Loaded)> = BTreeMap::new();
    let mut add = |name: &str, id: String, item: Loaded, problems: &mut Vec<Diagnostic>| {
        if let Some((first, _)) = by_id.get(&id) {
            problems.push(Diagnostic::new(
                name,
                "/id",
                format!("`{id}` is already defined in {first}"),
                "ids are unique within a pack; rename one of them",
            ));
        } else {
            by_id.insert(id, (name.to_owned(), item));
        }
    };
    for (name, text) in &src.aircraft {
        if let Some(a) = keep(load_aircraft(name, text, &pack.id, &locale), &mut problems) {
            add(name, a.id.clone(), Loaded::Aircraft(a), &mut problems);
        }
    }
    for (name, text) in &src.engines {
        if let Some(e) = keep(load_engine(name, text, &pack.id, &locale), &mut problems) {
            add(name, e.file.id.clone(), Loaded::Engine(e), &mut problems);
        }
    }
    for (name, text) in &src.aero {
        if let Some(a) = keep(load_aero(name, text, &pack.id), &mut problems) {
            add(name, a.file.id.clone(), Loaded::Aero(a), &mut problems);
        }
    }
    if !problems.is_empty() {
        return Err(problems);
    }

    check_cross_references(&by_id, &mut problems);
    check_unused_locale_keys(&by_id, &locale, &src.locale.0, &mut problems);
    if !problems.is_empty() {
        return Err(problems);
    }

    let handles: BTreeMap<&str, ContentId> = by_id
        .keys()
        .enumerate()
        .map(|(i, id)| (id.as_str(), ContentId(i as u32)))
        .collect();
    let resolve = |id: &str| handles.get(id).copied();
    let mut entries = Vec::with_capacity(by_id.len());
    let mut baked_files = (Vec::new(), Vec::new(), Vec::new());
    for (id, (name, item)) in &by_id {
        let entry = match item {
            Loaded::Aircraft(a) => {
                baked_files.0.push(a.clone());
                Entry::Aircraft(a.to_def(&resolve).ok_or_else(|| baker_bug(name))?)
            }
            Loaded::Engine(e) => {
                baked_files.1.push(e.file.clone());
                Entry::Engine(e.def.clone())
            }
            Loaded::Aero(a) => {
                baked_files.2.push(a.file.clone());
                Entry::Aero(a.def.clone())
            }
        };
        entries.push((id.clone(), entry));
    }
    let cpack = content::cpack::write(&entries, sha256).map_err(|e| {
        vec![Diagnostic::new(
            &src.pack.0,
            "",
            format!("cannot pack: {e}"),
            "this is a baker bug or a size limit; report it",
        )]
    })?;
    Ok(Baked {
        pack,
        cpack,
        aircraft: baked_files.0,
        engines: baked_files.1,
        aero: baked_files.2,
    })
}

/// References resolve to entities of the right kind, and the checks that need two files.
fn check_cross_references(
    by_id: &BTreeMap<String, (String, Loaded)>,
    problems: &mut Vec<Diagnostic>,
) {
    let engine = |id: &str| match by_id.get(id) {
        Some((_, Loaded::Engine(e))) => Some(e),
        _ => None,
    };
    let aero = |id: &str| match by_id.get(id) {
        Some((_, Loaded::Aero(a))) => Some(a),
        _ => None,
    };
    for (name, item) in by_id.values() {
        let Loaded::Aircraft(a) = item else { continue };
        let mut thrust_ok = true;
        let mut ab_thrust_n = 0.0;
        for (i, e) in a.engines.iter().enumerate() {
            if let Some(found) = engine(&e.engine_ref) {
                ab_thrust_n += f64::from(e.count) * found.file.thrust.sl_ab_n;
            } else {
                thrust_ok = false;
                problems.push(Diagnostic::new(
                    name,
                    format!("/engines/{i}/ref"),
                    format!("there is no engine `{}` in the pack", e.engine_ref),
                    "add the engine file under engines/, or fix the id",
                ));
            }
        }
        if thrust_ok {
            let t_w = ab_thrust_n / (reference_kg(a) * G);
            if !(THRUST_TO_WEIGHT.0..=THRUST_TO_WEIGHT.1).contains(&t_w) {
                problems.push(Diagnostic::new(
                    name,
                    "/engines",
                    format!(
                        "thrust-to-weight is {t_w:.2} at reference weight, outside {}-{}",
                        THRUST_TO_WEIGHT.0, THRUST_TO_WEIGHT.1
                    ),
                    "check the engine count, the engines' thrust and the masses",
                ));
            }
        }
        match aero(&a.aero.tables_ref) {
            None => problems.push(Diagnostic::new(
                name,
                "/aero/tables_ref",
                format!(
                    "there are no aero tables `{}` in the pack",
                    a.aero.tables_ref
                ),
                "add the aero file under aero/, or fix the id",
            )),
            Some(t) if !a.fcs.sas_required => {
                if let Some(mach) = unstable_mach(&t.file) {
                    problems.push(Diagnostic::new(
                        name,
                        "/fcs/sas_required",
                        format!(
                            "the pitching moment of `{}` rises with angle of attack at Mach {mach}, so the airframe is unstable without augmentation",
                            a.aero.tables_ref
                        ),
                        "set sas_required to true, or correct cm_0",
                    ));
                }
            }
            Some(_) => {}
        }
    }
}

/// The first Mach row of `C_m0` whose slope between 0 and the next angle of attack is not
/// negative (statically unstable), if any.
fn unstable_mach(a: &AeroFile) -> Option<f64> {
    let t = &a.pitch.cm_0;
    let i0 = t.x.iter().position(|&x| x >= 0.0)?;
    if i0 + 1 >= t.x.len() {
        return None;
    }
    t.y.iter()
        .zip(&t.z)
        .find(|(_, row)| row[i0 + 1] >= row[i0])
        .map(|(m, _)| *m)
}

/// Every key in the default locale must be used by some entity.
fn check_unused_locale_keys(
    by_id: &BTreeMap<String, (String, Loaded)>,
    locale: &BTreeMap<String, String>,
    locale_file: &str,
    problems: &mut Vec<Diagnostic>,
) {
    let used: Vec<&str> = by_id
        .values()
        .filter_map(|(_, item)| match item {
            Loaded::Aircraft(a) => Some(a.name_key.as_str()),
            Loaded::Engine(e) => Some(e.file.name_key.as_str()),
            Loaded::Aero(_) => None,
        })
        .collect();
    for key in locale.keys().filter(|k| !used.contains(&k.as_str())) {
        problems.push(Diagnostic::new(
            locale_file,
            pointer(&[key]),
            format!("locale key `{key}` is not used by any entity"),
            "remove it, or reference it from an entity's name_key",
        ));
    }
}

fn keep<T>(r: Result<T, Vec<Diagnostic>>, problems: &mut Vec<Diagnostic>) -> Option<T> {
    match r {
        Ok(v) => Some(v),
        Err(mut e) => {
            problems.append(&mut e);
            None
        }
    }
}

/// SHA-256 as a plain array.
#[must_use]
pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    let mut out = [0_u8; 32];
    out.copy_from_slice(&Sha256::digest(bytes));
    out
}

/// Lower-case hex.
#[must_use]
pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// Reads the `*.json` files of one entity directory, sorted by name. A missing directory is an
/// error only if `required`.
fn read_dir_json(
    dir: &Path,
    sub: &str,
    required: bool,
) -> Result<Vec<(String, String)>, Vec<Diagnostic>> {
    let path = dir.join(sub);
    let listing = match fs::read_dir(&path) {
        Ok(l) => l,
        Err(_) if !required && !path.exists() => return Ok(Vec::new()),
        Err(e) => {
            return Err(vec![Diagnostic::new(
                sub,
                "",
                format!("cannot read the directory: {e}"),
                format!("create {}", path.display()),
            )]);
        }
    };
    let mut names: Vec<String> = listing
        .filter_map(Result::ok)
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| Path::new(n).extension().is_some_and(|x| x == "json"))
        .collect();
    names.sort();
    names
        .iter()
        .map(|n| read_file(dir, &format!("{sub}/{n}")))
        .collect()
}

fn read_file(dir: &Path, rel: &str) -> Result<(String, String), Vec<Diagnostic>> {
    let text = fs::read_to_string(dir.join(rel)).map_err(|e| {
        vec![Diagnostic::new(
            rel,
            "",
            format!("cannot read: {e}"),
            format!("check that {} exists", dir.join(rel).display()),
        )]
    })?;
    Ok((rel.to_owned(), text))
}

/// Reads a pack directory (`pack.json`, `locales/en.json`, `aircraft/`, and optionally
/// `engines/` and `aero/`) and bakes it.
///
/// # Errors
/// Read failures and every diagnostic from [`bake_sources`].
pub fn bake_dir(dir: &Path) -> Result<Baked, Vec<Diagnostic>> {
    let pack = read_file(dir, "pack.json")?;
    let locale = read_file(dir, "locales/en.json")?;
    bake_sources(&Sources {
        pack,
        locale,
        aircraft: read_dir_json(dir, "aircraft", true)?,
        engines: read_dir_json(dir, "engines", false)?,
        aero: read_dir_json(dir, "aero", false)?,
    })
}

/// The `content-manifest.json` entry for a baked pack (GDD 62.4).
#[must_use]
pub fn manifest(baked: &Baked, cpack_file: &str) -> Value {
    let hash = &baked.cpack[content::cpack::header::CONTENT_HASH..][..32];
    json!({
        "schema": "content-manifest/1",
        "packs": [{
            "id": baked.pack.id,
            "version": baked.pack.version,
            "game": baked.pack.game,
            "file": cpack_file,
            "bytes": baked.cpack.len(),
            "content_hash": format!("sha256:{}", hex(hash)),
            "entities": {
                "aircraft": baked.aircraft.len(),
                "engines": baked.engines.len(),
                "aero": baked.aero.len(),
            },
        }],
    })
}
