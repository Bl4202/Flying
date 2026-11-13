//! The `GeoPack` v1 byte layout (ADR-54): the 128-byte header, the directory entries and the layer
//! blobs, with parsing and writing of the fixed-size parts. Decompression is not here: the
//! simulation never reads an archive (`geo-pack` does, offline and in tools; the browser's
//! `geo-stream` does at run time).
//!
//! An archive is: the header; the header JSON at `header_json_offset`; at `dir_offset` the
//! directory blob `deflate_raw(entries) ‖ crc32c(compressed)`; at `data_offset` every tile's layer
//! blobs in id order, each `deflate_raw(payload) ‖ crc32c(compressed)`. Little-endian throughout.

use crate::crc32c::crc32c;

/// The archive's magic. // DATA: ADR-54
pub const MAGIC: [u8; 4] = *b"GPK1";
/// The format version. // DATA: ADR-54
pub const VERSION: u16 = 1;
/// Header size, bytes. // DATA: ADR-54
pub const HEADER_SIZE: usize = 128;
/// Offset of the header's CRC-32C (over bytes 0..120).
pub const HEADER_CRC_OFFSET: usize = 120;

/// The height layer's index. // DATA: ADR-54
pub const LAYER_HEIGHT: usize = 0;
/// The landcover layer's index. // DATA: ADR-54
pub const LAYER_LANDCOVER: usize = 1;
/// Layers in a v1 archive.
pub const LAYER_COUNT: usize = 2;

/// Bytes of an entry before its layer references. // DATA: ADR-54
pub const ENTRY_BASE_SIZE: usize = 24;
/// Bytes of one layer reference (`u64` offset, `u32` length). // DATA: ADR-54
pub const ENTRY_LAYER_SIZE: usize = 12;
/// Bytes of one directory entry, `24 + 12 L`.
pub const ENTRY_SIZE: usize = ENTRY_BASE_SIZE + ENTRY_LAYER_SIZE * LAYER_COUNT;

/// The height payload: 129 x 129 `u16`, row-delta coded, bytes.
pub const HEIGHT_PAYLOAD_SIZE: usize = crate::tile::POST_COUNT * 2;
/// The landcover payload: 64 x 64 `u8`, bytes.
pub const LANDCOVER_PAYLOAD_SIZE: usize = crate::tile::LC_COUNT;
/// The CRC-32C after every compressed blob, bytes.
pub const BLOB_CRC_SIZE: usize = 4;

/// Tile quality: from the source at this level.
pub const QUALITY_SOURCE: u8 = 0;
/// Tile quality: decimated from a finer level.
pub const QUALITY_DERIVED: u8 = 1;
/// Tile quality: synthetic (the test fixture).
pub const QUALITY_SYNTHETIC: u8 = 2;

/// Landcover class: ocean. // DATA: ADR-54
pub const LC_OCEAN: u8 = 0;
/// Landcover class: land, not classified (until `WorldCover` in v0.2c). // DATA: ADR-54
pub const LC_LAND: u8 = 255;

/// Why an archive's bytes were refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GpkError {
    /// Fewer bytes than the structure needs.
    Truncated,
    /// The magic is not `GPK1`.
    BadMagic,
    /// The version is not 1.
    BadVersion,
    /// The header's CRC-32C does not match.
    BadHeaderCrc,
    /// The layer count is not 2.
    BadLayerCount,
    /// `entry_size` is not `24 + 12 L`.
    BadEntrySize,
    /// A blob's CRC-32C does not match.
    BadBlobCrc,
    /// The directory's ids are not strictly ascending.
    Unsorted,
    /// An offset or length points outside its region.
    OutOfRange,
    /// A decompressed payload has the wrong length, or does not inflate.
    BadPayload,
    /// The header JSON is not what the writer writes.
    BadJson,
}

/// The fixed header (Appendix B of the v0.2a plan; ADR-54).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    /// Always 0 in v1.
    pub flags: u16,
    /// Where the header JSON starts (128).
    pub header_json_offset: u64,
    /// Its length, bytes.
    pub header_json_len: u32,
    /// Layers (2).
    pub layer_count: u32,
    /// Where the directory blob starts.
    pub dir_offset: u64,
    /// The directory blob's length including its CRC.
    pub dir_blob_len: u32,
    /// Directory entries.
    pub dir_entry_count: u32,
    /// Where the tile data starts.
    pub data_offset: u64,
    /// The tile data's length.
    pub data_len: u64,
    /// The coarsest level in the archive.
    pub min_level: u8,
    /// The finest level in the archive.
    pub max_level: u8,
    /// Bytes per directory entry (48).
    pub entry_size: u16,
}

fn u16_at(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
fn u32_at(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}
fn u64_at(b: &[u8], o: usize) -> u64 {
    let mut a = [0u8; 8];
    a.copy_from_slice(&b[o..o + 8]);
    u64::from_le_bytes(a)
}
fn f32_at(b: &[u8], o: usize) -> f32 {
    f32::from_bits(u32_at(b, o))
}

impl Header {
    /// Parses and checks the header: magic, version, CRC, layer count and entry size.
    ///
    /// # Errors
    /// The first check that fails.
    pub fn parse(b: &[u8; HEADER_SIZE]) -> Result<Self, GpkError> {
        if b[0..4] != MAGIC {
            return Err(GpkError::BadMagic);
        }
        if u16_at(b, 4) != VERSION {
            return Err(GpkError::BadVersion);
        }
        if u32_at(b, HEADER_CRC_OFFSET) != crc32c(&b[..HEADER_CRC_OFFSET]) {
            return Err(GpkError::BadHeaderCrc);
        }
        let h = Self {
            flags: u16_at(b, 6),
            header_json_offset: u64_at(b, 8),
            header_json_len: u32_at(b, 16),
            layer_count: u32_at(b, 20),
            dir_offset: u64_at(b, 24),
            dir_blob_len: u32_at(b, 32),
            dir_entry_count: u32_at(b, 36),
            data_offset: u64_at(b, 40),
            data_len: u64_at(b, 48),
            min_level: b[56],
            max_level: b[57],
            entry_size: u16_at(b, 58),
        };
        if h.layer_count as usize != LAYER_COUNT {
            return Err(GpkError::BadLayerCount);
        }
        if h.entry_size as usize != ENTRY_SIZE {
            return Err(GpkError::BadEntrySize);
        }
        Ok(h)
    }

    /// The header's 128 bytes, CRC included.
    #[must_use]
    pub fn to_bytes(&self) -> [u8; HEADER_SIZE] {
        let mut b = [0u8; HEADER_SIZE];
        b[0..4].copy_from_slice(&MAGIC);
        b[4..6].copy_from_slice(&VERSION.to_le_bytes());
        b[6..8].copy_from_slice(&self.flags.to_le_bytes());
        b[8..16].copy_from_slice(&self.header_json_offset.to_le_bytes());
        b[16..20].copy_from_slice(&self.header_json_len.to_le_bytes());
        b[20..24].copy_from_slice(&self.layer_count.to_le_bytes());
        b[24..32].copy_from_slice(&self.dir_offset.to_le_bytes());
        b[32..36].copy_from_slice(&self.dir_blob_len.to_le_bytes());
        b[36..40].copy_from_slice(&self.dir_entry_count.to_le_bytes());
        b[40..48].copy_from_slice(&self.data_offset.to_le_bytes());
        b[48..56].copy_from_slice(&self.data_len.to_le_bytes());
        b[56] = self.min_level;
        b[57] = self.max_level;
        b[58..60].copy_from_slice(&self.entry_size.to_le_bytes());
        let crc = crc32c(&b[..HEADER_CRC_OFFSET]);
        b[HEADER_CRC_OFFSET..HEADER_CRC_OFFSET + 4].copy_from_slice(&crc.to_le_bytes());
        b
    }
}

/// A tile's metadata in its directory entry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TileHeader {
    /// Quantisation minimum, m.
    pub h_min: f32,
    /// Quantisation maximum, m.
    pub h_max: f32,
    /// Geometric error against the finer level (0 at the finest), m.
    pub eps: f32,
    /// `QUALITY_*`.
    pub quality: u8,
}

/// Where one layer's blob is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct LayerRef {
    /// Offset from `data_offset`, bytes.
    pub offset: u64,
    /// Length including the CRC, bytes; 0 = the layer is absent.
    pub length: u32,
}

/// One directory entry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DirEntry {
    /// The tile's id.
    pub tile_id: u64,
    /// Its metadata.
    pub tile: TileHeader,
    /// Its layers, in layer order.
    pub layers: [LayerRef; LAYER_COUNT],
}

impl DirEntry {
    /// Parses the 48 bytes of one entry.
    ///
    /// # Errors
    /// [`GpkError::Truncated`] if fewer than 48 bytes.
    pub fn parse(b: &[u8]) -> Result<Self, GpkError> {
        if b.len() < ENTRY_SIZE {
            return Err(GpkError::Truncated);
        }
        let mut layers = [LayerRef::default(); LAYER_COUNT];
        for (k, l) in layers.iter_mut().enumerate() {
            let o = ENTRY_BASE_SIZE + k * ENTRY_LAYER_SIZE;
            *l = LayerRef {
                offset: u64_at(b, o),
                length: u32_at(b, o + 8),
            };
        }
        Ok(Self {
            tile_id: u64_at(b, 0),
            tile: TileHeader {
                h_min: f32_at(b, 8),
                h_max: f32_at(b, 12),
                eps: f32_at(b, 16),
                quality: b[20],
            },
            layers,
        })
    }

    /// Writes the entry's 48 bytes into `out[..48]`.
    pub fn write_to(&self, out: &mut [u8]) {
        out[..ENTRY_SIZE].fill(0);
        out[0..8].copy_from_slice(&self.tile_id.to_le_bytes());
        out[8..12].copy_from_slice(&self.tile.h_min.to_bits().to_le_bytes());
        out[12..16].copy_from_slice(&self.tile.h_max.to_bits().to_le_bytes());
        out[16..20].copy_from_slice(&self.tile.eps.to_bits().to_le_bytes());
        out[20] = self.tile.quality;
        for (k, l) in self.layers.iter().enumerate() {
            let o = ENTRY_BASE_SIZE + k * ENTRY_LAYER_SIZE;
            out[o..o + 8].copy_from_slice(&l.offset.to_le_bytes());
            out[o + 8..o + 12].copy_from_slice(&l.length.to_le_bytes());
        }
    }
}

/// Splits a blob into its compressed bytes after checking its trailing CRC-32C.
///
/// # Errors
/// [`GpkError::Truncated`] if shorter than the CRC, [`GpkError::BadBlobCrc`] on a mismatch.
pub fn check_blob(blob: &[u8]) -> Result<&[u8], GpkError> {
    if blob.len() < BLOB_CRC_SIZE {
        return Err(GpkError::Truncated);
    }
    let (body, tail) = blob.split_at(blob.len() - BLOB_CRC_SIZE);
    if u32_at(tail, 0) != crc32c(body) {
        return Err(GpkError::BadBlobCrc);
    }
    Ok(body)
}

/// `x` rounded up to a multiple of 8.
#[must_use]
pub const fn align8(x: u64) -> u64 {
    (x + 7) & !7
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header() -> Header {
        Header {
            flags: 0,
            header_json_offset: 128,
            header_json_len: 77,
            layer_count: 2,
            dir_offset: align8(128 + 77),
            dir_blob_len: 1_000,
            dir_entry_count: 21,
            data_offset: align8(208 + 1_000),
            data_len: 123_456,
            min_level: 6,
            max_level: 12,
            entry_size: ENTRY_SIZE as u16,
        }
    }

    #[test]
    fn the_layout_constants() {
        assert_eq!(ENTRY_SIZE, 48);
        assert_eq!(HEIGHT_PAYLOAD_SIZE, 33_282);
        assert_eq!(LANDCOVER_PAYLOAD_SIZE, 4_096);
        assert_eq!(align8(205), 208);
        assert_eq!(align8(208), 208);
    }

    #[test]
    fn a_header_round_trips_and_its_checks_bite() {
        let h = header();
        let b = h.to_bytes();
        assert_eq!(Header::parse(&b), Ok(h));
        let mut bad = b;
        bad[0] = b'X';
        assert_eq!(Header::parse(&bad), Err(GpkError::BadMagic));
        let mut bad = b;
        bad[4] = 2;
        assert_eq!(Header::parse(&bad), Err(GpkError::BadVersion));
        let mut bad = b;
        bad[50] ^= 1;
        assert_eq!(Header::parse(&bad), Err(GpkError::BadHeaderCrc));
        let mut h3 = h;
        h3.entry_size = 40;
        assert_eq!(Header::parse(&h3.to_bytes()), Err(GpkError::BadEntrySize));
    }

    #[test]
    fn an_entry_round_trips() {
        let e = DirEntry {
            tile_id: 30_122_163,
            tile: TileHeader {
                h_min: -3.5,
                h_max: 4_462.872_5,
                eps: 0.0,
                quality: QUALITY_SOURCE,
            },
            layers: [
                LayerRef {
                    offset: 0,
                    length: 20_001,
                },
                LayerRef {
                    offset: 20_001,
                    length: 57,
                },
            ],
        };
        let mut b = [0xAAu8; ENTRY_SIZE];
        e.write_to(&mut b);
        assert_eq!(DirEntry::parse(&b), Ok(e));
        assert_eq!(&b[21..24], &[0, 0, 0]);
        assert_eq!(DirEntry::parse(&b[..47]), Err(GpkError::Truncated));
    }

    #[test]
    fn a_blob_crc_is_checked() {
        let mut blob = b"compressed".to_vec();
        blob.extend_from_slice(&crc32c(b"compressed").to_le_bytes());
        assert_eq!(check_blob(&blob), Ok(&b"compressed"[..]));
        blob[0] ^= 1;
        assert_eq!(check_blob(&blob), Err(GpkError::BadBlobCrc));
        assert_eq!(check_blob(&[1, 2]), Err(GpkError::Truncated));
    }
}
