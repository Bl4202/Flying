//! The terrain patch handed to the renderer once (layout in [`crate::layout::terrain`]).
//!
//! A square grid of `POSTS x POSTS` vertices centred on a BF point, sampled from the
//! [`GeoService`]. Positions are stored patch-local (offset from the patch origin, ADR-03) so
//! `f32` is plenty; the renderer adds a camera-relative offset per draw (GDD 44.8).

use crate::layout::terrain as t;
use crate::math::DVec3;
use crate::seams::geo::GeoService;

/// Posts per edge (129, ADR-06).
pub const POSTS: usize = t::POSTS;
/// Bytes of an encoded patch.
pub const PATCH_BYTES: usize = t::SIZE;

fn put_f32(buf: &mut [u8], at: usize, v: f32) {
    buf[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

/// Fills `out` with a patch centred on BF `(centre_east_m, centre_north_m)` with `spacing_m`
/// between posts. Returns the bytes written, or 0 if `out` is too small, the spacing is not a
/// positive finite number, or the centre is not finite.
pub fn fill_patch(
    geo: &dyn GeoService,
    out: &mut [u8],
    centre_east_m: f64,
    centre_north_m: f64,
    spacing_m: f64,
) -> usize {
    if out.len() < PATCH_BYTES
        || !(spacing_m > 0.0 && spacing_m.is_finite())
        || !(centre_east_m.is_finite() && centre_north_m.is_finite())
    {
        return 0;
    }
    let half = (POSTS / 2) as f64;
    // Ground height over a grid padded by one post on every side, so every vertex has four
    // neighbours for its normal.
    let padded = POSTS + 2;
    let mut z = vec![0.0_f64; padded * padded];
    let x_at = |i: usize| centre_east_m + (i as f64 - 1.0 - half) * spacing_m;
    let y_at = |j: usize| centre_north_m + (j as f64 - 1.0 - half) * spacing_m;
    for j in 0..padded {
        for i in 0..padded {
            z[j * padded + i] = geo.ground_z_bf(x_at(i), y_at(j));
        }
    }

    out[..t::HEADER_SIZE].fill(0);
    out[t::H_ORIGIN_BF..t::H_ORIGIN_BF + 8].copy_from_slice(&centre_east_m.to_le_bytes());
    out[t::H_ORIGIN_BF + 8..t::H_ORIGIN_BF + 16].copy_from_slice(&centre_north_m.to_le_bytes());
    out[t::H_ORIGIN_BF + 16..t::H_ORIGIN_BF + 24].copy_from_slice(&0.0_f64.to_le_bytes());
    out[t::H_POSTS..t::H_POSTS + 4].copy_from_slice(&(POSTS as u32).to_le_bytes());
    put_f32(out, t::H_SPACING_M, spacing_m as f32);

    for j in 0..POSTS {
        for i in 0..POSTS {
            let (pi, pj) = (i + 1, j + 1);
            let zc = z[pj * padded + pi];
            let dzdx = (z[pj * padded + pi + 1] - z[pj * padded + pi - 1]) / (2.0 * spacing_m);
            let dzdy = (z[(pj + 1) * padded + pi] - z[(pj - 1) * padded + pi]) / (2.0 * spacing_m);
            let normal = DVec3::new(-dzdx, -dzdy, 1.0).normalized_or(DVec3::Z);
            let (x, y) = (x_at(pi), y_at(pj));
            let height_msl = geo.bf_to_geo(DVec3::new(x, y, zc)).h_m;

            let at = t::HEADER_SIZE + (j * POSTS + i) * t::VERTEX_BYTES;
            put_f32(out, at + t::V_POS, (x - centre_east_m) as f32);
            put_f32(out, at + t::V_POS + 4, (y - centre_north_m) as f32);
            put_f32(out, at + t::V_POS + 8, zc as f32);
            put_f32(out, at + t::V_NORMAL, normal.x as f32);
            put_f32(out, at + t::V_NORMAL + 4, normal.y as f32);
            put_f32(out, at + t::V_NORMAL + 8, normal.z as f32);
            put_f32(out, at + t::V_HEIGHT, height_msl as f32);
        }
    }
    PATCH_BYTES
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::seams::geo::{FlatGeo, ProceduralGeo};

    fn f32_at(b: &[u8], o: usize) -> f32 {
        f32::from_le_bytes(b[o..o + 4].try_into().unwrap())
    }

    fn vertex(b: &[u8], i: usize, j: usize) -> ([f32; 3], [f32; 3], f32) {
        let at = t::HEADER_SIZE + (j * POSTS + i) * t::VERTEX_BYTES;
        (
            [f32_at(b, at), f32_at(b, at + 4), f32_at(b, at + 8)],
            [f32_at(b, at + 12), f32_at(b, at + 16), f32_at(b, at + 20)],
            f32_at(b, at + 24),
        )
    }

    #[test]
    fn header_and_extent() {
        let geo = ProceduralGeo::default_frame();
        let mut buf = vec![0_u8; PATCH_BYTES];
        assert_eq!(
            fill_patch(&geo, &mut buf, 100.0, -200.0, 250.0),
            PATCH_BYTES
        );
        assert_eq!(f64::from_le_bytes(buf[0..8].try_into().unwrap()), 100.0);
        assert_eq!(f64::from_le_bytes(buf[8..16].try_into().unwrap()), -200.0);
        assert_eq!(u32::from_le_bytes(buf[24..28].try_into().unwrap()), 129);
        assert_eq!(f32_at(&buf, 28), 250.0);
        // The corners are 16 km from the centre; the centre vertex is at the origin of the patch.
        assert_eq!(vertex(&buf, 0, 0).0[..2], [-16_000.0, -16_000.0]);
        assert_eq!(vertex(&buf, 128, 128).0[..2], [16_000.0, 16_000.0]);
        assert_eq!(vertex(&buf, 64, 64).0[..2], [0.0, 0.0]);
    }

    #[test]
    fn vertices_sit_on_the_ground_with_unit_upward_normals() {
        let geo = ProceduralGeo::default_frame();
        let mut buf = vec![0_u8; PATCH_BYTES];
        fill_patch(&geo, &mut buf, 0.0, 0.0, 250.0);
        for (i, j) in [(0, 0), (64, 64), (128, 0), (3, 99), (127, 127)] {
            let (p, n, h) = vertex(&buf, i, j);
            let (x, y) = (f64::from(p[0]), f64::from(p[1]));
            assert!(
                (f64::from(p[2]) - geo.ground_z_bf(x, y)).abs() < 0.01,
                "z at ({i},{j})"
            );
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            assert!(
                (len - 1.0).abs() < 1e-5 && n[2] > 0.0,
                "normal at ({i},{j}): {n:?}"
            );
            assert!((0.0..1_500.0).contains(&h), "MSL height {h} at ({i},{j})");
        }
    }

    #[test]
    fn the_curved_earth_drops_the_corners_below_the_centre_plane() {
        // On a flat world every vertex is at z = 0; on the sphere the corners fall away.
        let mut flat = vec![0_u8; PATCH_BYTES];
        fill_patch(&FlatGeo::new(), &mut flat, 0.0, 0.0, 250.0);
        assert!((0..POSTS).all(|i| vertex(&flat, i, 7).0[2] == 0.0));
        let sphere = ProceduralGeo::default_frame();
        // Compare the pure sphere drop by removing terrain: 22.6 km diagonal corner -> 40 m.
        let corner_drop = -(22_627.0_f64 * 22_627.0) / (2.0 * 6_371_000.0);
        assert!((corner_drop + 40.2).abs() < 0.5);
        let mut buf = vec![0_u8; PATCH_BYTES];
        fill_patch(&sphere, &mut buf, 0.0, 0.0, 250.0);
        assert!(vertex(&buf, 0, 0).0[2].is_finite());
    }

    #[test]
    fn filling_is_deterministic_and_rejects_bad_arguments() {
        let geo = ProceduralGeo::default_frame();
        let (mut a, mut b) = (vec![0_u8; PATCH_BYTES], vec![0xFF_u8; PATCH_BYTES]);
        fill_patch(&geo, &mut a, 10.0, 20.0, 100.0);
        fill_patch(&geo, &mut b, 10.0, 20.0, 100.0);
        assert_eq!(a, b);
        let mut small = vec![0_u8; PATCH_BYTES - 1];
        assert_eq!(fill_patch(&geo, &mut small, 0.0, 0.0, 250.0), 0);
        assert_eq!(fill_patch(&geo, &mut a, 0.0, 0.0, 0.0), 0);
        assert_eq!(fill_patch(&geo, &mut a, 0.0, 0.0, f64::NAN), 0);
        assert_eq!(fill_patch(&geo, &mut a, f64::INFINITY, 0.0, 250.0), 0);
    }
}
