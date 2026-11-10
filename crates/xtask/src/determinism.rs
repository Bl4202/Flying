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
//! - `plain-circuit`: the same tester in the `plain` arena (ADR-43) for two minutes, every ten
//!   seconds, so the second arena's ground contact is covered; golden
//!   `tests/golden/plain-circuit.json`.
//!
//! The native hashes must also equal the goldens, which catches accidental behaviour changes
//! rather than just cross-build drift. `--compare-only` skips the goldens (native against WASM
//! only), for work in progress that is known to change behaviour. `--suite NAME` runs one suite,
//! so a new suite can be blessed without touching the others.

use std::collections::BTreeSet;
use std::fs;
use std::process::Command;

use serde_json::{Value, json};

use crate::util::{run, workspace_root};

/// Same value as `sim_core::DEFAULT_SEED`, as hex.
const SEED_HEX: &str = "1f1e5eed00000001";
const FIELDS: [&str; 8] = [
    "combined", "tick_h", "rng", "bodies", "ownship", "input", "content", "combat",
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
/// Length of the plain-circuit suite, ticks (two minutes: the take-off roll, lift-off and climb).
const PLAIN_CIRCUIT_TICKS: u64 = 7_200;
/// The combat suite: the bot's whole sortie against a Regular, 10 minutes (ADR-45 DT2).
const COMBAT_TICKS: u64 = 36_000;
/// ... checkpointed every 10 s.
const COMBAT_EVERY: u64 = 600;

struct Options {
    ticks: u64,
    bless: bool,
    skip_wasm: bool,
    compare_only: bool,
    /// Run only the suite of this name.
    suite: Option<String>,
}

fn parse(args: &[String]) -> Result<Options, String> {
    let mut o = Options {
        ticks: 1_000_000,
        bless: false,
        skip_wasm: false,
        compare_only: false,
        suite: None,
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
            "--suite" => o.suite = Some(it.next().ok_or("--suite needs a value")?.clone()),
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
    /// What `--suite` calls it.
    name: &'static str,
    /// The `--script` name.
    script: &'static str,
    /// Checkpoint ticks, increasing.
    at: Vec<u64>,
    /// Golden file, relative to the repository root.
    golden: &'static str,
    /// The debug build is compared at this tick (debug is slow).
    debug_probe: u64,
    /// The session the suite runs under, as `fg hash` / `wasm-hash.ts` flags (ADR-36). The golden
    /// stores them joined and compares them strictly, so a suite's list never changes once
    /// blessed (the v0.1b suites name no arena: it is the default).
    session: &'static [&'static str],
}

/// The suites, in the order they run.
fn suites(ticks: u64) -> [Suite; 5] {
    [
        Suite {
            name: "erratic",
            script: "erratic",
            at: schedule(ticks),
            golden: "tests/golden/determinism.json",
            debug_probe: 10_000.min(ticks),
            session: &["--spawn", "air", "--assist", "standard", "--g", "full"],
        },
        Suite {
            name: "flight60",
            script: "flight60",
            at: every(FLIGHT60_TICKS, FLIGHT60_EVERY),
            golden: "tests/golden/flight60.json",
            debug_probe: FLIGHT60_TICKS,
            session: &["--spawn", "air", "--assist", "manual", "--g", "off"],
        },
        Suite {
            name: "circuit",
            script: "circuit",
            at: every(CIRCUIT_TICKS, CIRCUIT_EVERY),
            golden: "tests/golden/circuit.json",
            debug_probe: 3_000,
            session: &["--spawn", "runway", "--assist", "standard", "--g", "full"],
        },
        Suite {
            name: "plain-circuit",
            script: "circuit",
            at: every(PLAIN_CIRCUIT_TICKS, CIRCUIT_EVERY),
            golden: "tests/golden/plain-circuit.json",
            debug_probe: 3_000,
            session: &[
                "--spawn", "runway", "--assist", "standard", "--g", "full", "--arena", "plain",
            ],
        },
        Suite {
            name: "combat",
            script: "sortie",
            at: every(COMBAT_TICKS, COMBAT_EVERY),
            golden: "tests/golden/combat.json",
            debug_probe: 3_000,
            session: &[
                "--spawn",
                "runway",
                "--assist",
                "standard",
                "--g",
                "full",
                "--scenario",
                "duel",
                "--skill",
                "2",
            ],
        },
    ]
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

/// Bakes the content and builds the release `fg`; returns the path of the baked pack.
pub(crate) fn bake_and_build_release(root: &std::path::Path) -> Result<String, String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    println!("== bake content");
    super::content::run_command(&[])?;
    let content = root.join(CONTENT);
    let content = content
        .to_str()
        .ok_or("non-UTF-8 repository path")?
        .to_owned();
    run(Command::new(&cargo)
        .current_dir(root)
        .args(["build", "-q", "-p", "cli", "--release"]))?;
    Ok(content)
}

/// The native release hashes of the suite whose golden is `golden` (a path relative to the
/// repository root), as the golden file would hold them: the same keys, with the checkpoints of a
/// fresh run. `ticks` is the length of the `erratic` suite. Used by `golden-diff --fresh`.
pub(crate) fn fresh_golden(
    root: &std::path::Path,
    golden: &str,
    content: &str,
    ticks: u64,
) -> Result<Value, String> {
    let all = suites(ticks);
    let suite = all
        .iter()
        .find(|s| s.golden == golden)
        .ok_or_else(|| format!("no suite has the golden {golden}"))?;
    let native = native_checkpoints(root, suite, content)?;
    let last = *suite.at.last().expect("a suite has checkpoints");
    Ok(current_golden(suite, &native, last))
}

/// What a golden file for `suite` and `native` holds.
fn current_golden(suite: &Suite, native: &[Value], ticks: u64) -> Value {
    json!({
        "seed": SEED_HEX,
        "script": suite.script,
        "session": suite.session.join(" "),
        "ticks": ticks,
        "checkpoints": native,
    })
}

/// The `fg hash` arguments that set a suite up.
fn setup_args<'a>(suite: &'a Suite, content: &'a str) -> Vec<&'a str> {
    let mut setup = vec![
        "--seed",
        SEED_HEX,
        "--script",
        suite.script,
        "--content",
        content,
    ];
    setup.extend_from_slice(suite.session);
    setup
}

/// Runs the suite in the release `fg` and returns its checkpoints.
fn native_checkpoints(
    root: &std::path::Path,
    suite: &Suite,
    content: &str,
) -> Result<Vec<Value>, String> {
    let setup = setup_args(suite, content);
    let at = list(&suite.at);
    let last = *suite.at.last().expect("a suite has checkpoints");
    println!(
        "== {}: native release, {last} ticks, {} checkpoints",
        suite.name,
        suite.at.len()
    );
    parse_lines(
        "native",
        &capture(
            Command::new(root.join("target/release/fg"))
                .arg("hash")
                .args(&setup)
                .args(["--at", &at]),
        )?,
    )
}

pub fn run_command(args: &[String]) -> Result<(), String> {
    let opts = parse(args)?;
    let root = workspace_root();
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());

    let content = bake_and_build_release(&root)?;
    let content = content.as_str();
    run(Command::new(&cargo)
        .current_dir(&root)
        .args(["build", "-q", "-p", "cli"]))?;
    if !opts.skip_wasm && !root.join("wasm-out/wasm-manifest.json").exists() {
        return Err("wasm-out is missing: run `cargo xtask wasm` first".into());
    }

    let all = suites(opts.ticks);
    if let Some(name) = &opts.suite
        && !all.iter().any(|s| s.name == name)
    {
        let names: Vec<_> = all.iter().map(|s| s.name).collect();
        return Err(format!(
            "unknown --suite {name} (one of {})",
            names.join(", ")
        ));
    }
    let suites: Vec<&Suite> = all
        .iter()
        .filter(|s| opts.suite.as_deref().is_none_or(|n| n == s.name))
        .collect();
    for suite in suites {
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
    let setup = setup_args(suite, content);
    let at = list(&suite.at);
    let last = *suite.at.last().expect("a suite has checkpoints");
    let native = native_checkpoints(root, suite, content)?;

    // Release must equal debug (overflow checks, opt level).
    let probe = suite.debug_probe;
    let debug = parse_lines(
        "native-debug",
        &capture(
            Command::new(root.join("target/debug/fg"))
                .arg("hash")
                .args(&setup)
                .args(["--ticks", &probe.to_string()]),
        )?,
    )?;
    let release_at_probe: Vec<Value> = native
        .iter()
        .filter(|v| v["tick"] == probe)
        .cloned()
        .collect();
    if let Some(d) = first_difference("release", &release_at_probe, "debug", &debug) {
        return Err(format!("{}: native release vs debug: {d}", suite.name));
    }
    println!("   release == debug at tick {probe}");

    if !opts.skip_wasm {
        for variant in ["single", "threads"] {
            println!("== {}: wasm {variant} under node", suite.name);
            let wasm = parse_lines(
                variant,
                &capture(
                    Command::new("node")
                        .current_dir(root)
                        .args(["packages/tools/src/wasm-hash.ts", "--variant", variant])
                        .args(&setup)
                        .args(["--at", &at]),
                )?,
            )?;
            if let Some(d) = first_difference("native", &native, variant, &wasm) {
                return Err(format!("{}: native vs wasm {variant}: {d}", suite.name));
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
    let current = current_golden(suite, native, ticks);
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
             `cargo xtask determinism --bless` (or `--bless --suite NAME`) and explain why in \
             the PR.",
            suite.golden
        ));
    }
    println!(
        "== {}: golden matches ({} checkpoints)",
        suite.name,
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
            "ownship": "o", "input": "i", "content": "c", "combat": "k",
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
    fn the_old_suites_keep_their_session_lists_plain_circuit_adds_the_arena_and_combat_the_duel() {
        // The goldens store these joined and compare them strictly (ADR-39 D2: unchanged).
        let s = suites(1_000_000);
        let joined: Vec<String> = s.iter().map(|x| x.session.join(" ")).collect();
        assert_eq!(joined[0], "--spawn air --assist standard --g full");
        assert_eq!(joined[1], "--spawn air --assist manual --g off");
        assert_eq!(joined[2], "--spawn runway --assist standard --g full");
        assert_eq!(
            joined[3],
            "--spawn runway --assist standard --g full --arena plain"
        );
        assert_eq!(
            joined[4],
            "--spawn runway --assist standard --g full --scenario duel --skill 2"
        );
        let names: Vec<_> = s.iter().map(|x| x.name).collect();
        assert_eq!(
            names,
            ["erratic", "flight60", "circuit", "plain-circuit", "combat"]
        );
        let combat = &s[4];
        assert_eq!((combat.at.len(), combat.at[60]), (61, 36_000));
        assert!(combat.at.contains(&combat.debug_probe));
        let plain = &s[3];
        assert_eq!((plain.at.len(), plain.at[12]), (13, 7_200));
        assert!(plain.at.contains(&plain.debug_probe));
    }

    #[test]
    fn the_suite_filter_takes_a_name() {
        let args = |l: &[&str]| l.iter().map(|s| (*s).to_string()).collect::<Vec<_>>();
        let o = parse(&args(&["--bless", "--suite", "plain-circuit"])).unwrap();
        assert_eq!(o.suite.as_deref(), Some("plain-circuit"));
        assert!(parse(&args(&["--suite"])).is_err());
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
