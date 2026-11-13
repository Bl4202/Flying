//! The playable area's signed distance field (GDD 8.2, ADR-53): negative inside, positive outside,
//! exact (to `f32`) at every cell centre within `±D` of the boundary and clamped to `±D` beyond.
//!
//! - `W` (the warning band) is `warn_width_m` if positive, else `max(2 000 m, 3 % of the shorter
//!   side of the polygon's bounding box)`.
//! - `D = max(2 W, 10 000 m)`.
//! - The grid covers the bounding box expanded by `D` and two more cells on every side. Its square
//!   cells are `L0 / n0`, where `L0` is the longer side of the box expanded by `D` and
//!   `n0 = min(1 020, ceil(L0 / 100 m))`, so no side exceeds 1 024 cells (GDD 8.2's
//!   `min(1024, ceil(extent / 100 m))`, 4 MB at most).
//!
//! Build:
//! 1. every cell's squared distance to the nearest edge, for the cells within `D` of each edge's
//!    bounding box;
//! 2. per row, the non-zero winding number of each cell centre, by sorting the row's edge
//!    crossings;
//! 3. `sdf = ∓min(d, D)`, with `D` for cells no edge reached.

use crate::polygon::BfPolygon;

/// The least warning band, m. // DATA: GDD 8.5
pub const MIN_WARN_M: f64 = 2_000.0;
/// The warning band as a share of the shorter edge. // DATA: GDD 8.5
pub const WARN_SHARE: f64 = 0.03;
/// The least exact band `D`, m. // ALGO: ADR-53, covers the 10 km plan margin
pub const MIN_EXACT_M: f64 = 10_000.0;
/// The target cell size, m. // DATA: GDD 8.2
pub const TARGET_CELL_M: f64 = 100.0;
/// Cells along the longer side before the two-cell borders. // ALGO: 1 020 + 4 = GDD 8.2's 1 024
pub const MAX_INNER_CELLS: usize = 1_020;
/// Border cells on each side.
const BORDER: usize = 2;

/// The signed distance field of one polygon.
#[derive(Clone, Debug)]
pub struct Sdf {
    ox: f64,
    oy: f64,
    cell_m: f64,
    nx: usize,
    ny: usize,
    d_m: f64,
    warn_m: f64,
    values: Box<[f32]>,
}

/// The warning band `W` for `poly` (GDD 8.5): `warn_width_m` if positive, else
/// `max(2 000 m, 0.03 x the shorter side of the bounding box)`.
#[must_use]
pub fn warn_width(poly: &BfPolygon, warn_width_m: f64) -> f64 {
    if warn_width_m > 0.0 {
        return warn_width_m;
    }
    let (x0, y0, x1, y1) = poly.bbox();
    MIN_WARN_M.max(WARN_SHARE * (x1 - x0).min(y1 - y0))
}

impl Sdf {
    /// Builds the field of `poly` (counter-clockwise, at least 3 vertices).
    #[must_use]
    pub fn build(poly: &BfPolygon, warn_width_m: f64) -> Self {
        let warn_m = warn_width(poly, warn_width_m);
        let d_m = (2.0 * warn_m).max(MIN_EXACT_M);
        let (bx0, by0, bx1, by1) = poly.bbox();
        let (w, h) = (bx1 - bx0, by1 - by0);
        let l0 = w.max(h) + 2.0 * d_m;
        let n0 = ((l0 / TARGET_CELL_M).ceil() as usize).clamp(1, MAX_INNER_CELLS);
        let cell_m = l0 / n0 as f64;
        let cells = |side: f64| {
            ((((side + 2.0 * d_m) / cell_m) - 1e-9).ceil() as usize).clamp(1, MAX_INNER_CELLS)
                + 2 * BORDER
        }; // ALGO: 1e-9 absorbs L0 / cell rounding up
        let (nx, ny) = (cells(w), cells(h));
        let border = BORDER as f64 * cell_m;
        let mut sdf = Self {
            ox: bx0 - d_m - border,
            oy: by0 - d_m - border,
            cell_m,
            nx,
            ny,
            d_m,
            warn_m,
            values: vec![f32::INFINITY; nx * ny].into_boxed_slice(),
        };
        sdf.fill_distances(poly);
        sdf.apply_signs(poly);
        sdf
    }

    fn centre(&self, i: usize, j: usize) -> (f64, f64) {
        (
            self.ox + (i as f64 + 0.5) * self.cell_m,
            self.oy + (j as f64 + 0.5) * self.cell_m,
        )
    }

    /// The cell index range whose centres lie in [`lo`, `hi`] along an axis of `n` cells.
    fn range(&self, lo: f64, hi: f64, origin: f64, n: usize) -> (usize, usize) {
        let a = ((lo - origin) / self.cell_m - 0.5).ceil().max(0.0) as usize;
        let b = (((hi - origin) / self.cell_m - 0.5).floor().max(-1.0) + 1.0) as usize;
        (a.min(n), b.min(n))
    }

    fn fill_distances(&mut self, poly: &BfPolygon) {
        for e in 0..poly.len() {
            let (a, b) = poly.edge(e);
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let len2 = dx * dx + dy * dy;
            let inv = if len2 > 0.0 { 1.0 / len2 } else { 0.0 };
            let (i0, i1) = self.range(
                a[0].min(b[0]) - self.d_m,
                a[0].max(b[0]) + self.d_m,
                self.ox,
                self.nx,
            );
            let (j0, j1) = self.range(
                a[1].min(b[1]) - self.d_m,
                a[1].max(b[1]) + self.d_m,
                self.oy,
                self.ny,
            );
            for j in j0..j1 {
                let row = j * self.nx;
                let py = self.oy + (j as f64 + 0.5) * self.cell_m - a[1];
                for i in i0..i1 {
                    let px = self.ox + (i as f64 + 0.5) * self.cell_m - a[0];
                    let t = ((px * dx + py * dy) * inv).clamp(0.0, 1.0);
                    let (ex, ey) = (px - t * dx, py - t * dy);
                    let d2 = (ex * ex + ey * ey) as f32;
                    let v = &mut self.values[row + i];
                    if d2 < *v {
                        *v = d2;
                    }
                }
            }
        }
    }

    fn apply_signs(&mut self, poly: &BfPolygon) {
        let mut crossings: Vec<(f64, i32)> = Vec::with_capacity(poly.len());
        for j in 0..self.ny {
            let (_, yc) = self.centre(0, j);
            crossings.clear();
            for e in 0..poly.len() {
                let (a, b) = poly.edge(e);
                let up = a[1] <= yc && b[1] > yc;
                let down = b[1] <= yc && a[1] > yc;
                if up || down {
                    let x = a[0] + (yc - a[1]) * (b[0] - a[0]) / (b[1] - a[1]);
                    crossings.push((x, if up { 1 } else { -1 }));
                }
            }
            crossings.sort_unstable_by(|p, q| p.0.total_cmp(&q.0));
            let mut k = 0;
            let mut wind = 0;
            for i in 0..self.nx {
                let (xc, _) = self.centre(i, j);
                while k < crossings.len() && crossings[k].0 < xc {
                    wind += crossings[k].1;
                    k += 1;
                }
                let v = &mut self.values[j * self.nx + i];
                let d = if v.is_finite() {
                    f64::from(*v).sqrt().min(self.d_m)
                } else {
                    self.d_m
                };
                *v = (if wind != 0 { -d } else { d }) as f32;
            }
        }
    }

    /// The signed distance at (`x`, `y`), m: bilinear between the four nearest cell centres; `+D`
    /// outside the grid.
    #[must_use]
    pub fn sample(&self, x: f64, y: f64) -> f64 {
        let fx = (x - self.ox) / self.cell_m - 0.5;
        let fy = (y - self.oy) / self.cell_m - 0.5;
        if !(fx >= -0.5 && fy >= -0.5 && fx <= self.nx as f64 - 0.5 && fy <= self.ny as f64 - 0.5) {
            return self.d_m;
        }
        let fx = fx.clamp(0.0, (self.nx - 1) as f64);
        let fy = fy.clamp(0.0, (self.ny - 1) as f64);
        let i = (fx.floor() as usize).min(self.nx - 2);
        let j = (fy.floor() as usize).min(self.ny - 2);
        let (tx, ty) = (fx - i as f64, fy - j as f64);
        let v = |ii: usize, jj: usize| f64::from(self.values[jj * self.nx + ii]);
        let a = v(i, j) + (v(i + 1, j) - v(i, j)) * tx;
        let b = v(i, j + 1) + (v(i + 1, j + 1) - v(i, j + 1)) * tx;
        a + (b - a) * ty
    }

    /// The gradient by central differences over one cell.
    #[must_use]
    pub fn gradient(&self, x: f64, y: f64) -> [f64; 2] {
        let h = 0.5 * self.cell_m;
        [
            (self.sample(x + h, y) - self.sample(x - h, y)) / (2.0 * h),
            (self.sample(x, y + h) - self.sample(x, y - h)) / (2.0 * h),
        ]
    }

    /// The outward boundary normal (the normalised gradient), or (0, 0) where the field is flat.
    #[must_use]
    pub fn normal(&self, x: f64, y: f64) -> [f64; 2] {
        let g = self.gradient(x, y);
        let l = (g[0] * g[0] + g[1] * g[1]).sqrt();
        if l > 0.0 {
            [g[0] / l, g[1] / l]
        } else {
            [0.0, 0.0]
        }
    }

    /// The cell size, m.
    #[must_use]
    pub fn cell_m(&self) -> f64 {
        self.cell_m
    }

    /// The grid's (columns, rows).
    #[must_use]
    pub fn dims(&self) -> (usize, usize) {
        (self.nx, self.ny)
    }

    /// The exact band and clamp `D`, m.
    #[must_use]
    pub fn d_clamp_m(&self) -> f64 {
        self.d_m
    }

    /// The warning band `W`, m.
    #[must_use]
    pub fn warn_width_m(&self) -> f64 {
        self.warn_m
    }

    /// The value stored for cell (`i`, `j`), and its centre: for the tests and the selftests.
    #[must_use]
    pub fn cell(&self, i: usize, j: usize) -> (f64, f64, f32) {
        let (x, y) = self.centre(i, j);
        (x, y, self.values[j * self.nx + i])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(s: f64) -> BfPolygon {
        let mut p = BfPolygon::new();
        for (x, y) in [(-s, -s), (s, -s), (s, s), (-s, s)] {
            p.push(x, y);
        }
        p
    }

    #[test]
    fn the_sign_and_the_zero_crossing() {
        let p = square(25_000.0);
        let s = Sdf::build(&p, 0.0);
        assert_eq!(s.warn_width_m(), 2_000.0);
        assert_eq!(s.d_clamp_m(), 10_000.0);
        assert!(s.sample(0.0, 0.0) < 0.0);
        assert_eq!(s.sample(0.0, 0.0), -10_000.0);
        assert!((s.sample(25_000.0, 3_000.0)).abs() < s.cell_m());
        assert!((s.sample(24_000.0, 0.0) - -1_000.0).abs() < 1.0);
        assert!((s.sample(26_500.0, 0.0) - 1_500.0).abs() < 1.0);
        assert_eq!(s.sample(1e7, 0.0), 10_000.0);
        let n = s.normal(24_500.0, 0.0);
        assert!((n[0] - 1.0).abs() < 1e-6 && n[1].abs() < 1e-6, "{n:?}");
        let (nx, ny) = s.dims();
        assert!(nx <= 1_024 && ny <= 1_024);
    }

    #[test]
    fn a_large_polygon_keeps_to_1024_cells() {
        let s = Sdf::build(&square(500_000.0), 0.0);
        let (nx, ny) = s.dims();
        assert_eq!((nx, ny), (1_024, 1_024));
        assert_eq!(s.warn_width_m(), 30_000.0);
        assert_eq!(s.d_clamp_m(), 60_000.0);
    }
}
