//! The `sim_tile` record (ABI v6, ADR-52): one decoded tile as the host commits it to the core, and as
//! the pipeline keeps a cell's z6 tiles for building the base pack. Little-endian, 37 416 bytes:
//!
//! | Offset | Type | Field |
//! |---|---|---|
//! | 0 | u64 | tile id |
//! | 8 | u32 | flags (`OCEAN`: the rest is ignored, the tile is all zeros) |
//! | 12 / 16 / 20 | f32 | `h_min`, `h_max`, `eps` |
//! | 24 | u8 | quality; 25–31 reserved |
//! | 32 | 16 641 x u16 | posts, row 0 north, **not** delta-coded |
//! | 33 314 | 4 096 x u8 | landcover |
//! | 37 410 | 6 B | padding |

use crate::tile::{LC_COUNT, POST_COUNT};

/// The record's size, bytes. // DATA: ADR-52
pub const SIZE: usize = 37_416;
/// Offset of the tile id.
pub const OFF_ID: usize = 0;
/// Offset of the flags.
pub const OFF_FLAGS: usize = 8;
/// Offset of `h_min`.
pub const OFF_H_MIN: usize = 12;
/// Offset of `h_max`.
pub const OFF_H_MAX: usize = 16;
/// Offset of `eps`.
pub const OFF_EPS: usize = 20;
/// Offset of the quality byte.
pub const OFF_QUALITY: usize = 24;
/// Offset of the posts.
pub const OFF_POSTS: usize = 32;
/// Offset of the landcover.
pub const OFF_LANDCOVER: usize = OFF_POSTS + 2 * POST_COUNT;
/// Flag: the tile is ocean (all posts 0 m, landcover 0).
pub const FLAG_OCEAN: u32 = 1;

const _: () = assert!(OFF_LANDCOVER == 33_314);
const _: () = assert!(OFF_LANDCOVER + LC_COUNT + 6 == SIZE);

/// A tile's fields for [`write`].
#[derive(Clone, Copy, Debug)]
pub struct Record<'a> {
    /// The tile id.
    pub id: u64,
    /// `FLAG_*`.
    pub flags: u32,
    /// Quantisation minimum, m.
    pub h_min: f32,
    /// Quantisation maximum, m.
    pub h_max: f32,
    /// Geometric error, m.
    pub eps: f32,
    /// Quality (`gpk::QUALITY_*`).
    pub quality: u8,
    /// 16 641 posts (ignored for an ocean record).
    pub posts: &'a [u16],
    /// 4 096 landcover cells (ignored for an ocean record).
    pub landcover: &'a [u8],
}

/// Writes `r` into `out[..SIZE]`; an ocean record's posts and landcover are zeros.
pub fn write(out: &mut [u8], r: &Record<'_>) {
    out[..SIZE].fill(0);
    out[OFF_ID..OFF_ID + 8].copy_from_slice(&r.id.to_le_bytes());
    out[OFF_FLAGS..OFF_FLAGS + 4].copy_from_slice(&r.flags.to_le_bytes());
    if r.flags & FLAG_OCEAN != 0 {
        return;
    }
    out[OFF_H_MIN..OFF_H_MIN + 4].copy_from_slice(&r.h_min.to_bits().to_le_bytes());
    out[OFF_H_MAX..OFF_H_MAX + 4].copy_from_slice(&r.h_max.to_bits().to_le_bytes());
    out[OFF_EPS..OFF_EPS + 4].copy_from_slice(&r.eps.to_bits().to_le_bytes());
    out[OFF_QUALITY] = r.quality;
    for (k, q) in r.posts.iter().take(POST_COUNT).enumerate() {
        out[OFF_POSTS + 2 * k..OFF_POSTS + 2 * k + 2].copy_from_slice(&q.to_le_bytes());
    }
    out[OFF_LANDCOVER..OFF_LANDCOVER + LC_COUNT].copy_from_slice(&r.landcover[..LC_COUNT]);
}

/// A read-only view of one record.
#[derive(Clone, Copy, Debug)]
pub struct View<'a>(pub &'a [u8]);

impl View<'_> {
    fn u32_at(&self, o: usize) -> u32 {
        u32::from_le_bytes([self.0[o], self.0[o + 1], self.0[o + 2], self.0[o + 3]])
    }

    /// The tile id.
    #[must_use]
    pub fn id(&self) -> u64 {
        let mut a = [0u8; 8];
        a.copy_from_slice(&self.0[OFF_ID..OFF_ID + 8]);
        u64::from_le_bytes(a)
    }

    /// The flags.
    #[must_use]
    pub fn flags(&self) -> u32 {
        self.u32_at(OFF_FLAGS)
    }

    /// Whether the record is an ocean marker.
    #[must_use]
    pub fn is_ocean(&self) -> bool {
        self.flags() & FLAG_OCEAN != 0
    }

    /// `h_min`, m.
    #[must_use]
    pub fn h_min(&self) -> f32 {
        f32::from_bits(self.u32_at(OFF_H_MIN))
    }

    /// `h_max`, m.
    #[must_use]
    pub fn h_max(&self) -> f32 {
        f32::from_bits(self.u32_at(OFF_H_MAX))
    }

    /// `eps`, m.
    #[must_use]
    pub fn eps(&self) -> f32 {
        f32::from_bits(self.u32_at(OFF_EPS))
    }

    /// The quality byte.
    #[must_use]
    pub fn quality(&self) -> u8 {
        self.0[OFF_QUALITY]
    }

    /// Post `k` (row-major, row 0 north).
    #[must_use]
    pub fn post(&self, k: usize) -> u16 {
        let o = OFF_POSTS + 2 * k;
        u16::from_le_bytes([self.0[o], self.0[o + 1]])
    }

    /// The landcover cells.
    #[must_use]
    pub fn landcover(&self) -> &[u8] {
        &self.0[OFF_LANDCOVER..OFF_LANDCOVER + LC_COUNT]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_record_round_trips() {
        let posts: Vec<u16> = (0..POST_COUNT).map(|k| (k * 7 % 65_536) as u16).collect();
        let lc: Vec<u8> = (0..LC_COUNT)
            .map(|k| if k % 3 == 0 { 0 } else { 255 })
            .collect();
        let mut b = vec![0xAAu8; SIZE];
        write(
            &mut b,
            &Record {
                id: 30_122_163,
                flags: 0,
                h_min: -3.5,
                h_max: 4_000.25,
                eps: 1.5,
                quality: 1,
                posts: &posts,
                landcover: &lc,
            },
        );
        let v = View(&b);
        assert_eq!(v.id(), 30_122_163);
        assert!(!v.is_ocean());
        assert_eq!(
            (v.h_min(), v.h_max(), v.eps(), v.quality()),
            (-3.5, 4_000.25, 1.5, 1)
        );
        assert_eq!(v.post(16_640), posts[16_640]);
        assert_eq!(v.landcover(), &lc[..]);
        assert_eq!(&b[SIZE - 6..], &[0; 6]);
        write(
            &mut b,
            &Record {
                id: 5,
                flags: FLAG_OCEAN,
                h_min: 9.0,
                h_max: 9.0,
                eps: 0.0,
                quality: 0,
                posts: &posts,
                landcover: &lc,
            },
        );
        assert!(View(&b).is_ocean());
        assert!(b[12..].iter().all(|&x| x == 0));
    }
}
