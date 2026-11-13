//! Command line for the content baker.
//!
//! ```text
//! content-bake bake <pack-dir> <out-dir>       validate and write <out-dir>/<id>.cpack,
//!                                              <out-dir>/<id>.present.json (the HUD, cockpit,
//!                                              model, font and audio files, if the pack has
//!                                              any) and <out-dir>/content-manifest.json
//! content-bake export <file.cpack>             print the pack as JSON
//! content-bake schema [--check] <schemas-dir>  write (or verify) the JSON Schemas
//! content-bake world <world-dir> <out-dir>     validate and write <out-dir>/world.json and add it
//!                                              to <out-dir>/content-manifest.json
//! ```
//!
//! Exit status: 0 success, 1 validation or freshness failure, 2 usage or I/O error.

use std::fs;
use std::path::Path;
use std::process::ExitCode;

use content_bake::{bake, export, present, schema, world};

const USAGE: &str = "usage:
  content-bake bake <pack-dir> <out-dir>
  content-bake export <file.cpack>
  content-bake schema [--check] <schemas-dir>
  content-bake world <world-dir> <out-dir>";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        ["bake", pack_dir, out_dir] => run_bake(Path::new(pack_dir), Path::new(out_dir)),
        ["export", file] => run_export(Path::new(file)),
        ["schema", "--check", dir] => run_schema_check(Path::new(dir)),
        ["schema", dir] => run_schema_write(Path::new(dir)),
        ["world", world_dir, out_dir] => run_world(Path::new(world_dir), Path::new(out_dir)),
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
    let presentation = match present::bake_present_dir(pack_dir, &baked) {
        Ok(p) => p,
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
    let present_name = format!("{}.present.json", baked.pack.id);
    let mut manifest = bake::manifest(&baked, &cpack_name);
    if let Some(p) = &presentation {
        manifest["packs"][0]["presentation"] = present::manifest_entry(p, &present_name);
    }
    let manifest_text = serde_json::to_string_pretty(&manifest).expect("manifest is JSON") + "\n";
    let written = fs::create_dir_all(out_dir)
        .and_then(|()| fs::write(out_dir.join(&cpack_name), &baked.cpack))
        .and_then(|()| match &presentation {
            Some(p) => fs::write(out_dir.join(&present_name), &p.json),
            None => Ok(()),
        })
        .and_then(|()| fs::write(out_dir.join("content-manifest.json"), manifest_text));
    if let Err(e) = written {
        eprintln!("content-bake: cannot write to {}: {e}", out_dir.display());
        return ExitCode::from(2);
    }
    println!(
        "content-bake: {} -> {} ({} bytes, {} aircraft, {} engines, {} aero sets, {} control gains, {} weapons, {} skills)",
        pack_dir.display(),
        out_dir.join(&cpack_name).display(),
        baked.cpack.len(),
        baked.aircraft.len(),
        baked.engines.len(),
        baked.aero.len(),
        baked.fcs.len(),
        baked.weapons.len(),
        baked.skills.len()
    );
    if let Some(p) = &presentation {
        println!(
            "content-bake: presentation -> {} ({} bytes: {} HUD, {} cockpit, {} model, {} audio profile, {} font)",
            out_dir.join(&present_name).display(),
            p.json.len(),
            p.huds.len(),
            p.cockpits.len(),
            p.models.len(),
            p.audio.len(),
            p.fonts.len()
        );
    }
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

fn run_world(world_dir: &Path, out_dir: &Path) -> ExitCode {
    let baked = match world::bake_world_dir(world_dir) {
        Ok(w) => w,
        Err(problems) => {
            for p in &problems {
                eprintln!("{p}");
            }
            eprintln!(
                "\ncontent-bake: {} error(s) in {}; nothing was written",
                problems.len(),
                world_dir.display()
            );
            return ExitCode::from(1);
        }
    };
    let manifest_path = out_dir.join("content-manifest.json");
    let mut manifest: serde_json::Value = match fs::read(&manifest_path) {
        Ok(b) => match serde_json::from_slice(&b) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("content-bake: {}: {e}", manifest_path.display());
                return ExitCode::from(2);
            }
        },
        Err(_) => serde_json::json!({}),
    };
    manifest["world"] = world::manifest_entry(&baked, "world.json");
    let text = serde_json::to_string_pretty(&manifest).expect("manifest is JSON") + "\n";
    let written = fs::create_dir_all(out_dir)
        .and_then(|()| fs::write(out_dir.join("world.json"), &baked.json))
        .and_then(|()| fs::write(&manifest_path, text));
    if let Err(e) = written {
        eprintln!("content-bake: cannot write to {}: {e}", out_dir.display());
        return ExitCode::from(2);
    }
    println!(
        "content-bake: world -> {} ({} bytes: {} regions, {} sources)",
        out_dir.join("world.json").display(),
        baked.json.len(),
        baked.regions,
        baked.sources
    );
    ExitCode::SUCCESS
}
