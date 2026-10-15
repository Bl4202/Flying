//! `fg`: developer command line for the simulation core.
//!
//! ```text
//! fg version
//! fg hash  [--seed HEX] [--script none|erratic|flight60] [--content PATH]
//!          (--ticks N | --at T1,T2,... | --every N --ticks N)
//! fg bench [--seed HEX] [--script none|erratic|flight60] [--content PATH] [--ticks N] [--json]
//! fg conformance --content PATH [--aircraft-index N] [--json PATH] [--traces DIR]
//!                [--only ID,...] [--expect-deferred ID,...]
//! ```
//!
//! `fg hash` prints one JSON object per requested tick with the combined and per-subsystem state
//! hashes as 16-digit hex strings. The determinism check compares this output to the WebAssembly
//! builds (ADR-10). `fg bench` times the tick; its wall-clock numbers are observations about the
//! machine and never feed state. `fg conformance` runs the Performance Conformance Test
//! (ADR-31) and exits 1 when its gate fails.

mod conformance;

use std::io::Write;
use std::process::ExitCode;
use std::time::Instant;

use sim_core::layout::input_script;
use sim_core::{DEFAULT_SEED, SimCore};

const USAGE: &str = "usage:
  fg version
  fg hash  [--seed HEX] [--script none|erratic|flight60] [--content PATH]
//!          (--ticks N | --at T1,T2,... | --every N --ticks N)
  fg bench [--seed HEX] [--script none|erratic|flight60] [--content PATH] [--ticks N] [--json]
  fg conformance --content PATH [--aircraft-index N] [--json PATH] [--traces DIR]
                 [--only ID,...] [--expect-deferred ID,...]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("version") => {
            println!(
                "fg {} {}",
                env!("CARGO_PKG_VERSION"),
                option_env!("GIT_SHA").unwrap_or("unknown")
            );
            Ok(())
        }
        Some("hash") => run_hash(&args[1..]),
        Some("bench") => run_bench(&args[1..]),
        Some("conformance") => {
            let build = format!(
                "fg {} {}",
                env!("CARGO_PKG_VERSION"),
                option_env!("GIT_SHA").unwrap_or("unknown")
            );
            match conformance::run(&args[1..], &build) {
                Ok(true) => Ok(()),
                Ok(false) => return ExitCode::from(1),
                Err(e) => Err(e),
            }
        }
        _ => Err(String::new()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            if !msg.is_empty() {
                eprintln!("fg: {msg}");
            }
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

/// Flags shared by `hash` and `bench`.
struct Setup {
    seed: u64,
    script: u32,
    content: Option<Vec<u8>>,
}

impl Setup {
    /// A started core: content loaded first (only an idle core accepts it), then the script.
    fn core(&self) -> Result<SimCore, String> {
        let mut core = SimCore::new(self.seed);
        if let Some(bytes) = &self.content {
            core.load_content(bytes)
                .map_err(|e| format!("cannot load content: {e}"))?;
        }
        if !core.set_input_script(self.script) {
            return Err(format!("unknown input script {}", self.script));
        }
        core.start();
        Ok(core)
    }
}

/// Command-line arguments split into the shared setup and what is left.
struct Parsed {
    setup: Setup,
    /// Flags this command handles itself, as (name, value).
    rest: Vec<(String, String)>,
    /// Bare switches that were present.
    on: Vec<String>,
}

/// Splits `args` into the shared setup, the other flags (name, value) and bare switches.
fn parse(args: &[String], switches: &[&str]) -> Result<Parsed, String> {
    let mut setup = Setup {
        seed: DEFAULT_SEED,
        script: input_script::NONE as u32,
        content: None,
    };
    let mut rest = Vec::new();
    let mut on = Vec::new();
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        if switches.contains(&flag.as_str()) {
            on.push(flag.clone());
            continue;
        }
        let value = it.next().ok_or_else(|| format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--seed" => {
                let hex = value.trim_start_matches("0x");
                setup.seed =
                    u64::from_str_radix(hex, 16).map_err(|e| format!("bad --seed: {e}"))?;
            }
            "--script" => {
                setup.script = match value.as_str() {
                    "none" => input_script::NONE as u32,
                    "erratic" => input_script::ERRATIC as u32,
                    "flight60" => input_script::FLIGHT60 as u32,
                    other => {
                        return Err(format!(
                            "unknown --script {other:?} (none, erratic or flight60)"
                        ));
                    }
                };
            }
            "--content" => {
                setup.content = Some(
                    std::fs::read(value)
                        .map_err(|e| format!("cannot read --content {value}: {e}"))?,
                );
            }
            _ => rest.push((flag.clone(), value.clone())),
        }
    }
    Ok(Parsed { setup, rest, on })
}

fn run_hash(args: &[String]) -> Result<(), String> {
    let Parsed { setup, rest, .. } = parse(args, &[])?;
    let mut at: Option<Vec<u64>> = None;
    let mut last: Option<u64> = None;
    let mut every: Option<u64> = None;
    for (flag, value) in &rest {
        match flag.as_str() {
            "--ticks" => last = Some(value.parse().map_err(|e| format!("bad --ticks: {e}"))?),
            "--every" => {
                every = Some(value.parse().map_err(|e| format!("bad --every: {e}"))?);
            }
            "--at" => at = Some(parse_ticks(value)?),
            other => return Err(format!("unknown flag {other}")),
        }
    }
    let ticks = match (at, last, every) {
        (Some(at), None, None) => at,
        (None, Some(n), None) => vec![n],
        (None, Some(n), Some(k)) if k > 0 => every_ticks(n, k),
        _ => return Err("use --ticks N, --at T1,T2,... or --every K --ticks N".into()),
    };
    if ticks.windows(2).any(|w| w[0] >= w[1]) {
        return Err("--at ticks must be strictly increasing".into());
    }

    let mut core = setup.core()?;
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    for target in ticks {
        advance_to(&mut core, target);
        let h = core.subsystem_hashes();
        writeln!(
            out,
            "{{\"tick\":{},\"combined\":\"{:016x}\",\"tick_h\":\"{:016x}\",\"rng\":\"{:016x}\",\"bodies\":\"{:016x}\",\"ownship\":\"{:016x}\",\"input\":\"{:016x}\",\"content\":\"{:016x}\"}}",
            core.tick(),
            h.combined,
            h.tick,
            h.rng,
            h.bodies,
            h.ownship,
            h.input,
            h.content
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn advance_to(core: &mut SimCore, target: u64) {
    while core.tick() < target {
        let remaining = target - core.tick();
        let chunk = u32::try_from(remaining.min(1 << 20)).unwrap_or(1 << 20);
        core.step(chunk);
    }
}

/// Ticks in each timed chunk, and how many chunks: the median of the chunks is reported, which
/// ignores the odd interrupted one.
const BENCH_CHUNKS: u64 = 11;

fn run_bench(args: &[String]) -> Result<(), String> {
    let Parsed { setup, rest, on } = parse(args, &["--json"])?;
    let mut ticks = 220_000_u64;
    for (flag, value) in &rest {
        match flag.as_str() {
            "--ticks" => ticks = value.parse().map_err(|e| format!("bad --ticks: {e}"))?,
            other => return Err(format!("unknown flag {other}")),
        }
    }
    let per_chunk = ticks / BENCH_CHUNKS;
    if per_chunk == 0 {
        return Err(format!("--ticks must be at least {BENCH_CHUNKS}"));
    }

    let mut core = setup.core()?;
    // Warm-up: caches, branch predictors and every lazily grown buffer.
    advance_to(&mut core, 2_000);
    let mut ns_per_tick = Vec::new();
    for _ in 0..BENCH_CHUNKS {
        let target = core.tick() + per_chunk;
        let t0 = Instant::now();
        advance_to(&mut core, target);
        ns_per_tick.push(t0.elapsed().as_nanos() as f64 / per_chunk as f64);
    }
    ns_per_tick.sort_by(f64::total_cmp);
    let (min, median, max) = (
        ns_per_tick[0],
        ns_per_tick[ns_per_tick.len() / 2],
        ns_per_tick[ns_per_tick.len() - 1],
    );
    let script = match setup.script as usize {
        input_script::NONE => "none",
        input_script::ERRATIC => "erratic",
        _ => "flight60",
    };
    if on.iter().any(|f| f == "--json") {
        println!(
            "{{\"schema\":\"fg-bench/1\",\"ticks\":{},\"script\":\"{script}\",\"content\":{},\"ns_per_tick_min\":{min:.1},\"ns_per_tick_median\":{median:.1},\"ns_per_tick_max\":{max:.1}}}",
            per_chunk * BENCH_CHUNKS,
            setup.content.is_some()
        );
    } else {
        println!(
            "{} ticks, script {script}: median {median:.0} ns/tick (min {min:.0}, max {max:.0})",
            per_chunk * BENCH_CHUNKS
        );
    }
    Ok(())
}

/// `0, k, 2k, ..., n` (and `n` itself if it is not a multiple of `k`).
fn every_ticks(n: u64, k: u64) -> Vec<u64> {
    let mut v: Vec<u64> = (0..=n / k).map(|i| i * k).collect();
    if !n.is_multiple_of(k) {
        v.push(n);
    }
    v
}

fn parse_ticks(list: &str) -> Result<Vec<u64>, String> {
    list.split(',')
        .map(|t| {
            t.trim()
                .parse::<u64>()
                .map_err(|e| format!("bad tick {t:?}: {e}"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn shared_flags_are_parsed_and_the_rest_returned() {
        let Parsed { setup, rest, on } = parse(
            &args(&[
                "--seed", "0xff", "--script", "erratic", "--ticks", "5", "--json",
            ]),
            &["--json"],
        )
        .unwrap();
        assert_eq!(setup.seed, 0xff);
        assert_eq!(setup.script, input_script::ERRATIC as u32);
        assert_eq!(rest, [("--ticks".to_string(), "5".to_string())]);
        assert_eq!(on, ["--json"]);
    }

    #[test]
    fn bad_flags_are_reported() {
        assert!(parse(&args(&["--seed"]), &[]).is_err());
        assert!(parse(&args(&["--seed", "xyz"]), &[]).is_err());
        assert!(parse(&args(&["--script", "wild"]), &[]).is_err());
        assert!(parse(&args(&["--content", "/no/such/file"]), &[]).is_err());
    }

    #[test]
    fn every_lists_the_checkpoints() {
        assert_eq!(every_ticks(180, 60), [0, 60, 120, 180]);
        assert_eq!(every_ticks(130, 60), [0, 60, 120, 130]);
        assert_eq!(every_ticks(3_600, 60).len(), 61);
    }

    #[test]
    fn the_script_changes_the_input_hash_and_the_observer() {
        let run = |script| {
            let mut setup = parse(&[], &[]).unwrap().setup;
            setup.script = script;
            let mut core = setup.core().unwrap();
            advance_to(&mut core, 600);
            core.subsystem_hashes()
        };
        let quiet = run(input_script::NONE as u32);
        let busy = run(input_script::ERRATIC as u32);
        assert_ne!(quiet.input, busy.input);
        assert_ne!(quiet.ownship, busy.ownship);
        // The probe world does not depend on input.
        assert_eq!((quiet.rng, quiet.bodies), (busy.rng, busy.bodies));
    }
}
