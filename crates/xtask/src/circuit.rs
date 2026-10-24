//! `cargo xtask circuit`: bakes the core pack, builds `fg` in release, flies the virtual tester's
//! take-off, circuit and landing under every assist level (ADR-33) and fails if a gate run fails.
//! The `circuit_report/1` JSON and the CSV traces go to `target/circuit/`.

use std::process::Command;

use crate::util::{run, workspace_root};

/// The baked pack.
const CONTENT: &str = "content-out/core.cpack";
/// Where the report and traces go.
const OUT_DIR: &str = "target/circuit";

pub fn run_command(args: &[String]) -> Result<(), String> {
    let mut assist = String::from("all");
    let mut robustness = true;
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        match flag.as_str() {
            "--assist" => assist.clone_from(it.next().ok_or("--assist needs a value")?),
            "--no-robustness" => robustness = false,
            other => {
                return Err(format!(
                    "usage: cargo xtask circuit [--assist all|full|standard|reduced|manual] [--no-robustness]\nunknown flag {other}"
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
    println!("== fg circuit");
    let mut cmd = Command::new(root.join("target/release/fg"));
    cmd.current_dir(&root)
        .arg("circuit")
        .args(["--content", CONTENT])
        .args(["--assist", &assist])
        .arg("--json")
        .arg(out.join("report.json"))
        .arg("--traces")
        .arg(out.join("traces"));
    if robustness {
        cmd.arg("--robustness");
    }
    run(&mut cmd).map_err(|e| format!("circuit gate failed: {e}"))?;
    println!("   report: {}", out.join("report.json").display());
    Ok(())
}
