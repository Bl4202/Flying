//! `fg conformance`: the Performance Conformance Test (GDD 11.19, ADR-31).
//!
//! Prints the Markdown table, optionally writes the `conformance_report/1` JSON and the 10 Hz CSV
//! traces, and fails (exit 1) when the gate fails.

use std::path::PathBuf;

use content::Registry;

/// Parsed flags.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Args {
    content: PathBuf,
    aircraft_index: usize,
    json: Option<PathBuf>,
    traces: Option<PathBuf>,
    only: Vec<String>,
    expect_deferred: Vec<String>,
}

fn list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Parses `fg conformance` flags.
///
/// # Errors
/// A message for a missing value, an unknown flag or a missing `--content`.
pub fn parse(args: &[String]) -> Result<Args, String> {
    let mut a = Args::default();
    let mut content = None;
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let value = it.next().ok_or_else(|| format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--content" => content = Some(PathBuf::from(value)),
            "--aircraft-index" => {
                a.aircraft_index = value
                    .parse()
                    .map_err(|e| format!("bad --aircraft-index: {e}"))?;
            }
            "--json" => a.json = Some(PathBuf::from(value)),
            "--traces" => a.traces = Some(PathBuf::from(value)),
            "--only" => a.only.extend(list(value)),
            "--expect-deferred" => a.expect_deferred.extend(list(value)),
            other => return Err(format!("unknown flag {other}")),
        }
    }
    a.content = content.ok_or("--content is required")?;
    Ok(a)
}

/// Runs the test. `Ok(true)` when the gate passes.
///
/// # Errors
/// A message when the pack cannot be read or the aircraft cannot be built, or a file cannot be
/// written.
pub fn run(args: &[String], build: &str) -> Result<bool, String> {
    let a = parse(args)?;
    let bytes = std::fs::read(&a.content)
        .map_err(|e| format!("cannot read {}: {e}", a.content.display()))?;
    let registry = Registry::from_cpack(&bytes).map_err(|e| format!("cannot load content: {e}"))?;
    let handle = registry
        .aircraft_handles()
        .nth(a.aircraft_index)
        .ok_or_else(|| format!("the pack has no aircraft number {}", a.aircraft_index))?;
    let options = flight_test::Options {
        only: a.only.clone(),
    };
    let outcome = flight_test::run(&registry, handle, build, &options)
        .map_err(|e| format!("cannot build the aircraft: {e}"))?;
    let report = &outcome.report;
    print!("{}", report.to_markdown(&a.expect_deferred));
    if let Some(path) = &a.json {
        let text = serde_json::to_string_pretty(&report.to_json(&a.expect_deferred))
            .map_err(|e| e.to_string())?;
        std::fs::write(path, text + "\n")
            .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    }
    if let Some(dir) = &a.traces {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
        for t in &outcome.traces {
            let path = dir.join(format!("{}.csv", t.name));
            std::fs::write(&path, t.csv())
                .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        }
    }
    Ok(report.gate_failures(&a.expect_deferred).is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn flags_are_parsed() {
        let a = parse(&args(&[
            "--content",
            "x.cpack",
            "--only",
            "a, b",
            "--only",
            "c",
            "--expect-deferred",
            "to_roll,ldg_roll",
            "--aircraft-index",
            "2",
        ]))
        .unwrap();
        assert_eq!(a.content, PathBuf::from("x.cpack"));
        assert_eq!(a.only, ["a", "b", "c"]);
        assert_eq!(a.expect_deferred, ["to_roll", "ldg_roll"]);
        assert_eq!(a.aircraft_index, 2);
        assert!(parse(&args(&[])).is_err(), "--content is required");
        assert!(parse(&args(&["--content", "x", "--wat", "1"])).is_err());
        assert!(parse(&args(&["--content"])).is_err());
    }
}
