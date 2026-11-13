//! The writer and the reader together: a round trip, determinism, and every refusal of the format.

use geo::crc32c::crc32c;
use geo::gpk::{DirEntry, ENTRY_SIZE, GpkError, HEADER_SIZE, Header, LayerRef, TileHeader, align8};
use geo::tile::TileId;
use geo_pack::tile::{zero_landcover, zero_posts};
use geo_pack::writer::EncodedTile;
use geo_pack::{Archive, ArchiveWriter, DecodedTile, HeaderJson};

fn header(cell: Option<(u32, u32)>) -> HeaderJson {
    HeaderJson {
        datapack: "geo-test".into(),
        cell,
        min_level: 6,
        max_level: 8,
        licences: vec!["synthetic".into()],
        sources: vec![],
    }
}

fn tile(z: u8, x: u32, y: u32, seed: u16) -> DecodedTile {
    let mut posts = zero_posts();
    for (k, p) in posts.iter_mut().enumerate() {
        *p = (k as u16).wrapping_mul(seed).wrapping_add(seed);
    }
    let mut landcover = zero_landcover();
    landcover[..2_048].fill(255);
    DecodedTile {
        id: TileId::new(z, x, y).unwrap(),
        h_min: -10.0,
        h_max: 2_000.5,
        eps: 3.25,
        quality: 1,
        posts,
        landcover,
    }
}

fn three() -> Vec<DecodedTile> {
    vec![
        tile(8, 266, 58, 3),
        tile(6, 66, 14, 7),
        tile(7, 133, 29, 11),
    ]
}

fn build(tiles: &[DecodedTile]) -> Vec<u8> {
    let mut w = ArchiveWriter::new(6, 8);
    for t in tiles {
        w.add_tile(t);
    }
    w.finish(&header(Some((33, 7)))).unwrap()
}

#[test]
fn a_round_trip_in_any_insertion_order() {
    let tiles = three();
    let a = build(&tiles);
    let mut rev = tiles.clone();
    rev.reverse();
    assert_eq!(
        build(&rev),
        a,
        "the bytes do not depend on the insertion order"
    );
    let ar = Archive::open(a).unwrap();
    assert_eq!(ar.entries().len(), 3);
    assert!(ar.entries().windows(2).all(|w| w[0].tile_id < w[1].tile_id));
    for t in &tiles {
        assert_eq!(ar.tile(t.id).unwrap().as_ref(), Some(t));
    }
    assert_eq!(ar.tile(TileId::new(8, 0, 0).unwrap()), Ok(None));
    assert_eq!(ar.header_json(), &header(Some((33, 7))));
    assert_eq!(ar.header().min_level, 6);
}

#[test]
fn duplicates_and_wrong_levels_are_refused() {
    let mut w = ArchiveWriter::new(6, 8);
    w.add_tile(&tile(7, 1, 1, 1));
    w.add_tile(&tile(7, 1, 1, 2));
    assert!(w.finish(&header(None)).is_err());
    let mut w = ArchiveWriter::new(6, 8);
    w.add_tile(&tile(9, 1, 1, 1));
    assert!(w.finish(&header(None)).is_err());
}

#[test]
fn the_reader_refuses_a_bad_magic_header_crc_or_blob_crc() {
    let a = build(&three());
    let mut bad = a.clone();
    bad[1] = b'X';
    assert_eq!(Archive::open(bad).unwrap_err(), GpkError::BadMagic);
    let mut bad = a.clone();
    bad[40] ^= 1;
    assert_eq!(Archive::open(bad).unwrap_err(), GpkError::BadHeaderCrc);
    // A flipped byte inside the first tile's height blob: the archive opens, the tile is refused.
    let ar = Archive::open(a.clone()).unwrap();
    let first = ar.entries()[0];
    let at = (ar.header().data_offset + first.layers[0].offset + 5) as usize;
    let mut bad = a;
    bad[at] ^= 0x40;
    let ar = Archive::open(bad).unwrap();
    assert_eq!(
        ar.tile(TileId::from_id(first.tile_id).unwrap()),
        Err(GpkError::BadBlobCrc)
    );
    assert_eq!(Archive::open(vec![0; 50]).unwrap_err(), GpkError::Truncated);
}

/// An archive whose directory lists `order` (indices into the encoded tiles) as given.
fn assemble(order: &[usize], tiles: &[EncodedTile]) -> Vec<u8> {
    let json = header(None).to_text().into_bytes();
    let mut entries = vec![0u8; order.len() * ENTRY_SIZE];
    let mut data = Vec::new();
    for (k, &i) in order.iter().enumerate() {
        let t = &tiles[i];
        let h = LayerRef {
            offset: data.len() as u64,
            length: t.height.len() as u32,
        };
        data.extend_from_slice(&t.height);
        let l = LayerRef {
            offset: data.len() as u64,
            length: t.landcover.len() as u32,
        };
        data.extend_from_slice(&t.landcover);
        DirEntry {
            tile_id: t.id.id(),
            tile: TileHeader { ..t.meta },
            layers: [h, l],
        }
        .write_to(&mut entries[k * ENTRY_SIZE..]);
    }
    let mut dir = miniz_oxide::deflate::compress_to_vec(&entries, 9);
    let crc = crc32c(&dir);
    dir.extend_from_slice(&crc.to_le_bytes());
    let dir_offset = align8((HEADER_SIZE + json.len()) as u64);
    let data_offset = align8(dir_offset + dir.len() as u64);
    let h = Header {
        flags: 0,
        header_json_offset: HEADER_SIZE as u64,
        header_json_len: json.len() as u32,
        layer_count: 2,
        dir_offset,
        dir_blob_len: dir.len() as u32,
        dir_entry_count: order.len() as u32,
        data_offset,
        data_len: data.len() as u64,
        min_level: 6,
        max_level: 8,
        entry_size: ENTRY_SIZE as u16,
    };
    let mut out = h.to_bytes().to_vec();
    out.extend_from_slice(&json);
    out.resize(dir_offset as usize, 0);
    out.extend_from_slice(&dir);
    out.resize(data_offset as usize, 0);
    out.extend_from_slice(&data);
    out
}

#[test]
fn the_reader_refuses_an_unsorted_or_duplicated_directory() {
    let mut enc: Vec<EncodedTile> = three().iter().map(EncodedTile::encode).collect();
    enc.sort_by_key(|t| t.id.id());
    assert!(Archive::open(assemble(&[0, 1, 2], &enc)).is_ok());
    assert_eq!(
        Archive::open(assemble(&[1, 0, 2], &enc)).unwrap_err(),
        GpkError::Unsorted
    );
    assert_eq!(
        Archive::open(assemble(&[0, 1, 1], &enc)).unwrap_err(),
        GpkError::Unsorted
    );
}
