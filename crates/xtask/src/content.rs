//! Content pipeline entry points: bake the core pack, regenerate or verify the JSON Schemas.
//!
//! The work is done by the `content-bake` binary; this only fixes the paths so that every caller
//! (developers, `pnpm build`, CI) uses the same ones.

use std::process::Command;

use crate::util::{run, workspace_root};

/// Where the baked packs and the manifest go.
pub const OUT_DIR: &str = "content-out";
const PACK_DIR: &str = "content/core";
const SCHEMA_DIR: &str = "content/schemas";

pub fn run_command(args: &[String]) -> Result<(), String> {
    let mode = match args {
        [] => Mode::Bake,
        [a] if a == "--schemas" => Mode::WriteSchemas,
        [a] if a == "--check" => Mode::CheckSchemas,
        _ => return Err("usage: cargo xtask content [--schemas | --check]".into()),
    };
    let root = workspace_root();
    let bake = |extra: &[&str]| {
        let mut cmd = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
        cmd.current_dir(&root)
            .args(["run", "--quiet", "--package", "content-bake", "--"])
            .args(extra);
        run(&mut cmd)
    };
    match mode {
        Mode::Bake => bake(&["bake", PACK_DIR, OUT_DIR]),
        Mode::WriteSchemas => bake(&["schema", SCHEMA_DIR]),
        Mode::CheckSchemas => bake(&["schema", "--check", SCHEMA_DIR]),
    }
}

enum Mode {
    Bake,
    WriteSchemas,
    CheckSchemas,
}
