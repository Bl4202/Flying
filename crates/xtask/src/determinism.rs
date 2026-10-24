//! Cross-build determinism check (ADR-10, ADR-23, `AC-11.7`).
//!
//! Steps the same seeded simulation in the native `fg` binary (release and debug) and in both
//! WebAssembly builds (run under Node), and requires bit-identical per-subsystem state hashes at
//! every checkpoint. Three suites run, all with the baked core pack loaded, so the player's aircraft
//! flies the 6-DOF model, each under the session it names (ADR-36):
//!
//! - `erratic`: the seeded random-walk pilot for `--ticks` (10^6 by default) with crashes, resets
//!   and respawns; golden `tests/golden/determinism.json`.
//! - `flight60`: the 60 s scripted flight of `AC-11.7`, every second; golden
//!   `tests/golden/flight60.json`.
//! - `circuit`: the virtual tester's take-off, circuit and landing from the runway (ADR-33), every
//!   ten seconds for six minutes; golden `tests/golden/circuit.json`.
//!
//! The native hashes must also equal the goldens, which catches accidental behaviour changes
//! rather than just cross-build drift. `--compare-only` skips the goldens (native against WASM
//! only), for work in progress that is known to change behaviour.

use std::collections::BTreeSet;
use std::fs;
use std::process::Command;

use serde_json::{Value, json};

use crate::util::{run, workspace_root};

/// Same value as `sim_core::DEFAULT_SEED`, as hex.
const SEED_HEX: &str = "1f1e5eed00000001";
const FIELDS: [&str; 7] = [
    "combined", "tick_h", "rng", "bodies", "ownship", "input", "content",
];
/// The baked pack every run loads, relative to the repository root.
const CONTENT: &str = "content-out/core.cpack";
/// Length of the flight60 script, ticks.
const FLIGHT60_TICKS: u64 = 3_600;
/// Its checkpoint spacing: every second.
const FLIGHT60_EVERY: u64 = 60;
/// Length of the circuit suite, ticks (six minutes: the sortie takes about 220 s, the rest holds
/// the aircraft at rest).
const CIRCUIT_TICKS: u64 = 21_600;
/// Its checkpoint spacing: every ten seconds.
const CIRCUIT_EVERY: u64 = 600;

struct Options {
    ticks: u64,
    bless: bool,
    skip_wasm: bool,
    compare_only: bool,
}

fn parse(args: &[String]) -> Result<Options, String> {
    let mut o = Options {
        ticks: 1_000_000,
        bless: false,
        skip_wasm: false,
        compare_only: false,
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
            "--compare-only" => o.compare_only = true,
            other => return Err(format!("unknown flag {other}")),
        }
    }
    if o.ticks == 0 {
        return Err("--ticks must be positive".into());
    }
    if o.bless && o.compare_only {
        return Err("--bless and --compare-only do not go together".into());
    }
    Ok(o)
}

/// One scripted run and its golden.
struct Suite {
    /// The `--script` name.
    script: &'static str,
    /// Checkpoint ticks, increasing.
    at: Vec<u64>,
    /// Golden file, relative to the repository root.
    golden: &'static str,
    /// The debug build is compared at this tick (debug is slow).
    debug_probe: u64,
    /// The session the suite runs under, as `fg hash` / `wasm-hash.ts` flags (ADR-36).
    session: [&'static str; 6],
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

/// `0, k, 2k, ..., n`.
fn every(n: u64, k: u64) -> Vec<u64> {
    (0..=n / k).map(|i| i * k).collect()
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
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());

    println!("== bake content");
    super::content::run_command(&[])?;
    let content = root.join(CONTENT);
    let content = content.to_str().ok_or("non-UTF-8 repository path")?;
    run(Command::new(&cargo)
        .current_dir(&root)
        .args(["build", "-q", "-p", "cli", "--release"]))?;
    run(Command::new(&cargo)
        .current_dir(&root)
        .args(["build", "-q", "-p", "cli"]))?;
    if !opts.skip_wasm && !root.join("wasm-out/wasm-manifest.json").exists() {
        return Err("wasm-out is missing: run `cargo xtask wasm` first".into());
    }

    let suites = [
        Suite {
            script: "erratic",
            at: schedule(opts.ticks),
            golden: "tests/golden/determinism.json",
            debug_probe: 10_000.min(opts.ticks),
            session: ["--spawn", "air", "--assist", "standard", "--g", "full"],
        },
        Suite {
            script: "flight60",
            at: every(FLIGHT60_TICKS, FLIGHT60_EVERY),
            golden: "tests/golden/flight60.json",
            debug_probe: FLIGHT60_TICKS,
            session: ["--spawn", "air", "--assist", "manual", "--g", "off"],
        },
        Suite {
            script: "circuit",
            at: every(CIRCUIT_TICKS, CIRCUIT_EVERY),
            golden: "tests/golden/circuit.json",
            debug_probe: 3_000,
            session: ["--spawn", "runway", "--assist", "standard", "--g", "full"],
        },
    ];
    for suite in &suites {
        run_suite(&root, suite, content, &opts)?;
    }
    if opts.compare_only {
        println!("OK: native and WASM agree (goldens not checked: --compare-only)");
    } else {
        println!("OK: determinism holds");
    }
    Ok(())
}

fn run_suite(
    root: &std::path::Path,
    suite: &Suite,
    content: &str,
    opts: &Options,
) -> Result<(), String> {
    let setup = [
        "--seed",
        SEED_HEX,
        "--script",
        suite.script,
        "--content",
        content,
        suite.session[0],
        suite.session[1],
        suite.session[2],
        suite.session[3],
        suite.session[4],
        suite.session[5],
    ];
    let at = list(&suite.at);
    let last = *suite.at.last().expect("a suite has checkpoints");
    println!(
        "== {}: native release, {last} ticks, {} checkpoints",
        suite.script,
        suite.at.len()
    );
    let native = parse_lines(
        "native",
        &capture(
            Command::new(root.join("target/release/fg"))
                .arg("hash")
                .args(setup)
                .args(["--at", &at]),
        )?,
    )?;

    // Release must equal debug (overflow checks, opt level).
    let probe = suite.debug_probe;
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
        return Err(format!("{}: native release vs debug: {d}", suite.script));
    }
    println!("   release == debug at tick {probe}");

    if !opts.skip_wasm {
        for variant in ["single", "threads"] {
            println!("== {}: wasm {variant} under node", suite.script);
            let wasm = parse_lines(
                variant,
                &capture(
                    Command::new("node")
                        .current_dir(root)
                        .args(["packages/tools/src/wasm-hash.ts", "--variant", variant])
                        .args(setup)
                        .args(["--at", &at]),
                )?,
            )?;
            if let Some(d) = first_difference("native", &native, variant, &wasm) {
                return Err(format!("{}: native vs wasm {variant}: {d}", suite.script));
            }
            println!("   identical at all {} checkpoints", native.len());
        }
    }

    if opts.compare_only {
        return Ok(());
    }
    check_golden(root, suite, &native, last, opts.bless)
}

fn check_golden(
    root: &std::path::Path,
    suite: &Suite,
    native: &[Value],
    ticks: u64,
    bless: bool,
) -> Result<(), String> {
    let path = root.join(suite.golden);
    let session = suite.session.join(" ");
    let current = json!({
        "seed": SEED_HEX,
        "script": suite.script,
        "session": session,
        "ticks": ticks,
        "checkpoints": native,
    });
    if bless {
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
    // A golden recorded before the session was written down (v0.1a) has no `session` key; the
    // first bless that records one makes the comparison strict.
    let session_differs = !stored["session"].is_null() && stored["session"] != current["session"];
    if session_differs
        || ["seed", "script", "ticks"]
            .iter()
            .any(|k| stored[*k] != current[*k])
    {
        return Err(format!(
            "{} was recorded for seed {} / script {} / {} ticks, this run is seed {} / script {} / {} ticks",
            suite.golden,
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
            "native differs from {}: {d}\n\
             If this change to simulation behaviour is intended, re-bless with \
             `cargo xtask determinism --bless` and explain why in the PR.",
            suite.golden
        ));
    }
    println!(
        "== {}: golden matches ({} checkpoints)",
        suite.script,
        native.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cp(tick: u64, combined: &str) -> Value {
        json!({
            "tick": tick, "combined": combined, "tick_h": "t", "rng": "r", "bodies": "b",
            "ownship": "o", "input": "i", "content": "c",
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
    fn flight60_checkpoints_every_second() {
        let at = every(FLIGHT60_TICKS, FLIGHT60_EVERY);
        assert_eq!(at.len(), 61);
        assert_eq!((at[0], at[60]), (0, 3_600));
    }

    #[test]
    fn the_circuit_suite_checkpoints_every_ten_seconds_for_six_minutes() {
        let at = every(CIRCUIT_TICKS, CIRCUIT_EVERY);
        assert_eq!(at.len(), 37);
        assert_eq!((at[0], at[36]), (0, 21_600));
        assert!(at.contains(&3_000), "the debug probe is a checkpoint");
    }

    #[test]
    fn bless_and_compare_only_exclude_each_other() {
        let args = |l: &[&str]| l.iter().map(|s| (*s).to_string()).collect::<Vec<_>>();
        assert!(parse(&args(&["--bless", "--compare-only"])).is_err());
        assert!(parse(&args(&["--compare-only"])).unwrap().compare_only);
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
