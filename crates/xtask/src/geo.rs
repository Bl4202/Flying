//! `cargo xtask geo-fixture [--repro]`: the synthetic geodata fixture (ADR-51 section 9.5,
//! ADR-54) in `target/geo-fixture`, built by `geo-pipeline synth`.
//!
//! The world determinism suite, the e2e specs and the perf smoke read it. With `--repro` it is
//! built twice into two other directories and every file's SHA-256 compared (GP1).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::util::{hex, run, workspace_root};

/// Where the fixture goes.
pub const FIXTURE_DIR: &str = "target/geo-fixture";

/// Builds the fixture into `out` (removed first).
pub fn build(root: &Path, out: &Path) -> Result<(), String> {
    if out.exists() {
        fs::remove_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    }
    let mut cmd = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
    cmd.current_dir(root)
        .args([
            "run",
            "--quiet",
            "--release",
            "--package",
            "geo-pipeline",
            "--",
            "synth",
            "--out",
        ])
        .arg(out);
    run(&mut cmd)
}

/// Every file under `dir` (relative path) with its SHA-256.
fn hashes(dir: &Path) -> Result<BTreeMap<PathBuf, String>, String> {
    use sha2::{Digest, Sha256};
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(&d).map_err(|e| format!("{}: {e}", d.display()))? {
            let p = e.map_err(|e| e.to_string())?.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                let bytes = fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
                let rel = p
                    .strip_prefix(dir)
                    .map_err(|e| e.to_string())?
                    .to_path_buf();
                out.insert(rel, hex(&Sha256::digest(&bytes)));
            }
        }
    }
    Ok(out)
}

pub fn run_command(args: &[String]) -> Result<(), String> {
    let repro = match args {
        [] => false,
        [a] if a == "--repro" => true,
        _ => return Err("usage: cargo xtask geo-fixture [--repro]".into()),
    };
    let root = workspace_root();
    build(&root, &root.join(FIXTURE_DIR))?;
    println!("geo-fixture: built {FIXTURE_DIR}");
    if repro {
        let (a, b) = (
            root.join("target/geo-fixture-repro/a"),
            root.join("target/geo-fixture-repro/b"),
        );
        build(&root, &a)?;
        build(&root, &b)?;
        let (ha, hb) = (hashes(&a)?, hashes(&b)?);
        if ha != hb {
            let differ: Vec<String> = ha
                .iter()
                .filter(|(k, v)| hb.get(*k) != Some(v))
                .map(|(k, _)| k.display().to_string())
                .chain(
                    hb.keys()
                        .filter(|k| !ha.contains_key(*k))
                        .map(|k| k.display().to_string()),
                )
                .collect();
            return Err(format!(
                "geo-fixture: two builds differ (GP1):\n  {}",
                differ.join("\n  ")
            ));
        }
        println!(
            "geo-fixture: {} files byte-identical across two builds (GP1)",
            ha.len()
        );
    }
    Ok(())
}
