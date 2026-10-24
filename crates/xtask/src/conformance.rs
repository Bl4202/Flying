//! `cargo xtask conformance`: bakes the core pack, builds `fg` in release, runs the Performance
//! Conformance Test (GDD 11.19, ADR-31) and fails if its gate fails. The JSON report and the CSV
//! traces go to `target/conformance/`.

use std::process::Command;

use crate::util::{run, workspace_root};

/// The baked pack.
const CONTENT: &str = "content-out/core.cpack";
/// Where the report and traces go.
const OUT_DIR: &str = "target/conformance";

pub fn run_command(args: &[String]) -> Result<(), String> {
    let mut only = Vec::new();
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        match flag.as_str() {
            "--only" => only.push(it.next().ok_or("--only needs a value")?.clone()),
            other => {
                return Err(format!(
                    "usage: cargo xtask conformance [--only ID,...]\nunknown flag {other}"
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
    println!("== fg conformance");
    let mut cmd = Command::new(root.join("target/release/fg"));
    cmd.current_dir(&root)
        .arg("conformance")
        .args(["--content", CONTENT])
        .arg("--json")
        .arg(out.join("report.json"))
        .arg("--traces")
        .arg(out.join("traces"));
    for o in &only {
        cmd.args(["--only", o]);
    }
    run(&mut cmd).map_err(|e| format!("conformance gate failed: {e}"))?;
    println!("   report: {}", out.join("report.json").display());
    Ok(())
}
