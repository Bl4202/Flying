# ADR-54: GeoPack v1, cells and the geo-pipeline

- **Status:** Proposed
- **Owner:** Bl4202
- **Date:** 2026-11-13
- **Affects:** GDD 44.3, 44.11, 45.3, 45.5, 45.9, 46.2, A.15; `crates/geo` (`tile`, `codec`, `gpk`,
  `sim_tile`), `crates/geo-pack`, `tools/geo-pipeline`, `packages/tools/src/geo-fetch*.ts`,
  `crates/xtask` (`geo-fixture`, `gen-abi`)

## Context

v0.2a turns public elevation data into the world's terrain (roadmap 7.1; owner decision D1 of
ADR-51): the whole Earth to z10 from Copernicus DEM GLO-90, and the six pilot regions to z12 from
GLO-30. GDD 45.5 sketches the pipeline and the `.gpk` format:

- zstd blobs;
- one `world.gpk`;
- PMTiles-style two-level directories.

The gates are GP1 to GP7 and LA1 to LA3 of ADR-51. The data facts were measured for the plan:

- Copernicus DEM is tiled GeoTIFF, float32, zlib, predictor 3 and PixelIsPoint.
- Inside a tile the sea is exactly 0.0, and ocean areas have no tile.

## Decision

### The archive: GeoPack v1
- **A 128-byte header:**

  | Offset | Fields |
  |---|---|
  | 0 | magic `GPK1` |
  | 4 | version 1, flags |
  | 8 | the header JSON's offset and length |
  | 20 | the layer count, 2 |
  | 24 | the directory blob's offset, length and entry count |
  | 40 | the data's offset and length |
  | 56 | min and max level |
  | 58 | the entry size, 48 |
  | 120 | the CRC-32C of bytes 0 to 119 |

- **The header JSON** follows the header: UTF-8, keys sorted, no whitespace. It holds `cell`,
  `datapack`, `layers`, `licences`, `max_level`, `min_level`, `scheme`, `sim_level`, `sources` and
  `tile_posts`, and no dates, so a rebuild from the same files is byte-identical.
- **The directory blob** is `deflate_raw(entries) ‖ crc32c`.
  - Entries are sorted by tile id. Each is 48 bytes:
    - `u64` tile id;
    - `f32` `h_min`, `h_max` and ε;
    - `u8` quality, then 3 reserved bytes;
    - per layer, a `u64` offset and a `u32` length (0 means absent).
  - Each region is aligned to 8 bytes.
- **Layer blobs** are `deflate_raw(payload) ‖ crc32c(compressed)`:
  - heights: 129 x 129 `u16` LE, row 0 north, **row-delta coded** (33 282 B);
  - landcover: 64 x 64 `u8` (4 096 B).
- **The reader refuses:** a wrong magic, a version other than 1, a bad header CRC, an entry size
  other than 48, an unsorted or duplicated directory, a blob outside the data, a bad blob CRC, or a
  payload of the wrong length.

### Deviations from GDD 45.5, each for a reason
- **Raw DEFLATE, not zstd.** The browser inflates DEFLATE natively (`DecompressionStream
  ('deflate-raw')`); zstd would need a second WebAssembly module in the Stream Worker. The code is
  `miniz_oxide =0.9.1`, level 9.
- **One archive per z5 tile ("cell", 5.625°), plus `base.gpk` (z3 to z5), not a single
  `world.gpk`:**
  - each object stays far below R2's 5 GiB single upload;
  - a cell can be rebuilt alone;
  - a region download (v0.2d) is one file.

  GDD 45.3's 5° region cell becomes the z5 tile, because tiles cannot be split across cells.
- **One flat directory per archive, not PMTiles' two levels.** A z12 cell has at most 21 845 entries
  (about 1 MB raw before DEFLATE), read once per cell.
- **Implicit ocean.** A tile that has no entry in its cell archive, or any tile of a cell that has
  no archive, is ocean: posts all 0 m, landcover 0. Nothing is stored or fetched for it. This
  replaces GDD 44.11's 4-byte marker. **Every archive is complete to its max level:** each tile of
  the cell at every level is stored or ocean.
- **Coarser levels by decimation, not area averaging.**
  - A parent post equals the child post at the same place, so refinement never moves a shared post.
  - A parent landcover cell is land if any of the four child cells under it is.
- **ε is an upper bound against the finest level:**
  `ε(parent) = max over children (ε(child) + max |child post − bilinear parent surface|)`. It is 0
  at the finest level. (GDD 45.5 says "the next finer level". A bound over every finer level keeps
  the renderer's "stop refining when ε = 0" safe.)
- **Landcover in v0.2a** is only 0, ocean (the source DEM is exactly 0.0 at all four pixels around
  the cell's centre, or the tile is implicit ocean), and 255, land unclassified. WorldCover replaces
  255 in v0.2c.
- **The renderer's roots are the 128 z3 tiles.** z0 to z2 are not built.

### The pipeline (`tools/geo-pipeline`)
- **`sources`:**
  - the Copernicus tile lists and file names (`Copernicus_DSM_COG_{10|30}_{N|S}lat_00_{E|W}lon_00_DEM`);
  - the 1° tiles a cell reads: latitude rows `ceil(s) − 1` to `ceil(n) − 1`, longitude columns
    `floor(w)` to `floor(e)`, so 49 tiles for an inner cell;
  - `cells`, the cells with land, region cells first at level 12.
- **`tiff`:** a minimal GeoTIFF reader for exactly what Copernicus uses:
  - little-endian classic TIFF, first IFD only;
  - float32, zlib or none, predictor 1 or 3;
  - tiled or stripped;
  - PixelIsPoint or PixelIsArea, EPSG 4326.

  Voids (NaN or below −1 000 m) are filled from their 8-neighbours over at most 64 passes; any that
  remain are counted (GP2).
- **`raster`:**
  - All of a cell's source tiles are decoded once, in parallel, and kept for the cell: at most 49
    tiles, 2.5 GB for GLO-30 and 0.3 GB for GLO-90.
  - A region cell takes GLO-30 where its list has the tile, else GLO-90; a world cell takes GLO-90
    only. A tile in neither list is 0 m.
  - Sampling is bilinear between pixel centres. Each 1° tile keeps its own grid; a row's last column
    meets column 0 of the tile to the east, and a tile's last row meets row 0 of the tile below.
  - (The plan named a 12-entry LRU. Eight workers on separate Morton blocks need more than 12 tiles
    at once.)
- **`pyramid`, `cell`:** the finest level is sampled, the coarser ones are decimated, and the work is
  split over 8 threads by contiguous id ranges. Results are kept in id order and the writer sorts
  again, so the bytes never depend on thread timing (GP1). The outputs are:
  - `cells/5-X-Y.gpk`;
  - `z6/5-X-Y.bin` (the four z6 tiles as `sim_tile` records);
  - `qa/5-X-Y.json` (the QA part, with the archive's size, SHA-256, licences and sources).
- **`base`:** `base.gpk` (z3 to z5) is built by decimation from every cell's z6 tiles. Its header
  lists the products and tile-list hashes; each cell's own header names the files.
- **`qa`:** merges the parts into `qa.json`, with tiles, raw and compressed bytes per level, the ε
  histogram, voids, provenance and GDD 45.3's whole-Earth table recomputed. It **fails** if any cell
  has unfilled voids (GP2), or if a level's stored + ocean is not `4^(z − 5)` or its raw bytes are not
  stored x 33 282 (GP4).
- **`audit`:** the licence audit (GDD 45.9, `AC-45.4`, ADR-50). It fails if any of these holds:
  - an archive's licence has no `geo_source/1` descriptor;
  - the descriptor's `terms_checked` is more than 365 days before the build;
  - `commercial_ok`, `cache_allowed` or `redistribution_allowed` is false;
  - an ODbL source has no `dump`.

  On success it writes `licences.json`.
- **`manifest`:** `datapack.json` (`datapack/1`, GDD A.15 extended):
  - `latest`, and the versions newest first;
  - per version: the id, `released`, `compatible_game` `>=0.2.0`, `format` 1, the base file, every
    cell (x, y, `max_level`, file, size, SHA-256, sorted by (y, x)), and the paths of `licences` and
    `qa`;
  - paths `<id>/…` relative to the base URL; `--flat` for files beside `datapack.json` (the
    fixture).
- **`synth`:** the synthetic fixture (ADR-51 section 9.5), with every date fixed (2026-11-12):
  - two islands at 46.55° N 8° E and 53.4° N 180° E, the second across the antimeridian;
  - cells (33, 7) to z12, and (63, 6) and (0, 6) to z10;
  - descriptor `synthetic`;
  - 4.66 MB of archives.
- **`geo-fetch.ts`:**
  - It sends a HEAD for the size and skips a file already present at that size.
  - It GETs to a `.part` file, checks the size, then renames it.
  - Each file gets its first try and up to five retries, after 1, 2, 4, 8 and 16 s.
  - It writes `provenance.json` (URL, bytes, SHA-256, fetch time).
- **`cargo xtask geo-fixture [--repro]`:** builds `target/geo-fixture`. With `--repro` it builds
  twice more and compares every file (GP1).

## As measured in WP3 (2026-11-13)

- **GP1:**
  - two `synth` runs give byte-identical files, 15 of them (`cargo xtask geo-fixture --repro`);
  - the integration test `gp1_two_runs_give_identical_bytes` passes.
- **GP6:**
  - (a) bit-exact round trips, tiled and padded or stripped, zlib or none, predictor 3 or 1;
  - (b) the real GLO-90 N46 E007 gives min 398.0, max 4 462.872 558 593 75, Lake Thun 557.5 and
    Bern 553.630 004 882 812 5, all exactly.
- **GP7:** CRC-32C of "123456789" is `0xE3069283`.
- **LA2:** a missing descriptor, terms 400 days old, and `commercial_ok: false` each fail with their
  message.
- **The real-data smoke** (cell (33, 7) to z10 from 49 GLO-90 tiles, 244 MB fetched in 19 s):
  - 0.92 s and 642 MB peak on the M4;
  - 0 unfilled voids, heights 5.5 to 4 770 m;
  - archive 40.7 MB;
  - bicubic at z10: Bern 554.3 m (553.6 in the source), Lake Thun 557.5 m (exact).
- **Compression is much lower than GDD 45.3 assumed (≈ 3:1):** z10 compresses 1.12:1 in the Alps
  (30.4 MB from 34.1 MB raw). The 16-bit heights over alpine relief carry centimetre-level noise that
  no predictor removes. Measured on the same tiles:
  - byte planes with zigzag deltas: +2 %;
  - a planar (left + up − upleft) predictor: +5 %;
  - the MED (LOCO-I) predictor: +7 %.

  None is worth a format change. The hosted datapack is therefore estimated at **about 19 GB**, not
  the 11 GB of ADR-51's Appendix D. That is about $0.14 a month on R2 (ADR-50's prices), recorded
  in ADR-55.

## Alternatives considered

- *zstd:* about 10 to 20 % smaller [estimate], but a second decoder in the browser.
- *Lossy quantisation (a fixed 0.5 m step):* it would roughly halve the alpine sizes, but it is
  coarser than the source near sea level and changes `AC-45.1`'s meaning. Kept for a later format
  version if hosting cost ever matters.
- *A single `world.gpk` with two-level directories:* larger objects, and no per-cell rebuild.
- *GDAL in the pipeline:* a large native dependency on the runners, for one file format.

## Consequences

- The browser's `geo-stream` (WP6) reads exactly this layout. Its vectors are
  `tests/vectors/gpk_fixture.gpk` and `gpk.json` (GP5).
- A format change needs a new version number and the vectors regenerated.
- The datapack is about 19 GB.
