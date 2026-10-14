//! Crate dependency layering (GDD 61.7, `AC-61.1`).
//!
//! Lower layers never depend on higher ones. The rules are an explicit allow-list of each crate's
//! normal and build dependencies; anything not listed is a violation. Dev-dependencies are exempt.

use std::collections::BTreeMap;
use std::process::Command;

use serde_json::Value;

/// `(crate, allowed dependencies)`. `xtask` is tooling and is not constrained.
const RULES: &[(&str, &[&str])] = &[
    ("simmath", &["libm"]),
    ("flight", &["simmath", "content"]),
    ("sim-core", &["content", "simmath"]),
    ("protocol", &[]),
    ("content", &[]),
    (
        "content-bake",
        &[
            "content",
            "schemars",
            "serde",
            "serde_json",
            "serde_path_to_error",
            "sha2",
        ],
    ),
    ("sim-wasm", &["sim-core"]),
    ("cli", &["sim-core"]),
    ("server", &["sim-core", "protocol"]),
];

pub fn check() -> Result<(), String> {
    let out = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .output()
        .map_err(|e| format!("cannot run cargo metadata: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    let meta: Value = serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
    let deps = dependencies(&meta)?;
    violations(&deps)
}

/// Package name to the names of its normal and build dependencies.
fn dependencies(meta: &Value) -> Result<BTreeMap<String, Vec<String>>, String> {
    let packages = meta["packages"]
        .as_array()
        .ok_or("metadata has no packages")?;
    let mut map = BTreeMap::new();
    for p in packages {
        let name = p["name"]
            .as_str()
            .ok_or("package without name")?
            .to_string();
        let mut deps: Vec<String> = p["dependencies"]
            .as_array()
            .ok_or("package without dependencies")?
            .iter()
            .filter(|d| d["kind"].is_null() || d["kind"] == "build")
            .filter_map(|d| d["name"].as_str().map(str::to_string))
            .collect();
        deps.sort();
        deps.dedup();
        map.insert(name, deps);
    }
    Ok(map)
}

fn violations(deps: &BTreeMap<String, Vec<String>>) -> Result<(), String> {
    let mut problems = Vec::new();
    for (krate, allowed) in RULES {
        let Some(actual) = deps.get(*krate) else {
            problems.push(format!("rule for `{krate}` but no such workspace crate"));
            continue;
        };
        for d in actual {
            if !allowed.contains(&d.as_str()) {
                problems.push(format!(
                    "`{krate}` depends on `{d}`, allowed: [{}]",
                    allowed.join(", ")
                ));
            }
        }
    }
    for krate in deps.keys() {
        if krate != "xtask" && !RULES.iter().any(|(k, _)| k == krate) {
            problems.push(format!(
                "workspace crate `{krate}` has no layering rule; add one"
            ));
        }
    }
    if problems.is_empty() {
        println!("layers ok ({} crates checked)", RULES.len());
        Ok(())
    } else {
        Err(format!("layering violations:\n  {}", problems.join("\n  ")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(entries: &[(&str, &[&str])]) -> BTreeMap<String, Vec<String>> {
        entries
            .iter()
            .map(|(k, v)| {
                (
                    (*k).to_string(),
                    v.iter().map(|s| (*s).to_string()).collect(),
                )
            })
            .collect()
    }

    fn good() -> BTreeMap<String, Vec<String>> {
        map(&[
            ("simmath", &["libm"]),
            ("flight", &["simmath"]),
            ("sim-core", &["content", "simmath"]),
            ("protocol", &[]),
            ("content", &[]),
            ("content-bake", &["content", "serde_json", "sha2"]),
            ("sim-wasm", &["sim-core"]),
            ("cli", &["sim-core"]),
            ("server", &["protocol", "sim-core"]),
            ("xtask", &["serde_json"]),
        ])
    }

    #[test]
    fn accepts_the_intended_graph() {
        assert!(violations(&good()).is_ok());
    }

    #[test]
    fn rejects_sim_core_depending_on_protocol() {
        let mut g = good();
        g.get_mut("sim-core").unwrap().push("protocol".into());
        let err = violations(&g).unwrap_err();
        assert!(err.contains("`sim-core` depends on `protocol`"), "{err}");
    }

    #[test]
    fn the_runtime_content_crate_stays_dependency_free() {
        let mut g = good();
        g.get_mut("content").unwrap().push("serde".into());
        let err = violations(&g).unwrap_err();
        assert!(err.contains("`content` depends on `serde`"), "{err}");
    }

    #[test]
    fn the_baker_may_not_leak_into_the_sim() {
        let mut g = good();
        g.get_mut("sim-core").unwrap().push("content-bake".into());
        let err = violations(&g).unwrap_err();
        assert!(
            err.contains("`sim-core` depends on `content-bake`"),
            "{err}"
        );
    }

    #[test]
    fn rejects_unruled_crates() {
        let mut g = good();
        g.insert("mystery".into(), vec![]);
        assert!(violations(&g).unwrap_err().contains("mystery"));
    }
}
