//! A decoded tile and the blob codecs (ADR-54).
//!
//! A layer blob is `deflate_raw(payload) ‖ crc32c(compressed) as u32 LE`. The height payload is
//! 129 x 129 `u16` LE, row 0 north, row-delta coded (33 282 B); the landcover payload is 64 x 64
//! `u8` (4 096 B).

use geo::codec::{delta_decode_rows, delta_encode_rows};
use geo::crc32c::crc32c;
use geo::gpk::{GpkError, HEIGHT_PAYLOAD_SIZE, LANDCOVER_PAYLOAD_SIZE, check_blob};
use geo::sim_tile;
use geo::tile::{LC_COUNT, POST_COUNT, TileId};

/// The DEFLATE level of every blob. // DATA: ADR-54, smallest output
pub const DEFLATE_LEVEL: u8 = 9;

/// One tile, decoded.
#[derive(Clone, Debug, PartialEq)]
pub struct DecodedTile {
    /// The tile.
    pub id: TileId,
    /// Quantisation minimum, m.
    pub h_min: f32,
    /// Quantisation maximum, m.
    pub h_max: f32,
    /// Geometric error against the finest level (an upper bound), m.
    pub eps: f32,
    /// `geo::gpk::QUALITY_*`.
    pub quality: u8,
    /// 129 x 129 codes, row 0 north, not delta-coded.
    pub posts: Box<[u16; POST_COUNT]>,
    /// 64 x 64 classes, row 0 north.
    pub landcover: Box<[u8; LC_COUNT]>,
}

/// A zeroed box of posts.
///
/// # Panics
/// Never: the vector has exactly 16 641 elements.
#[must_use]
pub fn zero_posts() -> Box<[u16; POST_COUNT]> {
    vec![0u16; POST_COUNT]
        .into_boxed_slice()
        .try_into()
        .expect("16 641 posts")
}

/// A zeroed box of landcover.
///
/// # Panics
/// Never: the vector has exactly 4 096 elements.
#[must_use]
pub fn zero_landcover() -> Box<[u8; LC_COUNT]> {
    vec![0u8; LC_COUNT]
        .into_boxed_slice()
        .try_into()
        .expect("4 096 cells")
}

impl DecodedTile {
    /// The height at post `k` (row-major), m.
    #[must_use]
    pub fn height(&self, k: usize) -> f64 {
        geo::codec::dequantise(f64::from(self.posts[k]), self.h_min, self.h_max)
    }

    /// The tile as a `sim_tile` record.
    #[must_use]
    pub fn to_sim_record(&self) -> Vec<u8> {
        let mut out = vec![0u8; sim_tile::SIZE];
        sim_tile::write(
            &mut out,
            &sim_tile::Record {
                id: self.id.id(),
                flags: 0,
                h_min: self.h_min,
                h_max: self.h_max,
                eps: self.eps,
                quality: self.quality,
                posts: &self.posts[..],
                landcover: &self.landcover[..],
            },
        );
        out
    }

    /// A tile from a `sim_tile` record; `None` for an ocean marker or a bad id.
    #[must_use]
    pub fn from_sim_record(b: &[u8]) -> Option<Self> {
        let v = sim_tile::View(b);
        if v.is_ocean() {
            return None;
        }
        let mut posts = zero_posts();
        for (k, p) in posts.iter_mut().enumerate() {
            *p = v.post(k);
        }
        let mut landcover = zero_landcover();
        landcover.copy_from_slice(v.landcover());
        Some(Self {
            id: TileId::from_id(v.id())?,
            h_min: v.h_min(),
            h_max: v.h_max(),
            eps: v.eps(),
            quality: v.quality(),
            posts,
            landcover,
        })
    }
}

fn seal(payload: &[u8]) -> Vec<u8> {
    let mut blob = miniz_oxide::deflate::compress_to_vec(payload, DEFLATE_LEVEL);
    let crc = crc32c(&blob);
    blob.extend_from_slice(&crc.to_le_bytes());
    blob
}

/// The height blob of `posts`: row deltas, LE bytes, DEFLATE, CRC.
#[must_use]
pub fn encode_heights(posts: &[u16; POST_COUNT]) -> Vec<u8> {
    let mut d = zero_posts();
    d.copy_from_slice(&posts[..]);
    delta_encode_rows(&mut d);
    let mut bytes = Vec::with_capacity(HEIGHT_PAYLOAD_SIZE);
    for q in d.iter().copied() {
        bytes.extend_from_slice(&q.to_le_bytes());
    }
    seal(&bytes)
}

/// The landcover blob of `lc`: DEFLATE, CRC.
#[must_use]
pub fn encode_landcover(lc: &[u8; LC_COUNT]) -> Vec<u8> {
    seal(lc)
}

/// A blob's payload: CRC checked, inflated, length checked.
///
/// # Errors
/// `BadBlobCrc`, `Truncated`, or `BadPayload` if it does not inflate to `payload_len` bytes.
pub fn open_blob(blob: &[u8], payload_len: usize) -> Result<Vec<u8>, GpkError> {
    let body = check_blob(blob)?;
    let out = miniz_oxide::inflate::decompress_to_vec_with_limit(body, payload_len)
        .map_err(|_| GpkError::BadPayload)?;
    if out.len() == payload_len {
        Ok(out)
    } else {
        Err(GpkError::BadPayload)
    }
}

/// The posts of a height blob (undeltaed).
///
/// # Errors
/// As [`open_blob`].
pub fn decode_heights(blob: &[u8]) -> Result<Box<[u16; POST_COUNT]>, GpkError> {
    let bytes = open_blob(blob, HEIGHT_PAYLOAD_SIZE)?;
    let mut posts = zero_posts();
    for (k, p) in posts.iter_mut().enumerate() {
        *p = u16::from_le_bytes([bytes[2 * k], bytes[2 * k + 1]]);
    }
    delta_decode_rows(&mut posts);
    Ok(posts)
}

/// The cells of a landcover blob.
///
/// # Errors
/// As [`open_blob`].
pub fn decode_landcover(blob: &[u8]) -> Result<Box<[u8; LC_COUNT]>, GpkError> {
    let bytes = open_blob(blob, LANDCOVER_PAYLOAD_SIZE)?;
    let mut lc = zero_landcover();
    lc.copy_from_slice(&bytes);
    Ok(lc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blobs_round_trip_and_a_flipped_byte_is_caught() {
        let mut posts = zero_posts();
        for (k, p) in posts.iter_mut().enumerate() {
            *p = ((k * 31) % 65_536) as u16;
        }
        let blob = encode_heights(&posts);
        assert_eq!(decode_heights(&blob).unwrap(), posts);
        let mut bad = blob.clone();
        bad[3] ^= 0x10;
        assert_eq!(decode_heights(&bad), Err(GpkError::BadBlobCrc));
        let mut lc = zero_landcover();
        lc[100] = 255;
        assert_eq!(decode_landcover(&encode_landcover(&lc)).unwrap(), lc);
        // A blob that inflates to the wrong length.
        assert_eq!(open_blob(&seal(&[1, 2, 3]), 4), Err(GpkError::BadPayload));
    }
}
