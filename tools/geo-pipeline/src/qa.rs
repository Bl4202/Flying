//! `qa`: the datapack's QA report (GDD 45.5 step 9, `AC-45.1`, `AC-45.3`, ADR-54), merged from the
//! cells' parts and the base pack's part in `qa/`.
//!
//! **Checks** (the command exits 1 on a failure):
//! - GP2: no cell has unfilled voids;
//! - GP4: at every level of every cell, stored + ocean = `4^(z − 5)`, and the raw bytes equal
//!   stored x 33 282.
//!
//! **The report** also recomputes GDD 45.3's whole-Earth table (every tile, ocean included) for
//! comparison, and lists the provenance of the source files (the fetch logs).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::{Failure, io_err, read, write};

/// The raw size of one height tile, bytes (129 x 129 x 2). // DATA: GDD 45.3
pub const RAW_TILE_BYTES: u64 = geo::gpk::HEIGHT_PAYLOAD_SIZE as u64;

/// Reads the parts in `qa_dir`: the cells (sorted by (y, x)) and the base part, if present.
///
/// # Errors
/// An unreadable directory or part.
pub fn read_parts(qa_dir: &Path) -> Result<(Vec<Value>, Option<Value>), Failure> {
    let mut cells = Vec::new();
    let mut base = None;
    let mut files: Vec<PathBuf> = std::fs::read_dir(qa_dir)
        .map_err(|e| io_err(qa_dir, &e))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    for f in files {
        let v: Value = serde_json::from_slice(&read(&f)?)
            .map_err(|e| Failure::Check(format!("{}: {e}", f.display())))?;
        if v["cell"].is_null() {
            base = Some(v);
        } else {
            cells.push(v);
        }
    }
    cells.sort_by_key(|c| {
        (
            c["cell"][1].as_u64().unwrap_or(0),
            c["cell"][0].as_u64().unwrap_or(0),
        )
    });
    Ok((cells, base))
}

/// The GP2 and GP4 problems of the cell parts.
#[must_use]
pub fn problems(cells: &[Value]) -> Vec<String> {
    let mut out = Vec::new();
    for c in cells {
        let name = format!("cell {},{}", c["cell"][0], c["cell"][1]);
        if c["unfilled_voids"].as_u64() != Some(0) {
            out.push(format!(
                "{name}: {} unfilled voids (GP2)",
                c["unfilled_voids"]
            ));
        }
        for l in c["levels"].as_array().into_iter().flatten() {
            let z = l["z"].as_u64().unwrap_or(0);
            let (s, o) = (
                l["stored"].as_u64().unwrap_or(0),
                l["ocean"].as_u64().unwrap_or(0),
            );
            let all = 1u64 << (2 * z.saturating_sub(5));
            if s + o != all {
                out.push(format!(
                    "{name}: z{z} stored {s} + ocean {o} is not {all} (GP4)"
                ));
            }
            if l["raw_bytes"].as_u64() != Some(s * RAW_TILE_BYTES) {
                out.push(format!(
                    "{name}: z{z} raw bytes {} is not {s} x {RAW_TILE_BYTES} (GP4)",
                    l["raw_bytes"]
                ));
            }
        }
    }
    out
}

/// The per-level totals over the parts: z → (stored, ocean, raw, compressed).
#[must_use]
pub fn totals(parts: &[&Value]) -> BTreeMap<u64, [u64; 4]> {
    let mut t: BTreeMap<u64, [u64; 4]> = BTreeMap::new();
    for p in parts {
        for l in p["levels"].as_array().into_iter().flatten() {
            let e = t.entry(l["z"].as_u64().unwrap_or(0)).or_default();
            for (k, f) in ["stored", "ocean", "raw_bytes", "compressed_bytes"]
                .iter()
                .enumerate()
            {
                e[k] += l[*f].as_u64().unwrap_or(0);
            }
        }
    }
    t
}

/// GDD 45.3's whole-Earth table, z5 to z13: every tile, ocean included, at 33 282 B.
#[must_use]
pub fn gdd_table() -> Vec<Value> {
    (5..=13u32)
        .map(|z| {
            let tiles = 2u64 << (2 * z);
            json!({ "z": z, "tiles": tiles, "raw_bytes": tiles * RAW_TILE_BYTES })
        })
        .collect()
}

/// Runs `qa`: `qa.json` under `out`.
///
/// # Errors
/// [`Failure::Check`] on a GP2 or GP4 problem; I/O failures.
pub fn run(out: &Path, provenance: &[PathBuf]) -> Result<(), Failure> {
    let (cells, base) = read_parts(&out.join("qa"))?;
    let mut prov: Vec<Value> = Vec::new();
    for p in provenance {
        let v: Value = serde_json::from_slice(&read(p)?)
            .map_err(|e| Failure::Check(format!("{}: {e}", p.display())))?;
        prov.extend(v["files"].as_array().cloned().unwrap_or_default());
    }
    prov.sort_by(|a, b| a["url"].as_str().cmp(&b["url"].as_str()));
    prov.dedup_by(|a, b| a["url"] == b["url"]);
    let all: Vec<&Value> = cells.iter().chain(base.iter()).collect();
    let rows: Vec<Value> = totals(&all).into_iter().map(|(z, [s, o, r, c])| json!({ "z": z, "stored": s, "ocean": o, "raw_bytes": r, "compressed_bytes": c })).collect();
    let mut hist: BTreeMap<String, u64> = BTreeMap::new();
    for p in &all {
        for (k, v) in p["eps_hist"].as_object().into_iter().flatten() {
            *hist.entry(k.clone()).or_default() += v.as_u64().unwrap_or(0);
        }
    }
    let bytes: u64 = all
        .iter()
        .map(|p| p["size_bytes"].as_u64().unwrap_or(0))
        .sum();
    let report = json!({
        "schema": "geo_qa/1",
        "datapack": all.first().map_or(Value::Null, |p| p["datapack"].clone()),
        "cells": cells.len(),
        "archive_bytes": bytes,
        "unfilled_voids": cells.iter().map(|c| c["unfilled_voids"].as_u64().unwrap_or(0)).sum::<u64>(),
        "levels": rows,
        "eps_hist": hist,
        "gdd_45_3": gdd_table(),
        "per_cell": cells,
        "base": base,
        "provenance": prov,
    });
    let text =
        serde_json::to_string_pretty(&report).map_err(|e| Failure::Io(e.to_string()))? + "\n";
    write(&out.join("qa.json"), text.as_bytes())?;
    let bad = problems(&cells);
    if bad.is_empty() {
        println!(
            "qa: {} cells, {bytes} archive bytes, 0 unfilled voids",
            cells.len()
        );
        Ok(())
    } else {
        Err(Failure::Check(bad.join("\n")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gdd_table_matches_45_3() {
        let t = gdd_table();
        assert_eq!(t[0]["tiles"], 2_048);
        assert_eq!(t[0]["raw_bytes"], 2_048 * 33_282);
        assert_eq!(t[5]["tiles"], 2_097_152); // z10
        assert_eq!(t[8]["tiles"], 134_217_728); // z13
    }

    #[test]
    fn gp4_inconsistent_counts_are_found() {
        let good = json!({ "cell": [33, 7], "unfilled_voids": 0, "levels": [{ "z": 6, "stored": 3, "ocean": 1, "raw_bytes": 3 * 33_282 }] });
        assert_eq!(problems(std::slice::from_ref(&good)), Vec::<String>::new());
        let bad = json!({ "cell": [33, 7], "unfilled_voids": 2, "levels": [{ "z": 7, "stored": 3, "ocean": 1, "raw_bytes": 5 }] });
        assert_eq!(problems(&[bad]).len(), 3);
    }
}
