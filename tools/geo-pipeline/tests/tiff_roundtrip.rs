//! GP6: the `GeoTIFF` reader. (a) Bit-exact round trips of TIFFs this test writes: float32, zlib or
//! none, predictor 3 or 1, tiled and padded or stripped. (b) The real GLO-90 tile N46 E007 against
//! the values measured for the v0.2a plan (network, ignored by default; `geo-publish.yml` runs it).

use geo_pipeline::tiff::decode;

/// The geo tags' values: pixel scale and tiepoint (DOUBLE), and the `GeoKey` directory (SHORT).
struct Geo {
    sx: f64,
    sy: f64,
    lon: f64,
    lat: f64,
    point: bool,
}

/// Writes a TIFF of `w` x `h` floats from `f(col, row)`.
fn write_tiff(
    w: usize,
    h: usize,
    block: Option<(usize, usize)>,
    zlib: bool,
    predictor3: bool,
    g: &Geo,
    f: impl Fn(usize, usize) -> f32,
) -> Vec<u8> {
    let (bw, bh) = block.unwrap_or((w, 16));
    let (across, down) = (w.div_ceil(bw), h.div_ceil(bh));
    let mut blocks = Vec::new();
    for by in 0..down {
        for bx in 0..across {
            // A tile is padded to its full size; a strip is not.
            let rows = if block.is_some() {
                bh
            } else {
                bh.min(h - by * bh)
            };
            let mut raw = Vec::with_capacity(4 * bw * rows);
            for r in 0..rows {
                let vals: Vec<f32> = (0..bw)
                    .map(|c| {
                        let (col, row) = (bx * bw + c, by * bh + r);
                        if col < w && row < h { f(col, row) } else { 0.0 }
                    })
                    .collect();
                if predictor3 {
                    let mut b = vec![0u8; 4 * bw];
                    for (i, v) in vals.iter().enumerate() {
                        let be = v.to_be_bytes();
                        for k in 0..4 {
                            b[k * bw + i] = be[k];
                        }
                    }
                    for k in (1..4 * bw).rev() {
                        b[k] = b[k].wrapping_sub(b[k - 1]);
                    }
                    raw.extend_from_slice(&b);
                } else {
                    for v in vals {
                        raw.extend_from_slice(&v.to_le_bytes());
                    }
                }
            }
            blocks.push(if zlib {
                miniz_oxide::deflate::compress_to_vec_zlib(&raw, 6)
            } else {
                raw
            });
        }
    }
    assemble(w, h, block, (bw, bh), (zlib, predictor3), g, &blocks)
}

/// Lays out a TIFF: the header, then the blocks, then the arrays, then the IFD.
fn assemble(
    w: usize,
    h: usize,
    block: Option<(usize, usize)>,
    (bw, bh): (usize, usize),
    (zlib, predictor3): (bool, bool),
    g: &Geo,
    blocks: &[Vec<u8>],
) -> Vec<u8> {
    let mut out = vec![b'I', b'I', 42, 0, 0, 0, 0, 0];
    let mut offsets = Vec::new();
    for b in blocks {
        offsets.push(out.len() as u32);
        out.extend_from_slice(b);
    }
    let arr = |out: &mut Vec<u8>, bytes: &[u8]| -> u32 {
        while out.len() % 2 == 1 {
            out.push(0);
        }
        let at = out.len() as u32;
        out.extend_from_slice(bytes);
        at
    };
    let le32 = |v: &[u32]| v.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>();
    let off_at = arr(&mut out, &le32(&offsets));
    let cnt_at = arr(
        &mut out,
        &le32(&blocks.iter().map(|b| b.len() as u32).collect::<Vec<_>>()),
    );
    let scale: Vec<u8> = [g.sx, g.sy, 0.0]
        .iter()
        .flat_map(|x| x.to_le_bytes())
        .collect();
    let tie: Vec<u8> = [0.0, 0.0, 0.0, g.lon, g.lat, 0.0]
        .iter()
        .flat_map(|x| x.to_le_bytes())
        .collect();
    let keys: Vec<u8> = [
        1u16,
        1,
        0,
        2,
        1025,
        0,
        1,
        if g.point { 2 } else { 1 },
        2048,
        0,
        1,
        4326,
    ]
    .iter()
    .flat_map(|x| x.to_le_bytes())
    .collect();
    let scale_at = arr(&mut out, &scale);
    let tie_at = arr(&mut out, &tie);
    let keys_at = arr(&mut out, &keys);
    let n = blocks.len() as u32;
    let inline = |v: u32| v;
    let mut entries: Vec<(u16, u16, u32, u32)> = vec![
        (256, 4, 1, w as u32),
        (257, 4, 1, h as u32),
        (258, 3, 1, 32),
        (259, 3, 1, if zlib { 8 } else { 1 }),
        (277, 3, 1, 1),
        (284, 3, 1, 1),
        (317, 3, 1, if predictor3 { 3 } else { 1 }),
        (339, 3, 1, 3),
        (33_550, 12, 3, scale_at),
        (33_922, 12, 6, tie_at),
        (34_735, 3, 12, keys_at),
    ];
    let first = |at: u32| if n == 1 { inline(offsets[0]) } else { at };
    if block.is_some() {
        entries.extend([
            (322, 3, 1, bw as u32),
            (323, 3, 1, bh as u32),
            (324, 4, n, first(off_at)),
            (
                325,
                4,
                n,
                if n == 1 {
                    blocks[0].len() as u32
                } else {
                    cnt_at
                },
            ),
        ]);
    } else {
        entries.extend([
            (273, 4, n, first(off_at)),
            (278, 4, 1, bh as u32),
            (
                279,
                4,
                n,
                if n == 1 {
                    blocks[0].len() as u32
                } else {
                    cnt_at
                },
            ),
        ]);
    }
    write_ifd(&mut out, entries);
    out
}

/// Appends the IFD of `entries` (sorted by tag) and points the header at it.
fn write_ifd(out: &mut Vec<u8>, mut entries: Vec<(u16, u16, u32, u32)>) {
    entries.sort_by_key(|e| e.0);
    while out.len() % 2 == 1 {
        out.push(0);
    }
    let ifd = out.len() as u32;
    out[4..8].copy_from_slice(&ifd.to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    for (tag, ty, count, value) in entries {
        out.extend_from_slice(&tag.to_le_bytes());
        out.extend_from_slice(&ty.to_le_bytes());
        out.extend_from_slice(&count.to_le_bytes());
        if ty == 3 && count == 1 {
            out.extend_from_slice(&(value as u16).to_le_bytes());
            out.extend_from_slice(&[0, 0]);
        } else {
            out.extend_from_slice(&value.to_le_bytes());
        }
    }
    out.extend_from_slice(&0u32.to_le_bytes());
}

fn value(col: usize, row: usize) -> f32 {
    // Includes negatives, fractions and an exact zero.
    if (col + row).is_multiple_of(97) {
        0.0
    } else {
        (col as f32 * 1.7 - row as f32 * 2.3 + (col * row % 13) as f32 * 0.125) - 50.0
    }
}

#[test]
fn gp6a_round_trips_bit_exactly() {
    let g = Geo {
        sx: 1.0 / 300.0,
        sy: 1.0 / 200.0,
        lon: 7.0,
        lat: 47.0,
        point: true,
    };
    for (block, zlib, p3) in [
        (Some((256, 128)), true, true),
        (Some((64, 64)), false, true),
        (None, true, false),
        (None, false, false),
        (Some((300, 200)), true, true),
    ] {
        let tiff = write_tiff(300, 200, block, zlib, p3, &g, value);
        let r = decode(&tiff).unwrap_or_else(|e| panic!("{block:?} {zlib} {p3}: {e}"));
        assert_eq!((r.width, r.height), (300, 200));
        for row in 0..200 {
            for col in 0..300 {
                assert_eq!(
                    r.at(col, row).to_bits(),
                    value(col, row).to_bits(),
                    "{block:?} {zlib} {p3} at ({col}, {row})"
                );
            }
        }
        assert_eq!((r.lon0, r.lat0), (7.0, 47.0));
        assert_eq!(r.unfilled_voids, 0);
    }
    // PixelIsArea moves the centres by half a pixel.
    let area = Geo { point: false, ..g };
    let r = decode(&write_tiff(300, 200, None, false, false, &area, value)).unwrap();
    assert!(
        (r.lon0 - (7.0 + 0.5 / 300.0)).abs() < 1e-12
            && (r.lat0 - (47.0 - 0.5 / 200.0)).abs() < 1e-12
    );
}

#[test]
fn gp6a_unsupported_files_are_refused() {
    assert!(
        decode(b"MM\0*\0\0\0\x08")
            .unwrap_err()
            .contains("little-endian")
    );
    let g = Geo {
        sx: 0.1,
        sy: 0.1,
        lon: 0.0,
        lat: 0.0,
        point: true,
    };
    let mut t = write_tiff(8, 8, None, false, false, &g, value);
    // Turn SampleFormat 3 into 1 (unsigned integers): refused.
    let pos = t.windows(4).rposition(|w| w == [0x53, 0x01, 3, 0]).unwrap();
    t[pos + 8] = 1;
    assert!(decode(&t).unwrap_err().contains("float32"));
}

/// The pixel nearest (`lat`, `lon`) of a `PixelIsPoint` raster.
fn nearest(r: &geo_pipeline::tiff::Raster, lat: f64, lon: f64) -> f32 {
    let col = ((lon - r.lon0) / r.sx).round() as usize;
    let row = ((r.lat0 - lat) / r.sy).round() as usize;
    r.at(col, row)
}

#[test]
#[ignore = "downloads 5 MB from the Copernicus DEM bucket; run with --ignored"]
fn tiff_glo90_n46e007() {
    let name = "Copernicus_DSM_COG_30_N46_00_E007_00_DEM";
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/geo-cache/test");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join(format!("{name}.tif"));
    if !file.exists() {
        let url = format!("https://copernicus-dem-90m.s3.amazonaws.com/{name}/{name}.tif");
        let ok = std::process::Command::new("curl")
            .args(["-sSfL", "--retry", "5", "-o"])
            .arg(&file)
            .arg(&url)
            .status()
            .unwrap()
            .success();
        assert!(ok, "curl {url} failed");
    }
    let r = decode(&std::fs::read(&file).unwrap()).unwrap();
    assert_eq!((r.width, r.height), (1_200, 1_200));
    assert_eq!((r.lon0, r.lat0), (7.0, 47.0));
    let (mn, mx) = r
        .data
        .iter()
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), &v| {
            (a.min(v), b.max(v))
        });
    assert_eq!(mn, 398.0);
    assert_eq!(mx, 4_462.872_6);
    assert_eq!(f64::from(mx), 4_462.872_558_593_75);
    assert_eq!(nearest(&r, 46.69, 7.72), 557.5, "Lake Thun");
    assert_eq!(
        f64::from(nearest(&r, 46.948, 7.447)),
        553.630_004_882_812_5,
        "Bern"
    );
    assert_eq!(r.unfilled_voids, 0);
}
