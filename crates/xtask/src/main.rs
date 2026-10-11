//! Build, check and determinism orchestration. Run as `cargo xtask <command>`.

mod determinism;
mod genabi;
mod layers;
mod repro;
mod util;
mod wasm;

use std::process::ExitCode;

const USAGE: &str = "usage: cargo xtask <command>\n\ncommands:\n  check-layers   enforce crate dependency layering (AC-61.1)\n  gen-abi [--check]   regenerate (or verify) the TypeScript layout constants and shared test vectors\n  wasm [single|threads|all]   build, optimise and verify the WASM modules\n  determinism [--ticks N] [--bless] [--skip-wasm]   native vs WASM state hashes + golden\n  repro   build twice (second from a copy elsewhere) and compare every output";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("check-layers") => layers::check(),
        Some("gen-abi") => genabi::run_command(&args[1..]),
        Some("wasm") => wasm::run_command(&args[1..]),
        Some("repro") => repro::run_command(),
        Some("determinism") => determinism::run_command(&args[1..]),
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
