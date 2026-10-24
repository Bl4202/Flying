//! Build, check and determinism orchestration. Run as `cargo xtask <command>`.

mod bench;
mod circuit;
mod conformance;
mod content;
mod content_ids;
mod determinism;
mod genabi;
mod layers;
mod repro;
mod util;
mod wasm;

use std::process::ExitCode;

const USAGE: &str = "usage: cargo xtask <command>\n\ncommands:\n  check-layers   enforce crate dependency layering (AC-61.1)\n  check-content-ids   the simulation names no content ids or names outside tests (AC-12.4)\n  gen-abi [--check]   regenerate (or verify) the TypeScript layout constants and shared test vectors\n  content [--schemas | --check]   bake content/core into content-out/ (or regenerate / verify the JSON Schemas)\n  wasm [single|threads|all]   build, optimise and verify the WASM modules\n  determinism [--ticks N] [--bless | --compare-only] [--skip-wasm]   native vs WASM state hashes + goldens (erratic and flight60)\n  repro   build twice (second from a copy elsewhere) and compare every output\n  bench [--strict] [--update] [--ticks N]   time the native tick against tests/perf/baseline.json\n  conformance [--only ID,...]   fly the Performance Conformance Test (ADR-31) and gate on it\n  circuit [--assist LEVEL] [--no-robustness]   fly the virtual tester's circuit under every assist level (ADR-33) and gate on it";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("check-layers") => layers::check(),
        Some("check-content-ids") => content_ids::check(),
        Some("gen-abi") => genabi::run_command(&args[1..]),
        Some("content") => content::run_command(&args[1..]),
        Some("wasm") => wasm::run_command(&args[1..]),
        Some("repro") => repro::run_command(),
        Some("bench") => bench::run_command(&args[1..]),
        Some("determinism") => determinism::run_command(&args[1..]),
        Some("conformance") => conformance::run_command(&args[1..]),
        Some("circuit") => circuit::run_command(&args[1..]),
        _ => Err(USAGE.to_string()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("{msg}");
            ExitCode::FAILURE
        }
    }
}
