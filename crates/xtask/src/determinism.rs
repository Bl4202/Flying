//! Cross-build determinism check (ADR-10, ADR-23, `AC-11.7` precursor).
//!
//! Steps the same seeded simulation in the native `fg` binary and in both WebAssembly builds (run
//! under Node), and requires bit-identical per-subsystem state hashes at every checkpoint. The run
//! has the baked core content pack loaded and the scripted erratic pilot feeding the observer, so
//! the content, input and observer subsystems are exercised too. The native hashes must also equal
//! `tests/golden/determinism.json`, which catches accidental behaviour changes rather than just
//! cross-build drift.

use std::collections::BTreeSet;
use std::fs;
use std::process::Command;

use serde_json::{Value, json};

use crate::util::{run, workspace_root};

/// Same value as `sim_core::DEFAULT_SEED`, as hex.
const SEED_HEX: &str = "1f1e5eed00000001";
const FIELDS: [&str; 7] = [
    "combined", "tick_h", "rng", "bodies", "observer", "input", "content",
];
/// The scripted pilot every run uses.
const SCRIPT: &str = "erratic";
/// The baked pack every run loads, relative to the repository root.
const CONTENT: &str = "content-out/core.cpack";

struct Options {
    ticks: u64,
    bless: bool,
    skip_wasm: bool,
}

fn parse(args: &[String]) -> Result<Options, String> {
    let mut o = Options {
        ticks: 1_000_000,
        bless: false,
        skip_wasm: false,
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--ticks" => {
                let v = it.next().ok_or("--ticks needs a value")?;
                o.ticks = v.parse().map_err(|e| format!("bad --ticks: {e}"))?;
            }
            "--bless" => o.bless = true,
            "--skip-wasm" => o.skip_wasm = true,
            other => return Err(format!("unknown flag {other}")),
        }
    }
    if o.ticks == 0 {
        return Err("--ticks must be positive".into());
    }
    Ok(o)
}

/// Powers of ten, every 100 000, and the final tick.
pub fn schedule(n: u64) -> Vec<u64> {
    let mut set = BTreeSet::new();
    let mut p = 1_u64;
    while p <= n {
        set.insert(p);
        p = p.saturating_mul(10);
    }
    let mut t = 100_000;
    while t <= n {
        set.insert(t);
        t += 100_000;
    }
    set.insert(n);
    set.into_iter().collect()
}

fn list(ticks: &[u64]) -> String {
    ticks
        .iter()
        .map(u64::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

fn parse_lines(label: &str, bytes: &[u8]) -> Result<Vec<Value>, String> {
    String::from_utf8_lossy(bytes)
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).map_err(|e| format!("{label}: bad line {l:?}: {e}")))
        .collect()
}

/// First difference between two checkpoint series, as a readable message.
pub fn first_difference(a_label: &str, a: &[Value], b_label: &str, b: &[Value]) -> Option<String> {
    if a.len() != b.len() {
        return Some(format!(
            "{a_label} has {} checkpoints, {b_label} has {}",
            a.len(),
            b.len()
        ));
    }
    for (x, y) in a.iter().zip(b) {
        if x["tick"] != y["tick"] {
            return Some(format!(
                "tick mismatch: {a_label} {} vs {b_label} {}",
                x["tick"], y["tick"]
            ));
        }
        for f in FIELDS {
            if x[f] != y[f] {
                return Some(format!(
                    "first divergence at tick {}: `{f}` {a_label}={} {b_label}={}",
                    x["tick"], x[f], y[f]
                ));
            }
        }
    }
    None
}

fn capture(cmd: &mut Command) -> Result<Vec<u8>, String> {
    let out = cmd
        .output()
        .map_err(|e| format!("cannot run {cmd:?}: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "{cmd:?} failed with {}\n{}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(out.stdout)
}

pub fn run_command(args: &[String]) -> Result<(), String> {
    let opts = parse(args)?;
    let root = workspace_root();
    let at = list(&schedule(opts.ticks));
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());

    println!("== bake content");
    super::content::run_command(&[])?;
    let content = root.join(CONTENT);
    let content = content.to_str().ok_or("non-UTF-8 repository path")?;
    let setup = ["--seed", SEED_HEX, "--script", SCRIPT, "--content", content];

    println!(
        "== native release, {} ticks, {} checkpoints",
        opts.ticks,
        at.split(',').count()
    );
    run(Command::new(&cargo)
        .current_dir(&root)
        .args(["build", "-q", "-p", "cli", "--release"]))?;
    let fg = root.join("target/release/fg");
    let native = parse_lines(
        "native",
        &capture(
            Command::new(&fg)
                .arg("hash")
                .args(setup)
                .args(["--at", &at]),
        )?,
    )?;

    // Release must equal debug (overflow checks, opt level) at a cheap tick count.
    let probe = 10_000.min(opts.ticks);
    run(Command::new(&cargo)
        .current_dir(&root)
        .args(["build", "-q", "-p", "cli"]))?;
    let debug = parse_lines(
        "native-debug",
        &capture(
            Command::new(root.join("target/debug/fg"))
                .arg("hash")
                .args(setup)
                .args(["--ticks", &probe.to_string()]),
        )?,
    )?;
    let release_at_probe: Vec<Value> = native
        .iter()
        .filter(|v| v["tick"] == probe)
        .cloned()
        .collect();
    if let Some(d) = first_difference("release", &release_at_probe, "debug", &debug) {
        return Err(format!("native release vs debug: {d}"));
    }
    println!("   release == debug at tick {probe}");

    if !opts.skip_wasm {
        if !root.join("wasm-out/wasm-manifest.json").exists() {
            return Err("wasm-out is missing: run `cargo xtask wasm` first".into());
        }
        for variant in ["single", "threads"] {
            println!("== wasm {variant} under node");
            let wasm = parse_lines(
                variant,
                &capture(
                    Command::new("node")
                        .current_dir(&root)
                        .args(["packages/tools/src/wasm-hash.ts", "--variant", variant])
                        .args(setup)
                        .args(["--at", &at]),
                )?,
            )?;
            if let Some(d) = first_difference("native", &native, variant, &wasm) {
                return Err(format!("native vs wasm {variant}: {d}"));
            }
            println!("   identical at all {} checkpoints", native.len());
        }
    }

    check_golden(&root, &native, &opts)
}

fn check_golden(root: &std::path::Path, native: &[Value], opts: &Options) -> Result<(), String> {
    let path = root.join("tests/golden/determinism.json");
    let current = json!({
        "seed": SEED_HEX,
        "script": SCRIPT,
        "ticks": opts.ticks,
        "checkpoints": native,
    });
    if opts.bless {
        fs::create_dir_all(path.parent().expect("has parent")).map_err(|e| e.to_string())?;
        let text = serde_json::to_string_pretty(&current).map_err(|e| e.to_string())?;
        fs::write(&path, format!("{text}\n")).map_err(|e| e.to_string())?;
        println!("blessed {}", path.display());
        return Ok(());
    }
    let stored: Value = serde_json::from_slice(
        &fs::read(&path).map_err(|e| format!("{}: {e} (run with --bless once)", path.display()))?,
    )
    .map_err(|e| e.to_string())?;
    if ["seed", "script", "ticks"]
        .iter()
        .any(|k| stored[*k] != current[*k])
    {
        return Err(format!(
            "golden was recorded for seed {} / script {} / {} ticks, this run is seed {} / script {} / {} ticks",
            stored["seed"],
            stored["script"],
            stored["ticks"],
            current["seed"],
            current["script"],
            current["ticks"]
        ));
    }
    let stored_cp = stored["checkpoints"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if let Some(d) = first_difference("golden", &stored_cp, "native", native) {
        return Err(format!(
            "native differs from tests/golden/determinism.json: {d}\n\
             If this change to simulation behaviour is intended, re-bless with \
             `cargo xtask determinism --bless` and explain why in the PR."
        ));
    }
    println!("== golden: matches ({} checkpoints)", native.len());
    println!("OK: determinism holds for {} ticks", opts.ticks);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cp(tick: u64, combined: &str) -> Value {
        json!({
            "tick": tick, "combined": combined, "tick_h": "t", "rng": "r", "bodies": "b",
            "observer": "o", "input": "i", "content": "c",
        })
    }

    #[test]
    fn schedule_has_powers_of_ten_hundred_thousands_and_end() {
        assert_eq!(schedule(1), [1]);
        assert_eq!(schedule(250), [1, 10, 100, 250]);
        let s = schedule(1_000_000);
        assert_eq!(s.first(), Some(&1));
        assert_eq!(s.last(), Some(&1_000_000));
        assert!(s.contains(&100_000) && s.contains(&500_000) && s.contains(&1_000));
        assert!(s.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn identical_series_have_no_difference() {
        let a = vec![cp(1, "x"), cp(10, "y")];
        assert!(first_difference("a", &a, "b", &a.clone()).is_none());
    }

    #[test]
    fn reports_the_first_divergence() {
        let a = vec![cp(1, "x"), cp(10, "y"), cp(100, "z")];
        let b = vec![cp(1, "x"), cp(10, "Y"), cp(100, "Z")];
        let d = first_difference("native", &a, "wasm", &b).unwrap();
        assert!(d.contains("tick 10") && d.contains("combined"), "{d}");
    }

    #[test]
    fn every_subsystem_column_is_compared() {
        for field in FIELDS {
            let a = vec![cp(1, "x")];
            let mut b = a.clone();
            b[0][field] = json!("different");
            let d = first_difference("a", &a, "b", &b)
                .unwrap_or_else(|| panic!("a difference in `{field}` went unnoticed"));
            assert!(d.contains(field), "{d}");
        }
    }

    #[test]
    fn reports_length_mismatch() {
        let a = vec![cp(1, "x")];
        assert!(
            first_difference("a", &a, "b", &[])
                .unwrap()
                .contains("checkpoints")
        );
    }
}
