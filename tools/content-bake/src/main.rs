//! Command line for the content baker.
//!
//! ```text
//! content-bake bake <pack-dir> <out-dir>       validate and write <out-dir>/<id>.cpack
//!                                              and <out-dir>/content-manifest.json
//! content-bake export <file.cpack>             print the pack as JSON
//! content-bake schema [--check] <schemas-dir>  write (or verify) the JSON Schemas
//! ```
//!
//! Exit status: 0 success, 1 validation or freshness failure, 2 usage or I/O error.

use std::fs;
use std::path::Path;
use std::process::ExitCode;

use content_bake::{bake, export, schema};

const USAGE: &str = "usage:
  content-bake bake <pack-dir> <out-dir>
  content-bake export <file.cpack>
  content-bake schema [--check] <schemas-dir>";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        ["bake", pack_dir, out_dir] => run_bake(Path::new(pack_dir), Path::new(out_dir)),
        ["export", file] => run_export(Path::new(file)),
        ["schema", "--check", dir] => run_schema_check(Path::new(dir)),
        ["schema", dir] => run_schema_write(Path::new(dir)),
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn run_bake(pack_dir: &Path, out_dir: &Path) -> ExitCode {
    let baked = match bake::bake_dir(pack_dir) {
        Ok(b) => b,
        Err(problems) => {
            for p in &problems {
                eprintln!("{p}");
            }
            eprintln!(
                "\ncontent-bake: {} error(s); nothing was written",
                problems.len()
            );
            return ExitCode::from(1);
        }
    };
    let cpack_name = format!("{}.cpack", baked.pack.id);
    let manifest = bake::manifest(&baked, &cpack_name);
    let manifest_text = serde_json::to_string_pretty(&manifest).expect("manifest is JSON") + "\n";
    let written = fs::create_dir_all(out_dir)
        .and_then(|()| fs::write(out_dir.join(&cpack_name), &baked.cpack))
        .and_then(|()| fs::write(out_dir.join("content-manifest.json"), manifest_text));
    if let Err(e) = written {
        eprintln!("content-bake: cannot write to {}: {e}", out_dir.display());
        return ExitCode::from(2);
    }
    println!(
        "content-bake: {} -> {} ({} bytes, {} aircraft, {} engines, {} aero sets)",
        pack_dir.display(),
        out_dir.join(&cpack_name).display(),
        baked.cpack.len(),
        baked.aircraft.len(),
        baked.engines.len(),
        baked.aero.len()
    );
    ExitCode::SUCCESS
}

fn run_export(file: &Path) -> ExitCode {
    let bytes = match fs::read(file) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("content-bake: cannot read {}: {e}", file.display());
            return ExitCode::from(2);
        }
    };
    match export::export(&bytes) {
        Ok(x) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&x.to_json()).expect("export is JSON")
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("content-bake: {}: {e}", file.display());
            ExitCode::from(1)
        }
    }
}

fn run_schema_write(dir: &Path) -> ExitCode {
    let lint = lint_all();
    if !lint.is_empty() {
        return lint_failed(&lint);
    }
    match schema::write(dir) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("content-bake: {e}");
            ExitCode::from(2)
        }
    }
}

fn run_schema_check(dir: &Path) -> ExitCode {
    let lint = lint_all();
    if !lint.is_empty() {
        return lint_failed(&lint);
    }
    let stale = schema::stale(dir);
    if stale.is_empty() {
        println!("content-bake: schemas in {} are up to date", dir.display());
        ExitCode::SUCCESS
    } else {
        for f in &stale {
            eprintln!("stale: {f}");
        }
        eprintln!(
            "content-bake: run `content-bake schema {}` and commit the result",
            dir.display()
        );
        ExitCode::from(1)
    }
}

fn lint_all() -> Vec<String> {
    schema::all()
        .iter()
        .flat_map(|(name, s)| {
            schema::lint_units(s)
                .into_iter()
                .map(move |p| format!("{name}{p}"))
        })
        .collect()
}

fn lint_failed(problems: &[String]) -> ExitCode {
    for p in problems {
        eprintln!("error: {p}");
    }
    eprintln!(
        "content-bake: rename the key with a unit suffix, or add it to DIMENSIONLESS in schema.rs"
    );
    ExitCode::from(1)
}
