//! The bake pipeline: load, validate, resolve, pack (GDD 62.2).

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::check::{Loaded, load_aircraft, load_locale, load_pack};
use crate::diag::{Diagnostic, pointer};
use crate::model::{AircraftFile, PackFile};
use content::AircraftDef;

/// The text of every input file of one pack, with the names to report errors against.
pub struct Sources {
    /// `(name, text)` of `pack.json`.
    pub pack: (String, String),
    /// `(name, text)` of `locales/en.json`.
    pub locale: (String, String),
    /// `(name, text)` of each `aircraft/*.json`.
    pub aircraft: Vec<(String, String)>,
}

/// A successful bake.
#[derive(Debug)]
pub struct Baked {
    /// The pack manifest.
    pub pack: PackFile,
    /// The `.cpack` bytes.
    pub cpack: Vec<u8>,
    /// What went in, sorted by id (the order of the pack's handles).
    pub aircraft: Vec<AircraftFile>,
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

    let mut by_id: BTreeMap<String, (String, Loaded)> = BTreeMap::new();
    for (name, text) in &src.aircraft {
        let Some(a) = keep(load_aircraft(name, text, &pack.id, &locale), &mut problems) else {
            continue;
        };
        if let Some((first, _)) = by_id.get(&a.file.id) {
            problems.push(Diagnostic::new(
                name,
                "/id",
                format!("`{}` is already defined in {first}", a.file.id),
                "ids are unique within a pack; rename one of them",
            ));
            continue;
        }
        by_id.insert(a.file.id.clone(), (name.clone(), a));
    }

    // Unused keys are only reliable once every aircraft file has loaded.
    if problems.is_empty() {
        let used: Vec<&str> = by_id
            .values()
            .map(|(_, a)| a.file.name_key.as_str())
            .collect();
        for key in locale.keys().filter(|k| !used.contains(&k.as_str())) {
            problems.push(Diagnostic::new(
                &src.locale.0,
                pointer(&[key]),
                format!("locale key `{key}` is not used by any entity"),
                "remove it, or reference it from an entity's name_key",
            ));
        }
    }
    if !problems.is_empty() {
        return Err(problems);
    }

    let (files, entries): (Vec<AircraftFile>, Vec<(String, AircraftDef)>) = by_id
        .into_values()
        .map(|(_, a)| {
            let id = a.file.id.clone();
            (a.file, (id, a.def))
        })
        .unzip();
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
        aircraft: files,
    })
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

/// Reads a pack directory (`pack.json`, `locales/en.json`, `aircraft/*.json`) and bakes it.
///
/// # Errors
/// Read failures and every diagnostic from [`bake_sources`].
pub fn bake_dir(dir: &Path) -> Result<Baked, Vec<Diagnostic>> {
    let read = |rel: &str| -> Result<(String, String), Vec<Diagnostic>> {
        let text = fs::read_to_string(dir.join(rel)).map_err(|e| {
            vec![Diagnostic::new(
                rel,
                "",
                format!("cannot read: {e}"),
                format!("check that {} exists", dir.join(rel).display()),
            )]
        })?;
        Ok((rel.to_owned(), text))
    };
    let pack = read("pack.json")?;
    let locale = read("locales/en.json")?;

    let aircraft_dir = dir.join("aircraft");
    let mut names: Vec<String> = fs::read_dir(&aircraft_dir)
        .map_err(|e| {
            vec![Diagnostic::new(
                "aircraft",
                "",
                format!("cannot read the directory: {e}"),
                format!("create {}", aircraft_dir.display()),
            )]
        })?
        .filter_map(Result::ok)
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| Path::new(n).extension().is_some_and(|x| x == "json"))
        .collect();
    names.sort();
    let aircraft = names
        .iter()
        .map(|n| read(&format!("aircraft/{n}")))
        .collect::<Result<Vec<_>, _>>()?;
    bake_sources(&Sources {
        pack,
        locale,
        aircraft,
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
            "entities": { "aircraft": baked.aircraft.len() },
        }],
    })
}
