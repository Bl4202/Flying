//! Build, check and determinism orchestration. Run as `cargo xtask <command>`.

mod layers;
mod util;
mod wasm;

use std::process::ExitCode;

const USAGE: &str = "usage: cargo xtask <command>\n\ncommands:\n  check-layers   enforce crate dependency layering (AC-61.1)\n  wasm [single|threads|all]   build, optimise and verify the WASM modules";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("check-layers") => layers::check(),
        Some("wasm") => wasm::run_command(&args[1..]),
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
