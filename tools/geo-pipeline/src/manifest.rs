//! `manifest`: `datapack.json`, schema `datapack/1` (GDD A.15, extended; ADR-54, ADR-55).
//!
//! ```json
//! { "schema": "datapack/1", "latest": "geo-2026.12",
//!   "versions": [ { "id": "geo-2026.12", "released": "2026-12-03", "compatible_game": ">=0.2.0",
//!     "format": 1, "base": { "file": "geo-2026.12/base.gpk", "size_bytes": 0, "sha256": "…" },
//!     "cells": [ { "x": 33, "y": 7, "max_level": 12, "file": "geo-2026.12/cells/5-33-7.gpk",
//!                  "size_bytes": 0, "sha256": "…" } ],
//!     "licences": "geo-2026.12/licences.json", "qa": "geo-2026.12/qa.json" } ] }
//! ```
//!
//! - Cells are sorted by (y, x); a cell not listed is ocean at every level.
//! - Paths are relative to the base URL: `<id>/…` on R2 (ADR-55), or the bare file names with
//!   `--flat`, for a datapack whose files sit beside `datapack.json` (the fixture).
//! - Versions are kept newest first.
//! - Writing a version that already exists replaces it.

use std::path::Path;

use serde_json::{Value, json};

use crate::qa::read_parts;
use crate::{Failure, read, sha256_hex, write};

/// The game versions a v1 datapack serves. // DATA: ADR-55
pub const COMPATIBLE_GAME: &str = ">=0.2.0";
/// The format version. // DATA: ADR-54
pub const FORMAT: u64 = 1;

/// The version entry for datapack `id` from the archives and parts under `out`.
///
/// # Errors
/// A missing `base.gpk` or unreadable parts.
pub fn version_entry(out: &Path, id: &str, released: &str, flat: bool) -> Result<Value, Failure> {
    let dir = if flat {
        String::new()
    } else {
        format!("{id}/")
    };
    let base = read(&out.join("base.gpk"))?;
    let (cells, _) = read_parts(&out.join("qa"))?;
    let cells: Vec<Value> = cells
        .iter()
        .map(|c| {
            json!({
                "x": c["cell"][0], "y": c["cell"][1], "max_level": c["max_level"],
                "file": format!("{dir}{}", c["file"].as_str().unwrap_or_default()),
                "size_bytes": c["size_bytes"], "sha256": c["sha256"],
            })
        })
        .collect();
    Ok(json!({
        "id": id,
        "released": released,
        "compatible_game": COMPATIBLE_GAME,
        "format": FORMAT,
        "base": { "file": format!("{dir}base.gpk"), "size_bytes": base.len(), "sha256": sha256_hex(&base) },
        "cells": cells,
        "licences": format!("{dir}licences.json"),
        "qa": format!("{dir}qa.json"),
    }))
}

/// `previous` with `entry` added (replacing a version of the same id) and made latest.
#[must_use]
pub fn merge(previous: Option<&Value>, entry: Value) -> Value {
    let id = entry["id"].as_str().unwrap_or_default().to_string();
    let mut versions: Vec<Value> = previous
        .and_then(|p| p["versions"].as_array().cloned())
        .unwrap_or_default();
    versions.retain(|v| v["id"].as_str() != Some(&id));
    versions.push(entry);
    versions.sort_by(|a, b| b["id"].as_str().cmp(&a["id"].as_str()));
    json!({ "schema": "datapack/1", "latest": id, "versions": versions })
}

/// Runs `manifest`: `datapack.json` under `out`.
///
/// # Errors
/// As [`version_entry`], or a malformed `previous`.
pub fn run(
    out: &Path,
    id: &str,
    released: &str,
    previous: Option<&Path>,
    flat: bool,
) -> Result<(), Failure> {
    let prev = match previous {
        Some(p) if p.exists() => Some(
            serde_json::from_slice::<Value>(&read(p)?)
                .map_err(|e| Failure::Check(format!("{}: {e}", p.display())))?,
        ),
        _ => None,
    };
    if let Some(p) = &prev
        && p["schema"] != "datapack/1"
    {
        return Err(Failure::Check(
            "the previous datapack.json is not datapack/1".into(),
        ));
    }
    let doc = merge(prev.as_ref(), version_entry(out, id, released, flat)?);
    let text = serde_json::to_string_pretty(&doc).map_err(|e| Failure::Io(e.to_string()))? + "\n";
    write(&out.join("datapack.json"), text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merging_keeps_old_versions_and_replaces_a_rerun() {
        let a = merge(None, json!({ "id": "geo-2026.11", "n": 1 }));
        let b = merge(Some(&a), json!({ "id": "geo-2026.12", "n": 2 }));
        let c = merge(Some(&b), json!({ "id": "geo-2026.12", "n": 3 }));
        assert_eq!(c["latest"], "geo-2026.12");
        let v = c["versions"].as_array().unwrap();
        assert_eq!(v.len(), 2);
        assert_eq!(v[0]["n"], 3);
        assert_eq!(v[1]["id"], "geo-2026.11");
    }
}
