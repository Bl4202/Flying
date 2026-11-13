//! Source descriptors, `geo_source/1` (GDD 45.9, ADR-54): one per data source, in
//! `content/world/sources/`.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{Failure, io_err, read};

/// One data source's licence terms (GDD 45.9).
#[allow(clippy::struct_excessive_bools)] // DATA: GDD 45.9's descriptor fields, one flag per term
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeoSource {
    /// Always `geo_source/1`.
    pub schema: String,
    /// The id archives name in their `licences`.
    pub id: String,
    /// The source's name.
    pub name: String,
    /// The licence's name.
    pub licence: String,
    /// The attribution line.
    pub attribution: String,
    /// Where the terms are.
    pub url: String,
    /// The terms allow commercial use.
    pub commercial_ok: bool,
    /// The terms allow caching.
    pub cache_allowed: bool,
    /// The terms allow redistributing derived tiles.
    pub redistribution_allowed: bool,
    /// The terms require share-alike.
    pub share_alike: bool,
    /// When the terms were last read (ISO date).
    pub terms_checked: String,
    /// The notices the terms require, word for word.
    pub required_notices: Vec<String>,
    /// The file-name prefixes the descriptor covers.
    #[serde(default)]
    pub products: Vec<String>,
    /// For a share-alike database (ODbL): where its public dump is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dump: Option<String>,
}

/// The schema name.
pub const SCHEMA: &str = "geo_source/1";

/// Reads every `*.json` descriptor in `dir`, sorted by id.
///
/// # Errors
/// An unreadable directory or file, a malformed descriptor, or a duplicate id.
pub fn load_dir(dir: &Path) -> Result<Vec<GeoSource>, Failure> {
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| io_err(dir, &e))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    let mut out: Vec<GeoSource> = Vec::new();
    for f in files {
        let d: GeoSource = serde_json::from_slice(&read(&f)?)
            .map_err(|e| Failure::Check(format!("{}: {e}", f.display())))?;
        if d.schema != SCHEMA {
            return Err(Failure::Check(format!(
                "{}: schema {:?}, want {SCHEMA}",
                f.display(),
                d.schema
            )));
        }
        if out.iter().any(|o| o.id == d.id) {
            return Err(Failure::Check(format!(
                "{}: duplicate descriptor id {}",
                f.display(),
                d.id
            )));
        }
        out.push(d);
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}
