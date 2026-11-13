//! `build-base` (ADR-54): the base pack, z3 to z5 for the whole Earth, by decimation from every
//! cell's four z6 tiles (`z6/5-X-Y.bin`); a cell without a file is ocean.
//!
//! The base archive's header lists the licences of the cells (from their QA parts in `qa/`) and,
//! per product, the tile list's hash without the file list (the cells' own headers name the files).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use geo::sim_tile;
use geo::tile::{CELL_LEVEL, TileId};
use geo_pack::{ArchiveWriter, DecodedTile, HeaderJson, SourceSet};
use serde_json::json;

use crate::cell::{encode_all, eps_hist, height_range, level_rows};
use crate::pyramid::{Level, decimate};
use crate::{Failure, io_err, read, sha256_hex, write};

/// The base pack's coarsest level (the renderer's roots). // DATA: ADR-56
pub const BASE_MIN_LEVEL: u8 = 3;

/// Reads every `*.bin` in `dir`: the z6 tiles by id.
///
/// # Errors
/// An unreadable directory or file, or a file that is not four records.
pub fn read_z6(dir: &Path) -> Result<BTreeMap<u64, DecodedTile>, Failure> {
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| io_err(dir, &e))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "bin"))
        .collect();
    files.sort();
    let mut out = BTreeMap::new();
    for f in files {
        let b = read(&f)?;
        if b.len() != 4 * sim_tile::SIZE {
            return Err(Failure::Check(format!(
                "{}: {} bytes, not four records",
                f.display(),
                b.len()
            )));
        }
        for rec in b.as_chunks::<{ sim_tile::SIZE }>().0 {
            if let Some(t) = DecodedTile::from_sim_record(rec) {
                out.insert(t.id.id(), t);
            }
        }
    }
    Ok(out)
}

/// The licences and per-product sources named by the cells' QA parts in `qa_dir`.
///
/// # Errors
/// An unreadable or malformed part.
pub fn cell_provenance(qa_dir: &Path) -> Result<(Vec<String>, Vec<SourceSet>), Failure> {
    let mut licences = BTreeSet::new();
    let mut products: BTreeMap<String, String> = BTreeMap::new();
    if let Ok(rd) = std::fs::read_dir(qa_dir) {
        let mut files: Vec<_> = rd
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with("5-"))
            })
            .collect();
        files.sort();
        for f in files {
            let v: serde_json::Value = serde_json::from_slice(&read(&f)?)
                .map_err(|e| Failure::Check(format!("{}: {e}", f.display())))?;
            for l in v["licences"].as_array().into_iter().flatten() {
                licences.insert(l.as_str().unwrap_or_default().to_string());
            }
            for s in v["sources"].as_array().into_iter().flatten() {
                products.insert(
                    s["product"].as_str().unwrap_or_default().into(),
                    s["tile_list_sha256"].as_str().unwrap_or_default().into(),
                );
            }
        }
    }
    let sources = products
        .into_iter()
        .map(|(product, tile_list_sha256)| SourceSet {
            product,
            tile_list_sha256,
            files: vec![],
        })
        .collect();
    Ok((licences.into_iter().collect(), sources))
}

/// The base levels (z3, z4, z5) from the z6 tiles.
///
/// # Panics
/// Never: every (z, x, y) it builds is in range.
#[must_use]
pub fn build_levels(z6: &BTreeMap<u64, DecodedTile>) -> Vec<Level> {
    let mut by_level: Vec<BTreeMap<u64, DecodedTile>> = Vec::new();
    let mut below = z6.clone();
    for z in (BASE_MIN_LEVEL..=CELL_LEVEL).rev() {
        let mut level = BTreeMap::new();
        for y in 0..(1u32 << z) {
            for x in 0..(2u32 << z) {
                let id = TileId::new(z, x, y).expect("a tile");
                let kids = id.children().map(|c| below.get(&c.id()));
                if let Some(t) = decimate(id, kids) {
                    level.insert(id.id(), t);
                }
            }
        }
        below = level.clone();
        by_level.push(level);
    }
    by_level.reverse();
    by_level
        .into_iter()
        .map(|m| m.into_values().map(Some).collect())
        .collect()
}

/// Runs `build-base`: `base.gpk` and `qa/base.json` under `out`.
///
/// # Errors
/// As [`read_z6`] and [`cell_provenance`], or an I/O failure.
pub fn run(z6_dir: &Path, datapack: &str, built: &str, out: &Path) -> Result<(), Failure> {
    let z6 = read_z6(z6_dir)?;
    let (licences, sources) = cell_provenance(&out.join("qa"))?;
    if licences.is_empty() {
        return Err(Failure::Check(format!(
            "no cell QA parts with licences in {}",
            out.join("qa").display()
        )));
    }
    let levels = build_levels(&z6);
    let encoded = encode_all(&levels, crate::cell::DEFAULT_THREADS);
    let mut rows = level_rows(BASE_MIN_LEVEL, &levels, &encoded);
    // A base level's "ocean" is every tile of the level that is not stored.
    for (k, r) in rows.iter_mut().enumerate() {
        let z = BASE_MIN_LEVEL + k as u8;
        let all = 2u64 << (2 * u32::from(z));
        r["ocean"] = json!(all - r["stored"].as_u64().unwrap_or(0));
    }
    let mut w = ArchiveWriter::new(BASE_MIN_LEVEL, CELL_LEVEL);
    for e in encoded {
        w.add_encoded(e);
    }
    let header = HeaderJson {
        datapack: datapack.into(),
        cell: None,
        min_level: BASE_MIN_LEVEL,
        max_level: CELL_LEVEL,
        licences: licences.clone(),
        sources,
    };
    let bytes = w
        .finish(&header)
        .map_err(|e| Failure::Check(format!("base: {e:?}")))?;
    write(&out.join("base.gpk"), &bytes)?;
    let (lo, hi) = height_range(&levels);
    let part = json!({
        "schema": "geo_qa_part/1",
        "datapack": datapack,
        "built": built,
        "cell": null,
        "file": "base.gpk",
        "size_bytes": bytes.len(),
        "sha256": sha256_hex(&bytes),
        "levels": rows,
        "eps_hist": eps_hist(levels.iter().flatten().flatten()),
        "unfilled_voids": 0,
        "h_min": if lo.is_finite() { json!(lo) } else { serde_json::Value::Null },
        "h_max": if hi.is_finite() { json!(hi) } else { serde_json::Value::Null },
        "licences": licences,
        "z6_tiles": z6.len(),
    });
    let text = serde_json::to_string_pretty(&part).map_err(|e| Failure::Io(e.to_string()))? + "\n";
    write(&out.join("qa/base.json"), text.as_bytes())
}
