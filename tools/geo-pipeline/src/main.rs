//! `geo-pipeline`: the offline geodata pipeline (ADR-54).
//!
//! ```text
//! geo-pipeline sources --tile-list <txt> --product glo90|glo30 --cell X,Y        prints URLs
//! geo-pipeline cells --tile-list90 <txt> --regions <content/world/regions>       prints X,Y,L per cell with land
//! geo-pipeline build-cell --cell X,Y --max-level 10|12 --cache <dir> --datapack <id> --built <date>
//!                         --out <dir> --tile-list90 <txt> [--tile-list30 <txt>] [--threads N]
//!                                                     writes cells/5-X-Y.gpk, z6/5-X-Y.bin, qa/5-X-Y.json
//! geo-pipeline build-base --z6 <dir> --datapack <id> --built <date> --out <dir>    writes base.gpk, qa/base.json
//! geo-pipeline qa --out <dir> [--provenance <json>...]                           writes qa.json
//! geo-pipeline audit --out <dir> --sources <dir> --built <date>                  writes licences.json or fails
//! geo-pipeline manifest --out <dir> --datapack <id> --released <date> [--previous <datapack.json>] [--flat]
//!                                                                                writes datapack.json
//! geo-pipeline synth --out <dir>                                                 the synthetic fixture
//! ```
//!
//! Exit codes: 0 ok, 1 a check failed, 2 usage or I/O.
#![deny(missing_docs)]

use std::path::PathBuf;
use std::process::ExitCode;

use geo_pipeline::{Failure, audit, base, cell, manifest, qa, sources, synth};

/// The value after `--name`, if given.
fn opt(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

/// Every value after each `--name` up to the next flag.
fn many(args: &[String], name: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut take = false;
    for a in args {
        if a.starts_with("--") {
            take = a == name;
        } else if take {
            out.push(a.clone());
        }
    }
    out
}

fn req(args: &[String], name: &str) -> Result<String, Failure> {
    opt(args, name).ok_or_else(|| Failure::Usage(format!("missing {name}")))
}

fn path(args: &[String], name: &str) -> Result<PathBuf, Failure> {
    req(args, name).map(PathBuf::from)
}

fn run(args: &[String]) -> Result<(), Failure> {
    let cmd = args.first().map_or("", String::as_str);
    let rest = &args[1.min(args.len())..];
    match cmd {
        "sources" => {
            let list = sources::TileList::read(&path(rest, "--tile-list")?)?;
            let product = sources::Product::parse(&req(rest, "--product")?)?;
            let (x, y) = sources::parse_cell(&req(rest, "--cell")?)?;
            for url in sources::urls_for_cell(&list, product, x, y) {
                println!("{url}");
            }
            Ok(())
        }
        "cells" => {
            let list = sources::TileList::read(&path(rest, "--tile-list90")?)?;
            let regions = sources::region_cells(&path(rest, "--regions")?)?;
            for (x, y, l) in sources::cells_with_land(&list, &regions) {
                println!("{x},{y},{l}");
            }
            Ok(())
        }
        "build-cell" => {
            let (x, y) = sources::parse_cell(&req(rest, "--cell")?)?;
            let max_level: u8 = req(rest, "--max-level")?
                .parse()
                .map_err(|_| Failure::Usage("--max-level".into()))?;
            let threads = opt(rest, "--threads").map_or(Ok(cell::DEFAULT_THREADS), |t| {
                t.parse().map_err(|_| Failure::Usage("--threads".into()))
            })?;
            let job = cell::CellJob {
                x,
                y,
                max_level,
                cache: path(rest, "--cache")?,
                datapack: req(rest, "--datapack")?,
                built: req(rest, "--built")?,
                out: path(rest, "--out")?,
                list90: path(rest, "--tile-list90")?,
                list30: opt(rest, "--tile-list30").map(PathBuf::from),
                threads,
            };
            cell::run(&job)
        }
        "build-base" => base::run(
            &path(rest, "--z6")?,
            &req(rest, "--datapack")?,
            &req(rest, "--built")?,
            &path(rest, "--out")?,
        ),
        "qa" => qa::run(
            &path(rest, "--out")?,
            &many(rest, "--provenance")
                .into_iter()
                .map(PathBuf::from)
                .collect::<Vec<_>>(),
        ),
        "audit" => audit::run(
            &path(rest, "--out")?,
            &path(rest, "--sources")?,
            &req(rest, "--built")?,
        ),
        "manifest" => manifest::run(
            &path(rest, "--out")?,
            &req(rest, "--datapack")?,
            &req(rest, "--released")?,
            opt(rest, "--previous").map(PathBuf::from).as_deref(),
            rest.iter().any(|a| a == "--flat"),
        ),
        "synth" => synth::run(&path(rest, "--out")?),
        _ => Err(Failure::Usage(format!(
            "unknown command {cmd:?}; see the header of tools/geo-pipeline/src/main.rs"
        ))),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(Failure::Check(m)) => {
            eprintln!("geo-pipeline: check failed: {m}");
            ExitCode::from(1)
        }
        Err(Failure::Usage(m) | Failure::Io(m)) => {
            eprintln!("geo-pipeline: {m}");
            ExitCode::from(2)
        }
    }
}
