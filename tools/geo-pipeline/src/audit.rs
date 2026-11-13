//! `audit`: the licence audit of a datapack (GDD 45.9, `AC-45.4`, ADR-50, ADR-54).
//!
//! The audit fails if any of these holds:
//! - an archive names a licence with no descriptor;
//! - a descriptor's `terms_checked` is more than 365 days before the build date;
//! - `commercial_ok` is false (while OQ-04 is open, ADR-50);
//! - `cache_allowed` or `redistribution_allowed` is false;
//! - an `ODbL` source has no `dump`.
//!
//! On success it writes `licences.json`: the descriptors the datapack uses, sorted by id.

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::json;

use crate::descriptors::{GeoSource, load_dir};
use crate::{Failure, days_from_iso, read, write};

/// The longest a descriptor's terms may go unread, days. // DATA: GDD 45.9, 12 months
pub const MAX_TERMS_AGE_DAYS: i64 = 365;

/// The licence ids named by the datapack's archives: the base pack's header and the cells' QA
/// parts (which record their archives' headers).
///
/// # Errors
/// An unreadable or malformed file.
pub fn used_licences(out: &Path) -> Result<BTreeSet<String>, Failure> {
    let mut used = BTreeSet::new();
    let base = out.join("base.gpk");
    if base.exists() {
        let a = geo_pack::Archive::from_file(&base).map_err(Failure::Check)?;
        used.extend(a.header_json().licences.iter().cloned());
    }
    let qa = out.join("qa");
    if let Ok(rd) = std::fs::read_dir(&qa) {
        for e in rd.filter_map(Result::ok) {
            let p = e.path();
            if p.extension().is_some_and(|x| x == "json") {
                let v: serde_json::Value = serde_json::from_slice(&read(&p)?)
                    .map_err(|e| Failure::Check(format!("{}: {e}", p.display())))?;
                for l in v["licences"].as_array().into_iter().flatten() {
                    used.insert(l.as_str().unwrap_or_default().to_string());
                }
            }
        }
    }
    Ok(used)
}

/// Checks `used` against `descriptors` for a build on `built`; the descriptors used.
///
/// # Errors
/// [`Failure::Check`] listing every failure, one per line.
pub fn check(
    used: &BTreeSet<String>,
    descriptors: &[GeoSource],
    built: &str,
) -> Result<Vec<GeoSource>, Failure> {
    let today = days_from_iso(built)?;
    let mut problems = Vec::new();
    let mut found = Vec::new();
    if used.is_empty() {
        problems.push("no archive names a licence".to_string());
    }
    for id in used {
        let Some(d) = descriptors.iter().find(|d| &d.id == id) else {
            problems.push(format!("licence {id}: no descriptor"));
            continue;
        };
        let age = today - days_from_iso(&d.terms_checked)?;
        if age > MAX_TERMS_AGE_DAYS {
            problems.push(format!("licence {id}: terms_checked {} is {age} days before the build (at most {MAX_TERMS_AGE_DAYS})", d.terms_checked));
        }
        if !d.commercial_ok {
            problems.push(format!("licence {id}: commercial_ok is false (only commercial-safe data while OQ-04 is open, ADR-50)"));
        }
        if !d.cache_allowed {
            problems.push(format!("licence {id}: cache_allowed is false"));
        }
        if !d.redistribution_allowed {
            problems.push(format!("licence {id}: redistribution_allowed is false"));
        }
        if d.licence.to_ascii_lowercase().contains("odbl") && d.dump.is_none() {
            problems.push(format!("licence {id}: an ODbL source needs a public dump"));
        }
        found.push(d.clone());
    }
    if problems.is_empty() {
        Ok(found)
    } else {
        Err(Failure::Check(problems.join("\n")))
    }
}

/// Runs `audit`.
///
/// # Errors
/// As [`check`], or an I/O failure.
pub fn run(out: &Path, sources: &Path, built: &str) -> Result<(), Failure> {
    let used = used_licences(out)?;
    let descriptors = load_dir(sources)?;
    let found = check(&used, &descriptors, built)?;
    let doc = json!({ "schema": "geo_licences/1", "built": built, "descriptors": found });
    let text = serde_json::to_string_pretty(&doc).map_err(|e| Failure::Io(e.to_string()))? + "\n";
    write(&out.join("licences.json"), text.as_bytes())?;
    println!("audit: {} licence(s) pass", found.len());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor() -> GeoSource {
        GeoSource {
            schema: "geo_source/1".into(),
            id: "synthetic".into(),
            name: "Synthetic test terrain".into(),
            licence: "none".into(),
            attribution: "Synthetic terrain".into(),
            url: "https://example.invalid".into(),
            commercial_ok: true,
            cache_allowed: true,
            redistribution_allowed: true,
            share_alike: false,
            terms_checked: "2026-11-12".into(),
            required_notices: vec![],
            products: vec![],
            dump: None,
        }
    }

    fn used() -> BTreeSet<String> {
        BTreeSet::from(["synthetic".to_string()])
    }

    #[test]
    fn la1_a_good_descriptor_passes() {
        assert_eq!(
            check(&used(), &[descriptor()], "2026-11-13").unwrap().len(),
            1
        );
        assert!(
            check(&used(), &[descriptor()], "2027-11-12").is_ok(),
            "exactly 365 days"
        );
    }

    #[test]
    fn la2_a_missing_descriptor_fails() {
        let e = check(&used(), &[], "2026-11-13").unwrap_err();
        assert_eq!(e, Failure::Check("licence synthetic: no descriptor".into()));
    }

    #[test]
    fn la2_old_terms_fail() {
        let d = GeoSource {
            terms_checked: "2025-10-09".into(),
            ..descriptor()
        };
        let Failure::Check(m) = check(&used(), &[d], "2026-11-13").unwrap_err() else {
            panic!()
        };
        assert!(m.contains("400 days before the build"), "{m}");
    }

    #[test]
    fn la2_commercial_ok_false_fails() {
        let d = GeoSource {
            commercial_ok: false,
            ..descriptor()
        };
        let Failure::Check(m) = check(&used(), &[d], "2026-11-13").unwrap_err() else {
            panic!()
        };
        assert!(m.contains("commercial_ok is false"), "{m}");
    }

    #[test]
    fn odbl_needs_a_dump_and_caching_is_required() {
        let d = GeoSource {
            licence: "ODbL-1.0".into(),
            cache_allowed: false,
            ..descriptor()
        };
        let Failure::Check(m) = check(&used(), &[d], "2026-11-13").unwrap_err() else {
            panic!()
        };
        assert!(m.contains("ODbL") && m.contains("cache_allowed"), "{m}");
    }
}
