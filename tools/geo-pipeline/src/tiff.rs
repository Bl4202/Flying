//! A minimal `GeoTIFF` reader for exactly what Copernicus DEM uses (ADR-54); anything else is refused
//! with a clear error.
//!
//! - **The container:** little-endian classic TIFF (`II`, 42), first IFD only.
//! - **The samples:** one float32 sample per pixel (`SampleFormat` 3), planar configuration 1.
//! - **Compression:** none (1) or zlib (8).
//! - **Predictor:** none (1), or floating point (3, TIFF Technical Note 3).
//! - **The layout:** tiled or stripped.
//! - **The geo tags:** `ModelPixelScale`, `ModelTiepoint` and the `GeoKeyDirectory`, with
//!   `GTRasterTypeGeoKey` 1 (`PixelIsArea`) or 2 (`PixelIsPoint`) and EPSG 4326.
//!
//! **Voids** (NaN, or below −1 000 m) are filled with the mean of their valid 8-neighbours, over at
//! most 64 passes. What remains is counted.

/// A decoded source raster.
#[derive(Clone, Debug)]
pub struct Raster {
    /// Columns.
    pub width: usize,
    /// Rows.
    pub height: usize,
    /// The longitude of column 0's centre, degrees.
    pub lon0: f64,
    /// The latitude of row 0's centre, degrees.
    pub lat0: f64,
    /// Degrees per column.
    pub sx: f64,
    /// Degrees per row.
    pub sy: f64,
    /// Row-major samples, row 0 north.
    pub data: Vec<f32>,
    /// Voids left after filling.
    pub unfilled_voids: u64,
}

impl Raster {
    /// The sample at (`col`, `row`).
    #[must_use]
    pub fn at(&self, col: usize, row: usize) -> f32 {
        self.data[row * self.width + col]
    }
}

const TAG_WIDTH: u16 = 256;
const TAG_HEIGHT: u16 = 257;
const TAG_BITS: u16 = 258;
const TAG_COMPRESSION: u16 = 259;
const TAG_STRIP_OFFSETS: u16 = 273;
const TAG_SAMPLES: u16 = 277;
const TAG_ROWS_PER_STRIP: u16 = 278;
const TAG_STRIP_COUNTS: u16 = 279;
const TAG_PLANAR: u16 = 284;
const TAG_PREDICTOR: u16 = 317;
const TAG_TILE_WIDTH: u16 = 322;
const TAG_TILE_LENGTH: u16 = 323;
const TAG_TILE_OFFSETS: u16 = 324;
const TAG_TILE_COUNTS: u16 = 325;
const TAG_SAMPLE_FORMAT: u16 = 339;
const TAG_PIXEL_SCALE: u16 = 33_550;
const TAG_TIEPOINT: u16 = 33_922;
const TAG_GEOKEYS: u16 = 34_735;
const KEY_RASTER_TYPE: u64 = 1_025;
const KEY_GEOGRAPHIC_TYPE: u64 = 2_048;
const EPSG_WGS84: u64 = 4_326;

/// Below this a sample is a void, m. // DATA: ADR-54
pub const VOID_BELOW_M: f32 = -1_000.0;
/// The most void-filling passes. // DATA: ADR-54
pub const VOID_PASSES: usize = 64;

struct Ifd<'a> {
    b: &'a [u8],
    entries: Vec<(u16, u16, u32, usize)>, // tag, type, count, offset of the value field
}

fn u16_at(b: &[u8], o: usize) -> Result<u16, String> {
    b.get(o..o + 2)
        .map(|s| u16::from_le_bytes([s[0], s[1]]))
        .ok_or_else(|| "truncated TIFF".to_string())
}

fn u32_at(b: &[u8], o: usize) -> Result<u32, String> {
    b.get(o..o + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
        .ok_or_else(|| "truncated TIFF".to_string())
}

fn f64_at(b: &[u8], o: usize) -> Result<f64, String> {
    let s = b
        .get(o..o + 8)
        .ok_or_else(|| "truncated TIFF".to_string())?;
    let mut a = [0u8; 8];
    a.copy_from_slice(s);
    Ok(f64::from_le_bytes(a))
}

impl Ifd<'_> {
    fn find(&self, tag: u16) -> Option<&(u16, u16, u32, usize)> {
        self.entries.iter().find(|e| e.0 == tag)
    }

    /// Where a value's data is: inline in the 4-byte field if it fits, else at the offset it holds.
    fn data_at(&self, ty: u16, count: u32, field: usize) -> Result<usize, String> {
        let size = match ty {
            1 | 2 => 1,
            3 => 2,
            4 => 4,
            12 => 8,
            t => return Err(format!("unsupported TIFF field type {t}")),
        };
        if size * count as usize <= 4 {
            Ok(field)
        } else {
            Ok(u32_at(self.b, field)? as usize)
        }
    }

    /// The integer values of a SHORT or LONG tag.
    fn ints(&self, tag: u16) -> Result<Option<Vec<u64>>, String> {
        let Some(&(_, ty, count, field)) = self.find(tag) else {
            return Ok(None);
        };
        let at = self.data_at(ty, count, field)?;
        let mut out = Vec::with_capacity(count as usize);
        for k in 0..count as usize {
            out.push(match ty {
                3 => u64::from(u16_at(self.b, at + 2 * k)?),
                4 => u64::from(u32_at(self.b, at + 4 * k)?),
                t => return Err(format!("tag {tag}: type {t} is not an integer")),
            });
        }
        Ok(Some(out))
    }

    fn int(&self, tag: u16, default: Option<u64>) -> Result<u64, String> {
        match self.ints(tag)? {
            Some(v) if !v.is_empty() => Ok(v[0]),
            _ => default.ok_or_else(|| format!("TIFF tag {tag} missing")),
        }
    }

    fn doubles(&self, tag: u16) -> Result<Vec<f64>, String> {
        let &(_, ty, count, field) = self
            .find(tag)
            .ok_or_else(|| format!("GeoTIFF tag {tag} missing"))?;
        if ty != 12 {
            return Err(format!("tag {tag}: not DOUBLE"));
        }
        let at = self.data_at(ty, count, field)?;
        (0..count as usize)
            .map(|k| f64_at(self.b, at + 8 * k))
            .collect()
    }
}

/// Undoes predictor 3 on one row of `n` samples (`row` holds 4 n bytes) and appends the floats.
fn unpredict_row(row: &mut [u8], n: usize, out: &mut Vec<f32>) {
    for k in 1..4 * n {
        row[k] = row[k].wrapping_add(row[k - 1]);
    }
    for i in 0..n {
        out.push(f32::from_be_bytes([
            row[i],
            row[n + i],
            row[2 * n + i],
            row[3 * n + i],
        ]));
    }
}

/// Decodes one TIFF tile or strip of `w` x `h` samples.
fn decode_block(
    raw: &[u8],
    compression: u64,
    predictor: u64,
    w: usize,
    h: usize,
) -> Result<Vec<f32>, String> {
    let mut data = match compression {
        1 => raw.to_vec(),
        8 => {
            miniz_oxide::inflate::decompress_to_vec_zlib(raw).map_err(|e| format!("zlib: {e:?}"))?
        }
        c => return Err(format!("unsupported TIFF compression {c}")),
    };
    if data.len() < 4 * w * h {
        return Err(format!(
            "a block has {} bytes, wants {}",
            data.len(),
            4 * w * h
        ));
    }
    let mut out = Vec::with_capacity(w * h);
    match predictor {
        1 => {
            for k in 0..w * h {
                out.push(f32::from_le_bytes([
                    data[4 * k],
                    data[4 * k + 1],
                    data[4 * k + 2],
                    data[4 * k + 3],
                ]));
            }
        }
        3 => {
            for r in 0..h {
                unpredict_row(&mut data[4 * w * r..4 * w * (r + 1)], w, &mut out);
            }
        }
        p => return Err(format!("unsupported TIFF predictor {p}")),
    }
    Ok(out)
}

/// Parses the IFD at `off`.
fn ifd(b: &[u8], off: usize) -> Result<Ifd<'_>, String> {
    let n = u16_at(b, off)? as usize;
    let mut entries = Vec::with_capacity(n);
    for k in 0..n {
        let e = off + 2 + 12 * k;
        entries.push((u16_at(b, e)?, u16_at(b, e + 2)?, u32_at(b, e + 4)?, e + 8));
    }
    Ok(Ifd { b, entries })
}

/// The raster type (1 area, 2 point) from the `GeoKey` directory, after checking EPSG 4326.
fn raster_type(d: &Ifd<'_>) -> Result<u64, String> {
    let keys = d.ints(TAG_GEOKEYS)?.ok_or("GeoKeyDirectory missing")?;
    let mut kind = 1;
    for k in keys.as_chunks::<4>().0.iter().skip(1) {
        if k[1] != 0 {
            continue; // stored elsewhere: not a key we read
        }
        if k[0] == KEY_RASTER_TYPE {
            kind = k[3];
        }
        if k[0] == KEY_GEOGRAPHIC_TYPE && k[3] != EPSG_WGS84 {
            return Err(format!("geographic type {} is not EPSG 4326", k[3]));
        }
    }
    if kind == 1 || kind == 2 {
        Ok(kind)
    } else {
        Err(format!("raster type {kind}"))
    }
}

/// Decodes a `GeoTIFF` file's first image.
///
/// # Errors
/// A message naming the first unsupported or malformed part.
pub fn decode(b: &[u8]) -> Result<Raster, String> {
    if b.len() < 8 || &b[0..2] != b"II" || u16_at(b, 2)? != 42 {
        return Err("not a little-endian classic TIFF".into());
    }
    let d = ifd(b, u32_at(b, 4)? as usize)?;
    let (w, h) = (
        d.int(TAG_WIDTH, None)? as usize,
        d.int(TAG_HEIGHT, None)? as usize,
    );
    if d.int(TAG_SAMPLES, Some(1))? != 1
        || d.int(TAG_BITS, None)? != 32
        || d.int(TAG_SAMPLE_FORMAT, Some(1))? != 3
        || d.int(TAG_PLANAR, Some(1))? != 1
    {
        return Err("only one float32 sample per pixel is supported".into());
    }
    let compression = d.int(TAG_COMPRESSION, Some(1))?;
    let predictor = d.int(TAG_PREDICTOR, Some(1))?;
    let mut data = vec![0f32; w * h];
    let (bw, bh, offsets, counts) = if let Some(offs) = d.ints(TAG_TILE_OFFSETS)? {
        let counts = d.ints(TAG_TILE_COUNTS)?.ok_or("TileByteCounts missing")?;
        (
            d.int(TAG_TILE_WIDTH, None)? as usize,
            d.int(TAG_TILE_LENGTH, None)? as usize,
            offs,
            counts,
        )
    } else {
        let offs = d
            .ints(TAG_STRIP_OFFSETS)?
            .ok_or("neither tiles nor strips")?;
        let counts = d.ints(TAG_STRIP_COUNTS)?.ok_or("StripByteCounts missing")?;
        (
            w,
            d.int(TAG_ROWS_PER_STRIP, Some(h as u64))? as usize,
            offs,
            counts,
        )
    };
    let across = w.div_ceil(bw);
    let down = h.div_ceil(bh);
    if offsets.len() != across * down || counts.len() != offsets.len() {
        return Err(format!(
            "{} blocks listed, {} expected",
            offsets.len(),
            across * down
        ));
    }
    for (k, (&o, &c)) in offsets.iter().zip(&counts).enumerate() {
        let raw = b
            .get(o as usize..(o + c) as usize)
            .ok_or("a block lies past the end of the file")?;
        let (bx, by) = (k % across, k / across);
        // The last strip may be short; a padded tile holds at least these rows.
        let rows = bh.min(h - by * bh);
        let block = decode_block(raw, compression, predictor, bw, rows)?;
        for r in 0..rows {
            let row = by * bh + r;
            if row >= h {
                break;
            }
            for c in 0..bw {
                let col = bx * bw + c;
                if col < w {
                    data[row * w + col] = block[r * bw + c];
                }
            }
        }
    }
    let scale = d.doubles(TAG_PIXEL_SCALE)?;
    let tie = d.doubles(TAG_TIEPOINT)?;
    if scale.len() < 2 || tie.len() < 6 {
        return Err("bad ModelPixelScale or ModelTiepoint".into());
    }
    let (sx, sy) = (scale[0], scale[1]);
    let half = if raster_type(&d)? == 1 { 0.5 } else { 0.0 };
    let unfilled = fill_voids(&mut data, w, h);
    Ok(Raster {
        width: w,
        height: h,
        lon0: tie[3] - tie[0] * sx + half * sx,
        lat0: tie[4] + tie[1] * sy - half * sy,
        sx,
        sy,
        data,
        unfilled_voids: unfilled,
    })
}

fn is_void(v: f32) -> bool {
    v.is_nan() || v < VOID_BELOW_M
}

/// Fills voids by the mean of their valid 8-neighbours (at most 64 passes); the count left.
fn fill_voids(data: &mut [f32], w: usize, h: usize) -> u64 {
    let mut voids: Vec<usize> = (0..data.len()).filter(|&k| is_void(data[k])).collect();
    for _ in 0..VOID_PASSES {
        if voids.is_empty() {
            break;
        }
        let mut fills = Vec::new();
        for &k in &voids {
            let (r, c) = (k / w, k % w);
            let (mut sum, mut n) = (0.0f64, 0u32);
            for dr in [-1isize, 0, 1] {
                for dc in [-1isize, 0, 1] {
                    let (Some(rr), Some(cc)) = (r.checked_add_signed(dr), c.checked_add_signed(dc))
                    else {
                        continue;
                    };
                    if (dr, dc) != (0, 0) && rr < h && cc < w {
                        let v = data[rr * w + cc];
                        if !is_void(v) {
                            sum += f64::from(v);
                            n += 1;
                        }
                    }
                }
            }
            if n > 0 {
                fills.push((k, (sum / f64::from(n)) as f32));
            }
        }
        if fills.is_empty() {
            break;
        }
        for &(k, v) in &fills {
            data[k] = v;
        }
        voids.retain(|&k| is_void(data[k]));
    }
    voids.len() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voids_fill_from_their_neighbours() {
        let mut d = vec![10.0, 10.0, 10.0, 10.0, f32::NAN, 20.0, 10.0, 10.0, -5_000.0];
        assert_eq!(fill_voids(&mut d, 3, 3), 0);
        assert!((d[4] - 80.0 / 7.0).abs() < 1e-5, "{}", d[4]); // seven valid neighbours
        assert!(!is_void(d[8]));
        let mut all = vec![f32::NAN; 4];
        assert_eq!(fill_voids(&mut all, 2, 2), 4);
    }
}
