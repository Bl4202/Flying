//! `cargo xtask sortie`: bakes the core pack, builds `fg` in release and flies the duel gates
//! (ADR-45, ADR-48): the bot playtester's matrix (BP1, BP2) over seeds 1 to 50, the missile sweep
//! (MS5) and the bandit's gates (AI1, AI2, AI4, AI5, AI8), on four threads. Fails if a gate fails.
//! The `sortie_report/1` JSON and the bot's CSV traces go to `target/sortie/`. `--no-sweep` skips
//! MS5 for a quicker local run.

use std::process::Command;

use crate::util::{run, workspace_root};

/// The baked pack.
const CONTENT: &str = "content-out/core.cpack";
/// Where the report and traces go.
const OUT_DIR: &str = "target/sortie";

pub fn run_command(args: &[String]) -> Result<(), String> {
    let mut sweep = true;
    for flag in args {
        match flag.as_str() {
            "--no-sweep" => sweep = false,
            other => {
                return Err(format!(
                    "usage: cargo xtask sortie [--no-sweep]\nunknown flag {other}"
                ));
            }
        }
    }
    let root = workspace_root();
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    println!("== bake content");
    super::content::run_command(&[])?;
    println!("== build fg (release)");
    run(Command::new(&cargo)
        .current_dir(&root)
        .args(["build", "-q", "-p", "cli", "--release"]))?;
    let out = root.join(OUT_DIR);
    std::fs::create_dir_all(&out).map_err(|e| format!("cannot create {}: {e}", out.display()))?;
    println!("== fg sortie");
    let mut cmd = Command::new(root.join("target/release/fg"));
    cmd.current_dir(&root)
        .arg("sortie")
        .args(["--content", CONTENT])
        .args([
            "--matrix",
            "bp",
            "--seeds",
            "1-50",
            "--ai-gates",
            "--threads",
            "4",
        ])
        .arg("--json")
        .arg(out.join("report.json"))
        .arg("--traces")
        .arg(out.join("traces"));
    if sweep {
        cmd.arg("--sweep");
    }
    run(&mut cmd).map_err(|e| format!("sortie gate failed: {e}"))?;
    println!("   report: {}", out.join("report.json").display());
    Ok(())
}
