//! The archive reader (ADR-54). It refuses a wrong magic, a version other than 1, a bad header CRC,
//! an entry size other than `24 + 12 L`, a directory that is not strictly ascending, a blob outside
//! the data, a bad blob CRC, or a payload of the wrong length.

use std::path::Path;

use geo::gpk::{
    DirEntry, ENTRY_SIZE, GpkError, HEADER_SIZE, Header, LAYER_HEIGHT, LAYER_LANDCOVER, check_blob,
};
use geo::tile::TileId;

use crate::json::HeaderJson;
use crate::tile::{DecodedTile, decode_heights, decode_landcover};

/// An archive in memory, its header and directory checked.
#[derive(Clone, Debug)]
pub struct Archive {
    bytes: Vec<u8>,
    header: Header,
    json: HeaderJson,
    entries: Vec<DirEntry>,
}

impl Archive {
    /// Checks and indexes an archive's bytes.
    ///
    /// # Errors
    /// The first rule that fails.
    pub fn open(bytes: Vec<u8>) -> Result<Self, GpkError> {
        let head: &[u8; HEADER_SIZE] = bytes
            .get(..HEADER_SIZE)
            .and_then(|s| s.try_into().ok())
            .ok_or(GpkError::Truncated)?;
        let header = Header::parse(head)?;
        let range = |off: u64, len: u64| -> Result<core::ops::Range<usize>, GpkError> {
            let end = off.checked_add(len).ok_or(GpkError::OutOfRange)?;
            if end > bytes.len() as u64 {
                return Err(GpkError::Truncated);
            }
            Ok(off as usize..end as usize)
        };
        let jr = range(header.header_json_offset, u64::from(header.header_json_len))?;
        let text = core::str::from_utf8(&bytes[jr]).map_err(|_| GpkError::BadJson)?;
        let json = HeaderJson::parse(text)?;
        let dr = range(header.dir_offset, u64::from(header.dir_blob_len))?;
        let body = check_blob(&bytes[dr])?;
        let want = header.dir_entry_count as usize * ENTRY_SIZE;
        let raw = miniz_oxide::inflate::decompress_to_vec_with_limit(body, want)
            .map_err(|_| GpkError::BadPayload)?;
        if raw.len() != want {
            return Err(GpkError::BadPayload);
        }
        range(header.data_offset, header.data_len)?;
        let mut entries = Vec::with_capacity(header.dir_entry_count as usize);
        for chunk in raw.as_chunks::<ENTRY_SIZE>().0 {
            let e = DirEntry::parse(chunk)?;
            if entries
                .last()
                .is_some_and(|p: &DirEntry| p.tile_id >= e.tile_id)
            {
                return Err(GpkError::Unsorted);
            }
            for l in &e.layers {
                if l.offset
                    .checked_add(u64::from(l.length))
                    .is_none_or(|end| end > header.data_len)
                {
                    return Err(GpkError::OutOfRange);
                }
            }
            entries.push(e);
        }
        Ok(Self {
            bytes,
            header,
            json,
            entries,
        })
    }

    /// Reads and checks an archive file.
    ///
    /// # Errors
    /// An I/O error as text, or the first rule that fails.
    pub fn from_file(path: &Path) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::open(bytes).map_err(|e| format!("{}: {e:?}", path.display()))
    }

    /// The fixed header.
    #[must_use]
    pub fn header(&self) -> &Header {
        &self.header
    }

    /// The header JSON.
    #[must_use]
    pub fn header_json(&self) -> &HeaderJson {
        &self.json
    }

    /// The directory, ascending by tile id.
    #[must_use]
    pub fn entries(&self) -> &[DirEntry] {
        &self.entries
    }

    /// The archive's bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The directory entry of `id`, if stored.
    #[must_use]
    pub fn entry(&self, id: TileId) -> Option<&DirEntry> {
        let k = id.id();
        self.entries
            .binary_search_by_key(&k, |e| e.tile_id)
            .ok()
            .map(|i| &self.entries[i])
    }

    fn blob(&self, e: &DirEntry, layer: usize) -> &[u8] {
        let l = e.layers[layer];
        let start = (self.header.data_offset + l.offset) as usize;
        &self.bytes[start..start + l.length as usize]
    }

    /// The decoded tile `id`; `Ok(None)` if it is not stored (ocean).
    ///
    /// # Errors
    /// A bad blob.
    pub fn tile(&self, id: TileId) -> Result<Option<DecodedTile>, GpkError> {
        let Some(e) = self.entry(id) else {
            return Ok(None);
        };
        let posts = decode_heights(self.blob(e, LAYER_HEIGHT))?;
        let landcover = decode_landcover(self.blob(e, LAYER_LANDCOVER))?;
        Ok(Some(DecodedTile {
            id,
            h_min: e.tile.h_min,
            h_max: e.tile.h_max,
            eps: e.tile.eps,
            quality: e.tile.quality,
            posts,
            landcover,
        }))
    }
}
