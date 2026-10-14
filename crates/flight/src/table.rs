//! Interpolated data tables (GDD Appendix A conventions).
//!
//! `Table1D` is `y(x)`; `Table2D` is `z(x, y)` stored row-major as `z[y][x]`. Grids are strictly
//! increasing, interpolation is multilinear in `f64`, and the ends are clamped (no
//! extrapolation). Tables are built once at load (from the `f32` values of a content pack) and own
//! their storage; evaluation allocates nothing and is bit-identical on every target.

use core::fmt;

/// Why a table was rejected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TableError {
    /// A grid has no points.
    Empty,
    /// The value count does not match the grid sizes.
    LengthMismatch {
        /// Values expected.
        expected: usize,
        /// Values found.
        found: usize,
    },
    /// A grid is not strictly increasing at this index.
    NotIncreasing {
        /// Index of the offending grid point.
        index: usize,
    },
    /// A grid point or value is not finite at this index.
    NotFinite {
        /// Index into the grid or value array.
        index: usize,
    },
}

impl fmt::Display for TableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "table grid is empty"),
            Self::LengthMismatch { expected, found } => {
                write!(f, "table has {found} values, expected {expected}")
            }
            Self::NotIncreasing { index } => {
                write!(f, "table grid is not strictly increasing at index {index}")
            }
            Self::NotFinite { index } => write!(f, "table entry {index} is not finite"),
        }
    }
}

impl std::error::Error for TableError {}

fn check_grid(xs: &[f64]) -> Result<(), TableError> {
    if xs.is_empty() {
        return Err(TableError::Empty);
    }
    for (i, x) in xs.iter().enumerate() {
        if !x.is_finite() {
            return Err(TableError::NotFinite { index: i });
        }
        if i > 0 && xs[i - 1] >= *x {
            return Err(TableError::NotIncreasing { index: i });
        }
    }
    Ok(())
}

fn check_values(vs: &[f64], expected: usize) -> Result<(), TableError> {
    if vs.len() != expected {
        return Err(TableError::LengthMismatch {
            expected,
            found: vs.len(),
        });
    }
    match vs.iter().position(|v| !v.is_finite()) {
        Some(index) => Err(TableError::NotFinite { index }),
        None => Ok(()),
    }
}

/// The interval holding `x` and the fraction along it, clamped to the grid: `(i, t)` with
/// `0 <= t <= 1`, interpolating between `xs[i]` and `xs[i + 1]`. A one-point grid gives `(0, 0)`.
/// The caller handles NaN.
fn bracket(xs: &[f64], x: f64) -> (usize, f64) {
    let n = xs.len();
    if n == 1 || x <= xs[0] {
        return (0, 0.0);
    }
    if x >= xs[n - 1] {
        return (n - 2, 1.0);
    }
    // xs[0] < x < xs[n - 1], so 1 <= partition_point <= n - 1.
    let i = xs.partition_point(|v| *v <= x) - 1;
    (i, (x - xs[i]) / (xs[i + 1] - xs[i]))
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// `y(x)`, linear between grid points, clamped at the ends.
#[derive(Clone, Debug, PartialEq)]
pub struct Table1D {
    xs: Box<[f64]>,
    ys: Box<[f64]>,
}

impl Table1D {
    /// Builds a table, checking the grid and the values.
    ///
    /// # Errors
    /// [`TableError`] if the grid is empty, not strictly increasing or not finite, or the value
    /// count differs from the grid.
    pub fn new(xs: &[f64], ys: &[f64]) -> Result<Self, TableError> {
        check_grid(xs)?;
        check_values(ys, xs.len())?;
        Ok(Self {
            xs: xs.into(),
            ys: ys.into(),
        })
    }

    /// Builds a table from `f32` content values (each converts to `f64` exactly).
    ///
    /// # Errors
    /// As [`Table1D::new`].
    pub fn from_f32(xs: &[f32], ys: &[f32]) -> Result<Self, TableError> {
        let xs: Vec<f64> = xs.iter().map(|v| f64::from(*v)).collect();
        let ys: Vec<f64> = ys.iter().map(|v| f64::from(*v)).collect();
        Self::new(&xs, &ys)
    }

    /// A table with the same value everywhere.
    #[must_use]
    pub fn constant(value: f64) -> Self {
        Self {
            xs: Box::new([0.0]),
            ys: Box::new([value]),
        }
    }

    /// The value at `x`. NaN in, NaN out.
    #[must_use]
    pub fn eval(&self, x: f64) -> f64 {
        if x.is_nan() {
            return f64::NAN;
        }
        let (i, t) = bracket(&self.xs, x);
        if self.xs.len() == 1 {
            return self.ys[0];
        }
        lerp(self.ys[i], self.ys[i + 1], t)
    }

    /// The grid.
    #[must_use]
    pub fn xs(&self) -> &[f64] {
        &self.xs
    }

    /// The values.
    #[must_use]
    pub fn ys(&self) -> &[f64] {
        &self.ys
    }
}

/// `z(x, y)`, bilinear between grid points, clamped at the ends. Values are row-major `z[y][x]`.
#[derive(Clone, Debug, PartialEq)]
pub struct Table2D {
    xs: Box<[f64]>,
    ys: Box<[f64]>,
    zs: Box<[f64]>,
}

impl Table2D {
    /// Builds a table from the two grids and `xs.len() * ys.len()` row-major values.
    ///
    /// # Errors
    /// [`TableError`] if a grid is empty, not strictly increasing or not finite, or the value
    /// count is wrong.
    pub fn new(xs: &[f64], ys: &[f64], zs: &[f64]) -> Result<Self, TableError> {
        check_grid(xs)?;
        check_grid(ys)?;
        check_values(zs, xs.len() * ys.len())?;
        Ok(Self {
            xs: xs.into(),
            ys: ys.into(),
            zs: zs.into(),
        })
    }

    /// Builds a table from `f32` content values.
    ///
    /// # Errors
    /// As [`Table2D::new`].
    pub fn from_f32(xs: &[f32], ys: &[f32], zs: &[f32]) -> Result<Self, TableError> {
        let to64 = |s: &[f32]| s.iter().map(|v| f64::from(*v)).collect::<Vec<f64>>();
        Self::new(&to64(xs), &to64(ys), &to64(zs))
    }

    /// The value at `(x, y)`. NaN in, NaN out.
    #[must_use]
    pub fn eval(&self, x: f64, y: f64) -> f64 {
        if x.is_nan() || y.is_nan() {
            return f64::NAN;
        }
        let nx = self.xs.len();
        let (i, tx) = bracket(&self.xs, x);
        let (j, ty) = bracket(&self.ys, y);
        let i1 = if nx == 1 { i } else { i + 1 };
        let j1 = if self.ys.len() == 1 { j } else { j + 1 };
        let z = |jj: usize, ii: usize| self.zs[jj * nx + ii];
        let lo = lerp(z(j, i), z(j, i1), tx);
        let hi = lerp(z(j1, i), z(j1, i1), tx);
        lerp(lo, hi, ty)
    }

    /// The x grid.
    #[must_use]
    pub fn xs(&self) -> &[f64] {
        &self.xs
    }

    /// The y grid.
    #[must_use]
    pub fn ys(&self) -> &[f64] {
        &self.ys
    }

    /// Row `j` (all x values at `ys[j]`).
    #[must_use]
    pub fn row(&self, j: usize) -> &[f64] {
        let nx = self.xs.len();
        &self.zs[j * nx..(j + 1) * nx]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_dimensional_interpolation_and_clamping() {
        // GDD C.4 wave drag.
        let t = Table1D::new(
            &[0.0, 0.75, 0.85, 0.95, 1.05, 1.20, 1.50, 2.00, 2.40],
            &[
                0.0, 0.0, 0.0030, 0.0100, 0.0260, 0.0250, 0.0190, 0.0130, 0.0120,
            ],
        )
        .unwrap();
        assert_eq!(t.eval(0.85), 0.0030); // exact grid hit
        assert!((t.eval(0.90) - 0.0065).abs() < 1e-15);
        assert!((t.eval(1.0) - 0.0180).abs() < 1e-15);
        assert_eq!(t.eval(-1.0), 0.0); // clamped below
        assert_eq!(t.eval(3.0), 0.0120); // clamped above
        assert!(t.eval(f64::NAN).is_nan());
        assert_eq!(Table1D::constant(0.0235).eval(1.7), 0.0235);
    }

    #[test]
    fn bilinear_worked_example() {
        // z(x, y) = 1 + 2x + 3y + 4xy on a 2 x 3 grid is reproduced exactly inside each cell.
        let xs = [0.0, 2.0];
        let ys = [0.0, 1.0, 3.0];
        let f = |x: f64, y: f64| 1.0 + 2.0 * x + 3.0 * y + 4.0 * x * y;
        let mut zs = Vec::new();
        for &y in &ys {
            for &x in &xs {
                zs.push(f(x, y));
            }
        }
        let t = Table2D::new(&xs, &ys, &zs).unwrap();
        for (x, y) in [(0.5, 0.25), (1.0, 2.0), (2.0, 3.0), (0.0, 0.0), (1.5, 1.0)] {
            assert!((t.eval(x, y) - f(x, y)).abs() < 1e-12, "({x}, {y})");
        }
        // Clamped outside the grid.
        assert_eq!(t.eval(-5.0, -5.0), f(0.0, 0.0));
        assert_eq!(t.eval(9.0, 9.0), f(2.0, 3.0));
        assert_eq!(t.row(1), &[f(0.0, 1.0), f(2.0, 1.0)]);
    }

    #[test]
    fn single_point_axes_degenerate_gracefully() {
        let t = Table2D::new(&[1.0], &[0.0, 1.0], &[10.0, 20.0]).unwrap();
        assert_eq!(t.eval(-3.0, 0.5), 15.0);
        let t = Table2D::new(&[0.0, 1.0], &[5.0], &[10.0, 20.0]).unwrap();
        assert_eq!(t.eval(0.25, 99.0), 12.5);
    }

    #[test]
    fn bad_tables_are_rejected() {
        assert_eq!(Table1D::new(&[], &[]), Err(TableError::Empty));
        assert_eq!(
            Table1D::new(&[0.0, 1.0], &[1.0]),
            Err(TableError::LengthMismatch {
                expected: 2,
                found: 1
            })
        );
        assert_eq!(
            Table1D::new(&[0.0, 1.0, 1.0], &[1.0, 2.0, 3.0]),
            Err(TableError::NotIncreasing { index: 2 })
        );
        assert_eq!(
            Table1D::new(&[0.0, f64::NAN], &[1.0, 2.0]),
            Err(TableError::NotFinite { index: 1 })
        );
        assert_eq!(
            Table2D::new(&[0.0, 1.0], &[0.0], &[1.0, f64::INFINITY]),
            Err(TableError::NotFinite { index: 1 })
        );
        assert!(Table2D::new(&[0.0, 1.0], &[0.0, 1.0], &[1.0; 3]).is_err());
    }

    #[test]
    fn f32_content_converts_exactly() {
        let t = Table1D::from_f32(&[0.0, 0.5], &[0.0235, 0.1]).unwrap();
        assert_eq!(t.eval(0.0), f64::from(0.0235_f32));
        let t = Table2D::from_f32(&[0.0, 1.0], &[0.0, 1.0], &[0.0, 1.0, 2.0, 3.0]).unwrap();
        assert_eq!(t.eval(0.5, 0.5), 1.5);
    }
}
