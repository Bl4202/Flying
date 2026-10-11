//! `fg`: developer command line for the simulation core.
//!
//! ```text
//! fg version
//! fg hash [--seed HEX] (--ticks N | --at T1,T2,...)
//! ```
//!
//! `fg hash` prints one JSON object per requested tick with the combined and per-subsystem state
//! hashes as 16-digit hex strings. The determinism check compares this output to the WebAssembly
//! builds (ADR-10).

use std::io::Write;
use std::process::ExitCode;

use sim_core::{DEFAULT_SEED, SimCore};

const USAGE: &str = "usage:\n  fg version\n  fg hash [--seed HEX] (--ticks N | --at T1,T2,...)";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("version") => {
            println!(
                "fg {} {}",
                env!("CARGO_PKG_VERSION"),
                option_env!("GIT_SHA").unwrap_or("unknown")
            );
            ExitCode::SUCCESS
        }
        Some("hash") => match run_hash(&args[1..]) {
            Ok(()) => ExitCode::SUCCESS,
            Err(msg) => {
                eprintln!("fg: {msg}\n{USAGE}");
                ExitCode::from(2)
            }
        },
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn run_hash(args: &[String]) -> Result<(), String> {
    let mut seed = DEFAULT_SEED;
    let mut at: Option<Vec<u64>> = None;
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let value = it.next().ok_or_else(|| format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--seed" => {
                let hex = value.trim_start_matches("0x");
                seed = u64::from_str_radix(hex, 16).map_err(|e| format!("bad --seed: {e}"))?;
            }
            "--ticks" => {
                let n: u64 = value.parse().map_err(|e| format!("bad --ticks: {e}"))?;
                at = Some(vec![n]);
            }
            "--at" => at = Some(parse_ticks(value)?),
            other => return Err(format!("unknown flag {other}")),
        }
    }
    let ticks = at.ok_or("one of --ticks or --at is required")?;
    if ticks.windows(2).any(|w| w[0] >= w[1]) {
        return Err("--at ticks must be strictly increasing".into());
    }

    let mut core = SimCore::new(seed);
    core.start();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    for target in ticks {
        while core.tick() < target {
            let remaining = target - core.tick();
            let chunk = u32::try_from(remaining.min(1 << 20)).unwrap_or(1 << 20);
            core.step(chunk);
        }
        let h = core.subsystem_hashes();
        writeln!(
            out,
            "{{\"tick\":{},\"combined\":\"{:016x}\",\"tick_h\":\"{:016x}\",\"rng\":\"{:016x}\",\"bodies\":\"{:016x}\"}}",
            core.tick(),
            h.combined,
            h.tick,
            h.rng,
            h.bodies
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
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
