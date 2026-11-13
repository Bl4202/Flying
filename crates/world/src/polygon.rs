//! The battlefield's polygon in the horizontal BF plane (GDD 8.2): at most 512 vertices, stored
//! counter-clockwise, in a fixed array.

/// The most vertices a BF polygon holds (a capsule has 258, a circle or ellipse 256). // ALGO: ADR-53
pub const MAX_VERTICES: usize = 512;

/// A simple polygon in the BF (x, y) plane, m, counter-clockwise.
#[derive(Clone, Debug, PartialEq)]
pub struct BfPolygon {
    pts: [[f64; 2]; MAX_VERTICES],
    n: usize,
}

impl Default for BfPolygon {
    fn default() -> Self {
        Self {
            pts: [[0.0; 2]; MAX_VERTICES],
            n: 0,
        }
    }
}

/// The squared distance from `p` to the segment `a`–`b`.
#[must_use]
pub fn dist2_to_segment(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let (px, py) = (p[0] - a[0], p[1] - a[1]);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 {
        ((px * dx + py * dy) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (ex, ey) = (px - t * dx, py - t * dy);
    ex * ex + ey * ey
}

/// The orientation of `c` against the line `a` → `b`: > 0 left, < 0 right, 0 on it.
fn orient(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

/// Whether `c` (collinear with `a`–`b`) lies within the segment's bounding box.
fn on_segment(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> bool {
    c[0] >= a[0].min(b[0])
        && c[0] <= a[0].max(b[0])
        && c[1] >= a[1].min(b[1])
        && c[1] <= a[1].max(b[1])
}

/// Whether the closed segments `p1`–`p2` and `q1`–`q2` meet.
fn segments_meet(p1: [f64; 2], p2: [f64; 2], q1: [f64; 2], q2: [f64; 2]) -> bool {
    let (d1, d2) = (orient(q1, q2, p1), orient(q1, q2, p2));
    let (d3, d4) = (orient(p1, p2, q1), orient(p1, p2, q2));
    if ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0))
        && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0))
    {
        return true;
    }
    (d1 == 0.0 && on_segment(q1, q2, p1))
        || (d2 == 0.0 && on_segment(q1, q2, p2))
        || (d3 == 0.0 && on_segment(p1, p2, q1))
        || (d4 == 0.0 && on_segment(p1, p2, q2))
}

impl BfPolygon {
    /// An empty polygon.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends a vertex; ignored once [`MAX_VERTICES`] are held.
    pub fn push(&mut self, x: f64, y: f64) {
        if self.n < MAX_VERTICES {
            self.pts[self.n] = [x, y];
            self.n += 1;
        }
    }

    /// The number of vertices.
    #[must_use]
    pub fn len(&self) -> usize {
        self.n
    }

    /// Whether it has no vertices.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    /// The vertices.
    #[must_use]
    pub fn points(&self) -> &[[f64; 2]] {
        &self.pts[..self.n]
    }

    /// Vertex `i`.
    #[must_use]
    pub fn point(&self, i: usize) -> [f64; 2] {
        self.pts[i]
    }

    /// Edge `i`, from vertex `i` to vertex `i + 1` (wrapping).
    #[must_use]
    pub fn edge(&self, i: usize) -> ([f64; 2], [f64; 2]) {
        (self.pts[i], self.pts[(i + 1) % self.n])
    }

    /// The signed area by the shoelace formula, m² (> 0 counter-clockwise).
    #[must_use]
    pub fn signed_area_m2(&self) -> f64 {
        let mut s = 0.0;
        for i in 0..self.n {
            let (a, b) = self.edge(i);
            s += a[0] * b[1] - b[0] * a[1];
        }
        0.5 * s
    }

    /// The area, m².
    #[must_use]
    pub fn area_m2(&self) -> f64 {
        self.signed_area_m2().abs()
    }

    /// Reverses the vertex order if the polygon is clockwise.
    pub fn make_ccw(&mut self) {
        if self.signed_area_m2() < 0.0 {
            self.pts[..self.n].reverse();
        }
    }

    /// The largest vertex-to-vertex distance, m (O(n²)).
    #[must_use]
    pub fn extent_m(&self) -> f64 {
        let mut best = 0.0f64;
        for i in 0..self.n {
            for j in i + 1..self.n {
                let (dx, dy) = (
                    self.pts[j][0] - self.pts[i][0],
                    self.pts[j][1] - self.pts[i][1],
                );
                best = best.max(dx * dx + dy * dy);
            }
        }
        best.sqrt()
    }

    /// The bounding box `(min_x, min_y, max_x, max_y)`, m.
    #[must_use]
    pub fn bbox(&self) -> (f64, f64, f64, f64) {
        let mut b = (
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        );
        for p in self.points() {
            b.0 = b.0.min(p[0]);
            b.1 = b.1.min(p[1]);
            b.2 = b.2.max(p[0]);
            b.3 = b.3.max(p[1]);
        }
        b
    }

    /// Whether no two non-adjacent edges meet (O(n²); run for authored polygons only).
    #[must_use]
    pub fn is_simple(&self) -> bool {
        let n = self.n;
        if n < 3 {
            return false;
        }
        for i in 0..n {
            let (a, b) = self.edge(i);
            for j in i + 1..n {
                let adjacent = j == i + 1 || (i == 0 && j == n - 1);
                if adjacent {
                    continue;
                }
                let (c, d) = self.edge(j);
                if segments_meet(a, b, c, d) {
                    return false;
                }
            }
        }
        true
    }

    /// The non-zero winding number of the polygon around `p`.
    #[must_use]
    pub fn winding(&self, p: [f64; 2]) -> i32 {
        let mut w = 0;
        for i in 0..self.n {
            let (a, b) = self.edge(i);
            if a[1] <= p[1] {
                if b[1] > p[1] && orient(a, b, p) > 0.0 {
                    w += 1;
                }
            } else if b[1] <= p[1] && orient(a, b, p) < 0.0 {
                w -= 1;
            }
        }
        w
    }

    /// Whether `p` is inside (non-zero winding).
    #[must_use]
    pub fn contains(&self, p: [f64; 2]) -> bool {
        self.winding(p) != 0
    }

    /// The distance from `p` to the polygon's boundary, m (O(n)).
    #[must_use]
    pub fn boundary_distance_m(&self, p: [f64; 2]) -> f64 {
        let mut best = f64::INFINITY;
        for i in 0..self.n {
            let (a, b) = self.edge(i);
            best = best.min(dist2_to_segment(p, a, b));
        }
        best.sqrt()
    }

    /// The signed distance (negative inside), m (O(n), exact; the SDF's reference).
    #[must_use]
    pub fn signed_distance_m(&self, p: [f64; 2]) -> f64 {
        let d = self.boundary_distance_m(p);
        if self.contains(p) { -d } else { d }
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
    fn a_square() {
        let p = square(1_000.0);
        assert_eq!(p.area_m2(), 4e6);
        assert!(p.signed_area_m2() > 0.0);
        assert!((p.extent_m() - 2_000.0 * 2f64.sqrt()).abs() < 1e-9);
        assert!(p.is_simple());
        assert!(p.contains([0.0, 0.0]));
        assert!(!p.contains([1_001.0, 0.0]));
        assert_eq!(p.signed_distance_m([0.0, 0.0]), -1_000.0);
        assert_eq!(p.signed_distance_m([1_500.0, 0.0]), 500.0);
        assert_eq!(p.bbox(), (-1_000.0, -1_000.0, 1_000.0, 1_000.0));
    }

    #[test]
    fn clockwise_is_reversed_and_a_bow_tie_is_not_simple() {
        let mut cw = BfPolygon::new();
        for (x, y) in [(0.0, 0.0), (0.0, 1.0), (1.0, 1.0), (1.0, 0.0)] {
            cw.push(x, y);
        }
        assert!(cw.signed_area_m2() < 0.0);
        cw.make_ccw();
        assert!(cw.signed_area_m2() > 0.0);
        let mut bow = BfPolygon::new();
        for (x, y) in [(0.0, 0.0), (1.0, 1.0), (1.0, 0.0), (0.0, 1.0)] {
            bow.push(x, y);
        }
        assert!(!bow.is_simple());
    }

    #[test]
    fn a_concave_polygon_winds_correctly() {
        // A "C" shape: the notch at x > 0, |y| < 1 is outside.
        let mut c = BfPolygon::new();
        for (x, y) in [
            (-3.0, -3.0),
            (3.0, -3.0),
            (3.0, -1.0),
            (0.0, -1.0),
            (0.0, 1.0),
            (3.0, 1.0),
            (3.0, 3.0),
            (-3.0, 3.0),
        ] {
            c.push(x, y);
        }
        assert!(c.is_simple());
        assert!(c.contains([-1.0, 0.0]));
        assert!(!c.contains([1.0, 0.0]));
        assert!(c.contains([1.0, 2.0]));
    }
}
