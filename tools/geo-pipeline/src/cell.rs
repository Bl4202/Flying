//! `build-cell` (ADR-54): one cell's archive, its z6 tiles for the base pack, and its QA part.
//!
//! Outputs under `--out`:
//! - `cells/5-X-Y.gpk`, the archive (z6 to the cell's max level);
//! - `z6/5-X-Y.bin`, the cell's four z6 tiles as `sim_tile` records, ocean flagged;
//! - `qa/5-X-Y.json`, the QA part: counts, sizes, ε histogram, voids, the archive's hash and its
//!   licences and sources.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use geo::sim_tile;
use geo::tile::{CELL_LEVEL, TileId};
use geo_pack::writer::EncodedTile;
use geo_pack::{ArchiveWriter, HeaderJson, SourceFile, SourceSet};
use serde_json::{Value, json};

use crate::pyramid::{self, HeightSource, Level, par_map};
use crate::raster::{Mosaic, SourceStore};
use crate::sources::{Product, TileList, tiles_for_cell};
use crate::{Failure, cell_stem, sha256_hex, write};

/// Worker threads. // DATA: ADR-54, the publish runners have 4 cores; 8 overlaps decoding
pub const DEFAULT_THREADS: usize = 8;
/// The coarsest level of a cell archive. // DATA: ADR-54
pub const MIN_LEVEL: u8 = 6;
/// The descriptor of the Copernicus DEM. // DATA: `content/world/sources/copernicus_dem.json`
pub const COPERNICUS: &str = "copernicus_dem";

/// One `build-cell` run.
#[derive(Clone, Debug)]
pub struct CellJob {
    /// The cell's column.
    pub x: u32,
    /// The cell's row.
    pub y: u32,
    /// The finest level (10 or 12).
    pub max_level: u8,
    /// Where the source files are (`<name>.tif`).
    pub cache: PathBuf,
    /// The datapack id.
    pub datapack: String,
    /// The build date (ISO).
    pub built: String,
    /// The output directory.
    pub out: PathBuf,
    /// The GLO-90 tile list.
    pub list90: PathBuf,
    /// The GLO-30 tile list (region cells).
    pub list30: Option<PathBuf>,
    /// Worker threads.
    pub threads: usize,
}

/// Runs `build-cell`.
///
/// # Errors
/// Bad arguments, a missing or undecodable source, or an I/O failure.
pub fn run(job: &CellJob) -> Result<(), Failure> {
    if !(MIN_LEVEL..=geo::tile::MAX_LEVEL).contains(&job.max_level) {
        return Err(Failure::Usage(format!(
            "--max-level {} is not 6 to 13",
            job.max_level
        )));
    }
    let list90 = TileList::read(&job.list90)?;
    let list30 = job.list30.as_deref().map(TileList::read).transpose()?;
    let mut prefs: Vec<(Product, &TileList)> = Vec::new();
    if let Some(l) = list30.as_ref().filter(|_| job.max_level >= 12) {
        prefs.push((Product::Glo30, l));
    }
    prefs.push((Product::Glo90, &list90));
    let store = SourceStore::load(
        &tiles_for_cell(job.x, job.y),
        &prefs,
        &job.cache,
        job.threads,
    )?;
    let mut sets: BTreeMap<Product, SourceSet> = BTreeMap::new();
    for f in store.files() {
        let list = if f.product == Product::Glo30 {
            list30.as_ref().unwrap_or(&list90)
        } else {
            &list90
        };
        let set = sets.entry(f.product).or_insert_with(|| SourceSet {
            product: f.product.name().into(),
            tile_list_sha256: list.sha256.clone(),
            files: vec![],
        });
        set.files.push(SourceFile {
            name: f.file.clone(),
            sha256: f.sha256.clone(),
            bytes: f.bytes,
        });
    }
    let voids = store.unfilled_voids();
    let src = Mosaic { store };
    let out = CellOut {
        x: job.x,
        y: job.y,
        max_level: job.max_level,
        datapack: &job.datapack,
        built: &job.built,
        licences: vec![COPERNICUS.into()],
        sources: sets.into_values().collect(),
        unfilled_voids: voids,
    };
    let levels = build_levels(&out, &src, job.threads);
    out.write(&job.out, &levels, job.threads)
}

/// Builds the pyramid of a cell.
///
/// # Panics
/// If the cell is out of range.
#[must_use]
pub fn build_levels(c: &CellOut<'_>, src: &dyn HeightSource, threads: usize) -> Vec<Level> {
    let root = TileId::new(CELL_LEVEL, c.x, c.y).expect("a cell");
    pyramid::build(root, MIN_LEVEL, c.max_level, src, threads)
}

/// What a cell's outputs record.
#[derive(Clone, Debug)]
pub struct CellOut<'a> {
    /// The cell's column.
    pub x: u32,
    /// The cell's row.
    pub y: u32,
    /// The finest level.
    pub max_level: u8,
    /// The datapack id.
    pub datapack: &'a str,
    /// The build date.
    pub built: &'a str,
    /// The descriptor ids of the sources.
    pub licences: Vec<String>,
    /// The sources.
    pub sources: Vec<SourceSet>,
    /// Voids left in the sources.
    pub unfilled_voids: u64,
}

/// The ε histogram's upper bounds, m (the last bin is everything above). // DATA: ADR-54
pub const EPS_BINS: [(&str, f32); 6] = [
    ("0", 0.0),
    ("<1", 1.0),
    ("<5", 5.0),
    ("<20", 20.0),
    ("<100", 100.0),
    ("<500", 500.0),
];

/// The ε histogram of some tiles: bins `0`, `<1`, `<5`, `<20`, `<100`, `<500`, `>=500`.
///
/// # Panics
/// Never: every key is one of the bins.
#[must_use]
pub fn eps_hist<'t>(
    tiles: impl Iterator<Item = &'t geo_pack::DecodedTile>,
) -> BTreeMap<String, u64> {
    let mut h: BTreeMap<String, u64> = EPS_BINS
        .iter()
        .map(|(k, _)| ((*k).to_string(), 0))
        .collect();
    h.insert(">=500".into(), 0);
    for t in tiles {
        let key = if t.eps == 0.0 {
            "0"
        } else {
            EPS_BINS[1..]
                .iter()
                .find(|(_, b)| t.eps < *b)
                .map_or(">=500", |(k, _)| k)
        };
        *h.get_mut(key).expect("a bin") += 1;
    }
    h
}

/// Encodes every stored tile of `levels` in parallel, in id order.
#[must_use]
pub fn encode_all(levels: &[Level], threads: usize) -> Vec<EncodedTile> {
    let tiles: Vec<&geo_pack::DecodedTile> = levels.iter().flatten().flatten().collect();
    par_map(tiles.len(), threads, |k| EncodedTile::encode(tiles[k]))
}

/// The per-level counts of a pyramid with their encoded sizes: (z, stored, ocean, compressed bytes).
#[must_use]
pub fn level_rows(min_level: u8, levels: &[Level], encoded: &[EncodedTile]) -> Vec<Value> {
    levels
        .iter()
        .enumerate()
        .map(|(k, l)| {
            let z = min_level + k as u8;
            let stored = l.iter().filter(|t| t.is_some()).count() as u64;
            let compressed: u64 = encoded.iter().filter(|e| e.id.z == z).map(|e| (e.height.len() + e.landcover.len()) as u64).sum();
            json!({ "z": z, "stored": stored, "ocean": l.len() as u64 - stored, "raw_bytes": stored * geo::gpk::HEIGHT_PAYLOAD_SIZE as u64, "compressed_bytes": compressed })
        })
        .collect()
}

/// The (min, max) height over every stored tile's posts.
#[must_use]
pub fn height_range(levels: &[Level]) -> (f64, f64) {
    levels
        .iter()
        .flatten()
        .flatten()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), t| {
            (a.min(f64::from(t.h_min)), b.max(f64::from(t.h_max)))
        })
}

impl CellOut<'_> {
    fn header(&self) -> HeaderJson {
        HeaderJson {
            datapack: self.datapack.into(),
            cell: Some((self.x, self.y)),
            min_level: MIN_LEVEL,
            max_level: self.max_level,
            licences: self.licences.clone(),
            sources: self.sources.clone(),
        }
    }

    /// Writes the archive, the z6 records and the QA part.
    ///
    /// # Errors
    /// An I/O failure, or a writer error.
    ///
    /// # Panics
    /// If the cell is out of range.
    pub fn write(&self, out: &Path, levels: &[Level], threads: usize) -> Result<(), Failure> {
        let stem = cell_stem(self.x, self.y);
        let encoded = encode_all(levels, threads);
        let rows = level_rows(MIN_LEVEL, levels, &encoded);
        let mut w = ArchiveWriter::new(MIN_LEVEL, self.max_level);
        for e in encoded {
            w.add_encoded(e);
        }
        let bytes = w
            .finish(&self.header())
            .map_err(|e| Failure::Check(format!("{stem}: {e:?}")))?;
        let file = format!("cells/{stem}.gpk");
        write(&out.join(&file), &bytes)?;
        let mut z6 = vec![0u8; 4 * sim_tile::SIZE];
        let root = TileId::new(CELL_LEVEL, self.x, self.y).expect("a cell");
        for (k, (child, t)) in root.children().iter().zip(&levels[0]).enumerate() {
            let rec = &mut z6[k * sim_tile::SIZE..(k + 1) * sim_tile::SIZE];
            match t {
                Some(t) => rec.copy_from_slice(&t.to_sim_record()),
                None => sim_tile::write(
                    rec,
                    &sim_tile::Record {
                        id: child.id(),
                        flags: sim_tile::FLAG_OCEAN,
                        h_min: 0.0,
                        h_max: 0.0,
                        eps: 0.0,
                        quality: 0,
                        posts: &[],
                        landcover: &[],
                    },
                ),
            }
        }
        write(&out.join(format!("z6/{stem}.bin")), &z6)?;
        let (lo, hi) = height_range(levels);
        let sources: Vec<Value> = self.sources.iter().map(|s| json!({ "product": s.product, "tile_list_sha256": s.tile_list_sha256, "files": s.files.iter().map(|f| json!({ "name": f.name, "sha256": f.sha256, "bytes": f.bytes })).collect::<Vec<_>>() })).collect();
        let part = json!({
            "schema": "geo_qa_part/1",
            "datapack": self.datapack,
            "built": self.built,
            "cell": [self.x, self.y],
            "max_level": self.max_level,
            "file": file,
            "size_bytes": bytes.len(),
            "sha256": sha256_hex(&bytes),
            "levels": rows,
            "eps_hist": eps_hist(levels.iter().flatten().flatten()),
            "unfilled_voids": self.unfilled_voids,
            "h_min": if lo.is_finite() { json!(lo) } else { Value::Null },
            "h_max": if hi.is_finite() { json!(hi) } else { Value::Null },
            "licences": self.licences,
            "sources": sources,
        });
        let text =
            serde_json::to_string_pretty(&part).map_err(|e| Failure::Io(e.to_string()))? + "\n";
        write(&out.join(format!("qa/{stem}.json")), text.as_bytes())
    }
}
