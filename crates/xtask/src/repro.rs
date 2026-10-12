//! Reproducible-build check (GDD 61.3, `AC-61.7`).
//!
//! Builds the WASM modules and the web app twice from scratch: once in place, once from a copy of
//! the source tree at a different absolute path. Every output file must hash identically, which
//! proves the build depends on neither the checkout location nor leftover state.

use std::fs;
use std::path::Path;
use std::process::Command;

use serde_json::Value;

use crate::util::{run, workspace_root};

/// Directories never copied into the second tree.
const SKIP: &[&str] = &[
    "target",
    "node_modules",
    "dist",
    "wasm-out",
    "content-out",
    ".git",
    "test-results",
    "playwright-report",
];

pub fn run_command() -> Result<(), String> {
    let root = workspace_root();
    println!("== build A (in place)");
    let a = build_and_manifest(&root, false)?;

    let copy = std::env::temp_dir().join(format!("fg-repro-{}", std::process::id()));
    if copy.exists() {
        fs::remove_dir_all(&copy).map_err(|e| e.to_string())?;
    }
    println!("== build B (copy at {})", copy.display());
    copy_tree(&root, &copy)?;
    let b = build_and_manifest(&copy, true);
    // Best effort: a leftover temp dir is not worth failing the check over.
    let _ = fs::remove_dir_all(&copy);
    let b = b?;

    if a == b {
        println!(
            "OK: {} output files are byte-identical across both builds",
            count(&a)
        );
        Ok(())
    } else {
        Err(format!("builds differ:\n{}", describe_diff(&a, &b)))
    }
}

fn count(v: &Value) -> usize {
    ["wasm", "content", "dist"]
        .iter()
        .map(|part| v[*part].as_array().map_or(0, Vec::len))
        .sum()
}

fn build_and_manifest(dir: &Path, install: bool) -> Result<Value, String> {
    for d in [
        "target/wasm-single",
        "target/wasm-threads",
        "wasm-out",
        "content-out",
        "dist",
    ] {
        let p = dir.join(d);
        if p.exists() {
            fs::remove_dir_all(&p).map_err(|e| format!("{}: {e}", p.display()))?;
        }
    }
    // Same build id in both trees: it is embedded in the bundle, and a copy has no .git.
    let sh = |cmd: &mut Command| run(cmd.current_dir(dir).env("GIT_SHA", "repro"));
    if install {
        sh(Command::new("pnpm").args(["install", "--frozen-lockfile", "--prefer-offline"]))?;
    }
    sh(Command::new("cargo").args(["xtask", "wasm"]))?;
    sh(Command::new("cargo").args(["xtask", "content"]))?;
    sh(Command::new("pnpm").args(["build:web"]))?;
    let manifest = |target: &str, out: &str| -> Result<Value, String> {
        sh(Command::new("node").args([
            "packages/tools/src/dist-manifest.ts",
            target,
            "--out",
            out,
        ]))?;
        let text = fs::read_to_string(dir.join(out)).map_err(|e| e.to_string())?;
        serde_json::from_str(&text).map_err(|e| e.to_string())
    };
    Ok(serde_json::json!({
        "wasm": manifest("wasm-out", "target/repro-wasm.json")?,
        "content": manifest("content-out", "target/repro-content.json")?,
        "dist": manifest("dist", "target/repro-dist.json")?,
    }))
}

fn describe_diff(a: &Value, b: &Value) -> String {
    let mut lines = Vec::new();
    for part in ["wasm", "content", "dist"] {
        let (xa, xb) = (a[part].as_array(), b[part].as_array());
        let (Some(xa), Some(xb)) = (xa, xb) else {
            continue;
        };
        for e in xa {
            match xb.iter().find(|o| o["path"] == e["path"]) {
                None => lines.push(format!("  only in A: {part}/{}", e["path"])),
                Some(o) if o["sha256"] != e["sha256"] => {
                    lines.push(format!("  differs: {part}/{}", e["path"]));
                }
                Some(_) => {}
            }
        }
        for o in xb {
            if !xa.iter().any(|e| e["path"] == o["path"]) {
                lines.push(format!("  only in B: {part}/{}", o["path"]));
            }
        }
    }
    lines.join("\n")
}

fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(from).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name();
        if SKIP.iter().any(|s| name == *s) {
            continue;
        }
        let src = entry.path();
        let dst = to.join(&name);
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        if kind.is_dir() {
            copy_tree(&src, &dst)?;
        } else if kind.is_file() {
            fs::copy(&src, &dst).map_err(|e| format!("{}: {e}", src.display()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn diff_names_changed_and_missing_files() {
        let a = json!({"wasm": [{"path": "a.wasm", "sha256": "1"}, {"path": "b.wasm", "sha256": "2"}], "dist": []});
        let b = json!({"wasm": [{"path": "a.wasm", "sha256": "X"}, {"path": "c.wasm", "sha256": "3"}], "dist": []});
        let d = describe_diff(&a, &b);
        assert!(d.contains("differs: wasm/\"a.wasm\""), "{d}");
        assert!(d.contains("only in A: wasm/\"b.wasm\""), "{d}");
        assert!(d.contains("only in B: wasm/\"c.wasm\""), "{d}");
    }
}
