//! Per-tick microbenchmark against a committed baseline (the PS-9 skeleton, GDD 60.9).
//!
//! Times the native core with `fg bench` (the erratic pilot and the baked pack, as the determinism
//! run has) and compares the median nanoseconds per tick with `tests/perf/baseline.json`. Wall-clock
//! numbers depend on the machine, so the verdict is informational unless `--strict` is given, and
//! a baseline recorded on another OS or architecture is never compared at all. `--update` records
//! the measurement as the new baseline.

use std::fs;
use std::process::Command;

use serde_json::{Value, json};

use crate::util::{run_capture, workspace_root};

const BASELINE: &str = "tests/perf/baseline.json";
const CONTENT: &str = "content-out/core.cpack";
const SCRIPT: &str = "erratic";
/// The regression budget of GDD 60.9: more than this slower than the baseline is a regression.
const TOLERANCE_PCT: f64 = 5.0;

struct Options {
    strict: bool,
    update: bool,
    ticks: Option<u64>,
}

fn parse(args: &[String]) -> Result<Options, String> {
    let mut o = Options {
        strict: false,
        update: false,
        ticks: None,
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--strict" => o.strict = true,
            "--update" => o.update = true,
            "--ticks" => {
                let v = it.next().ok_or("--ticks needs a value")?;
                o.ticks = Some(v.parse().map_err(|e| format!("bad --ticks: {e}"))?);
            }
            other => return Err(format!("unknown flag {other}")),
        }
    }
    Ok(o)
}

/// How a measurement compares with its baseline.
#[derive(Debug, PartialEq)]
enum Verdict {
    Within,
    Faster,
    Slower,
}

/// `Slower` if `measured` exceeds `baseline` by more than `tolerance_pct`, `Faster` if it is
/// below by more than that, otherwise `Within`.
fn compare(baseline: f64, measured: f64, tolerance_pct: f64) -> Verdict {
    // A sliver of slack so a change of exactly the budget is within it despite float rounding.
    let slack = 1e-9;
    let change = (measured / baseline - 1.0) * 100.0;
    if change > tolerance_pct + slack {
        Verdict::Slower
    } else if change < -tolerance_pct - slack {
        Verdict::Faster
    } else {
        Verdict::Within
    }
}

/// The median ns per tick from `fg bench --json` output.
fn median_ns(json_line: &str) -> Result<f64, String> {
    let v: Value = serde_json::from_str(json_line.trim())
        .map_err(|e| format!("cannot read `fg bench --json` output: {e}"))?;
    if v["schema"] != "fg-bench/1" {
        return Err(format!("unexpected bench schema {}", v["schema"]));
    }
    v["ns_per_tick_median"]
        .as_f64()
        .filter(|n| n.is_finite() && *n > 0.0)
        .ok_or_else(|| "no usable ns_per_tick_median in the bench output".to_string())
}

fn machine_class() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

pub fn run_command(args: &[String]) -> Result<(), String> {
    let opts = parse(args)?;
    let root = workspace_root();
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());

    println!("== bake content");
    super::content::run_command(&[])?;
    println!("== build native release");
    crate::util::run(Command::new(&cargo).current_dir(&root).args([
        "build",
        "-q",
        "-p",
        "cli",
        "--release",
    ]))?;

    let content = root.join(CONTENT);
    let content = content.to_str().ok_or("non-UTF-8 repository path")?;
    let mut bench = Command::new(root.join("target/release/fg"));
    bench.args(["bench", "--json", "--script", SCRIPT, "--content", content]);
    if let Some(t) = opts.ticks {
        bench.args(["--ticks", &t.to_string()]);
    }
    println!("== fg bench");
    let measured = median_ns(&run_capture(&mut bench)?)?;
    println!("   {measured:.0} ns per tick (median), {}", machine_class());

    let path = root.join(BASELINE);
    let baseline: Option<Value> = fs::read_to_string(&path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok());

    if opts.update {
        let commit = run_capture(Command::new("git").current_dir(&root).args([
            "rev-parse",
            "--short",
            "HEAD",
        ]))
        .unwrap_or_default();
        let doc = json!({
            "schema": "fg-bench-baseline/1",
            "note": "Median ns per tick of `fg bench` (erratic pilot, core pack), on the machine class below. Compared by `cargo xtask bench`; informational unless --strict. Re-record with --update after an intended change, and say why in the commit.",
            "machine": machine_class(),
            "script": SCRIPT,
            "commit": commit.trim(),
            "ns_per_tick_median": (measured * 10.0).round() / 10.0,
            "tolerance_pct": TOLERANCE_PCT,
        });
        let text = serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?;
        fs::write(&path, format!("{text}\n")).map_err(|e| format!("{}: {e}", path.display()))?;
        println!("== baseline updated: {BASELINE}");
        return Ok(());
    }

    let Some(base) = baseline else {
        println!("== no baseline at {BASELINE} (record one with --update); nothing to compare");
        return Ok(());
    };
    let recorded_on = base["machine"].as_str().unwrap_or("");
    if recorded_on != machine_class() {
        println!(
            "== baseline was recorded on {recorded_on}, this is {}: not compared",
            machine_class()
        );
        return Ok(());
    }
    let expected = base["ns_per_tick_median"]
        .as_f64()
        .ok_or("the baseline has no ns_per_tick_median")?;
    let tolerance = base["tolerance_pct"].as_f64().unwrap_or(TOLERANCE_PCT);
    let change = (measured / expected - 1.0) * 100.0;
    match compare(expected, measured, tolerance) {
        Verdict::Within => {
            println!("== within {tolerance}% of the baseline ({expected:.0} ns): {change:+.1}%");
        }
        Verdict::Faster => println!(
            "== faster than the baseline ({expected:.0} ns) by {:.1}%: consider --update",
            -change
        ),
        Verdict::Slower => {
            let msg = format!(
                "slower than the baseline ({expected:.0} ns) by {change:.1}% (budget {tolerance}%)"
            );
            if opts.strict {
                return Err(msg);
            }
            println!("== {msg} (informational; --strict makes it fail)");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_with_the_regression_budget() {
        assert_eq!(compare(100.0, 100.0, 5.0), Verdict::Within);
        assert_eq!(compare(100.0, 105.0, 5.0), Verdict::Within);
        assert_eq!(compare(100.0, 105.1, 5.0), Verdict::Slower);
        assert_eq!(compare(100.0, 95.0, 5.0), Verdict::Within);
        assert_eq!(compare(100.0, 94.9, 5.0), Verdict::Faster);
    }

    #[test]
    fn reads_the_median_from_bench_output() {
        let line = r#"{"schema":"fg-bench/1","ticks":220000,"script":"erratic","content":true,"ns_per_tick_min":2700.0,"ns_per_tick_median":2810.5,"ns_per_tick_max":3000.0}"#;
        assert_eq!(median_ns(line), Ok(2810.5));
        assert_eq!(median_ns(&format!("{line}\n")), Ok(2810.5));
    }

    #[test]
    fn refuses_output_it_does_not_understand() {
        assert!(median_ns("not json").is_err());
        assert!(median_ns(r#"{"schema":"other/1"}"#).is_err());
        assert!(
            median_ns(r#"{"schema":"fg-bench/1","ns_per_tick_median":0}"#).is_err(),
            "a zero median is not a measurement"
        );
        assert!(median_ns(r#"{"schema":"fg-bench/1"}"#).is_err());
    }

    #[test]
    fn parses_flags() {
        let o = parse(&["--strict".into(), "--ticks".into(), "1100".into()]).unwrap();
        assert!(o.strict && !o.update);
        assert_eq!(o.ticks, Some(1100));
        assert!(parse(&["--bogus".into()]).is_err());
        assert!(parse(&["--ticks".into()]).is_err());
    }
}
