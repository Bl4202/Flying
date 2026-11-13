//! The archive writer (ADR-54). Deterministic: the directory and the data are in tile-id order,
//! whatever order the tiles were added in, and every blob is DEFLATE level 9.

use geo::crc32c::crc32c;
use geo::gpk::{
    DirEntry, ENTRY_SIZE, HEADER_SIZE, Header, LAYER_COUNT, LayerRef, TileHeader, align8,
};
use geo::tile::TileId;

use crate::json::HeaderJson;
use crate::tile::{DEFLATE_LEVEL, DecodedTile, encode_heights, encode_landcover};

/// A tile ready to write: its metadata and its two encoded blobs.
#[derive(Clone, Debug)]
pub struct EncodedTile {
    /// The tile.
    pub id: TileId,
    /// Its metadata.
    pub meta: TileHeader,
    /// The height blob.
    pub height: Vec<u8>,
    /// The landcover blob.
    pub landcover: Vec<u8>,
}

impl EncodedTile {
    /// Encodes a tile (the expensive part: two DEFLATE runs).
    #[must_use]
    pub fn encode(t: &DecodedTile) -> Self {
        Self {
            id: t.id,
            meta: TileHeader {
                h_min: t.h_min,
                h_max: t.h_max,
                eps: t.eps,
                quality: t.quality,
            },
            height: encode_heights(&t.posts),
            landcover: encode_landcover(&t.landcover),
        }
    }
}

/// Why an archive could not be written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteError {
    /// The same tile was added twice.
    Duplicate(u64),
    /// A tile outside the archive's levels.
    WrongLevel(u64),
    /// More than `u32::MAX` bytes in one field.
    TooLarge,
}

/// Collects tiles and writes one archive.
#[derive(Clone, Debug)]
pub struct ArchiveWriter {
    min_level: u8,
    max_level: u8,
    tiles: Vec<EncodedTile>,
}

impl ArchiveWriter {
    /// A writer for an archive holding levels `min_level ..= max_level`.
    #[must_use]
    pub fn new(min_level: u8, max_level: u8) -> Self {
        Self {
            min_level,
            max_level,
            tiles: Vec::new(),
        }
    }

    /// Adds a decoded tile (encodes it now).
    pub fn add_tile(&mut self, t: &DecodedTile) {
        self.tiles.push(EncodedTile::encode(t));
    }

    /// Adds an already encoded tile.
    pub fn add_encoded(&mut self, t: EncodedTile) {
        self.tiles.push(t);
    }

    /// The number of tiles added.
    #[must_use]
    pub fn len(&self) -> usize {
        self.tiles.len()
    }

    /// Whether no tile was added.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }

    /// Writes the archive.
    ///
    /// # Errors
    /// A duplicate tile, a tile outside the levels, or a field overflow.
    pub fn finish(mut self, header_json: &HeaderJson) -> Result<Vec<u8>, WriteError> {
        self.tiles.sort_by_key(|t| t.id.id());
        for w in self.tiles.windows(2) {
            if w[0].id == w[1].id {
                return Err(WriteError::Duplicate(w[0].id.id()));
            }
        }
        if let Some(t) = self
            .tiles
            .iter()
            .find(|t| t.id.z < self.min_level || t.id.z > self.max_level)
        {
            return Err(WriteError::WrongLevel(t.id.id()));
        }
        let json = header_json.to_text().into_bytes();
        let mut entries = vec![0u8; self.tiles.len() * ENTRY_SIZE];
        let mut data_len = 0u64;
        for (k, t) in self.tiles.iter().enumerate() {
            let mut layers = [LayerRef::default(); LAYER_COUNT];
            for (l, blob) in [&t.height, &t.landcover].into_iter().enumerate() {
                let length = u32::try_from(blob.len()).map_err(|_| WriteError::TooLarge)?;
                layers[l] = LayerRef {
                    offset: data_len,
                    length,
                };
                data_len += u64::from(length);
            }
            DirEntry {
                tile_id: t.id.id(),
                tile: t.meta,
                layers,
            }
            .write_to(&mut entries[k * ENTRY_SIZE..]);
        }
        let mut dir_blob = miniz_oxide::deflate::compress_to_vec(&entries, DEFLATE_LEVEL);
        let crc = crc32c(&dir_blob);
        dir_blob.extend_from_slice(&crc.to_le_bytes());
        let json_len = u32::try_from(json.len()).map_err(|_| WriteError::TooLarge)?;
        let dir_offset = align8(HEADER_SIZE as u64 + u64::from(json_len));
        let dir_blob_len = u32::try_from(dir_blob.len()).map_err(|_| WriteError::TooLarge)?;
        let data_offset = align8(dir_offset + u64::from(dir_blob_len));
        let header = Header {
            flags: 0,
            header_json_offset: HEADER_SIZE as u64,
            header_json_len: json_len,
            layer_count: LAYER_COUNT as u32,
            dir_offset,
            dir_blob_len,
            dir_entry_count: u32::try_from(self.tiles.len()).map_err(|_| WriteError::TooLarge)?,
            data_offset,
            data_len,
            min_level: self.min_level,
            max_level: self.max_level,
            entry_size: ENTRY_SIZE as u16,
        };
        let mut out = Vec::with_capacity((data_offset + data_len) as usize);
        out.extend_from_slice(&header.to_bytes());
        out.extend_from_slice(&json);
        out.resize(dir_offset as usize, 0);
        out.extend_from_slice(&dir_blob);
        out.resize(data_offset as usize, 0);
        for t in &self.tiles {
            out.extend_from_slice(&t.height);
            out.extend_from_slice(&t.landcover);
        }
        Ok(out)
    }
}
