//! Table data as stored in a pack (GDD Appendix A): `f32` values with strictly increasing grids.
//! The flight model turns these into its own `f64` interpolators at load.

use crate::cpack::CpackError;

/// `y(x)`: a one-dimensional table (Appendix A `Table1D`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Curve {
    /// Grid, strictly increasing.
    pub xs: Vec<f32>,
    /// Values, one per grid point.
    pub ys: Vec<f32>,
}

/// `z(x, y)`: a two-dimensional table stored row-major as `z[y][x]` (Appendix A `Table2D`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Grid {
    /// X grid (the fast axis), strictly increasing.
    pub xs: Vec<f32>,
    /// Y grid (one row per point), strictly increasing.
    pub ys: Vec<f32>,
    /// `xs.len() * ys.len()` values, row by row.
    pub zs: Vec<f32>,
}

impl Curve {
    /// Builds a curve from `(x, y)` pairs.
    #[must_use]
    pub fn from_points(points: &[(f32, f32)]) -> Self {
        Self {
            xs: points.iter().map(|p| p.0).collect(),
            ys: points.iter().map(|p| p.1).collect(),
        }
    }

    /// Checks the shape: at least one point, strictly increasing finite grid, finite values.
    ///
    /// # Errors
    /// [`CpackError::Record`] naming the defect.
    pub fn validate(&self) -> Result<(), CpackError> {
        check_grid(&self.xs)?;
        if self.ys.len() != self.xs.len() {
            return Err(CpackError::Record(
                "a curve has a different number of values and grid points",
            ));
        }
        check_values(&self.ys)
    }
}

impl Grid {
    /// Row `j` (the values at `ys[j]`).
    #[must_use]
    pub fn row(&self, j: usize) -> &[f32] {
        &self.zs[j * self.xs.len()..(j + 1) * self.xs.len()]
    }

    /// Checks the shape: non-empty strictly increasing finite grids and `nx * ny` finite values.
    ///
    /// # Errors
    /// [`CpackError::Record`] naming the defect.
    pub fn validate(&self) -> Result<(), CpackError> {
        check_grid(&self.xs)?;
        check_grid(&self.ys)?;
        if self.zs.len() != self.xs.len() * self.ys.len() {
            return Err(CpackError::Record("a grid has the wrong number of values"));
        }
        check_values(&self.zs)
    }
}

fn check_grid(xs: &[f32]) -> Result<(), CpackError> {
    if xs.is_empty() {
        return Err(CpackError::Record("a table grid is empty"));
    }
    if xs.iter().any(|x| !x.is_finite()) {
        return Err(CpackError::Record("a table grid point is not finite"));
    }
    if xs.windows(2).any(|w| w[0] >= w[1]) {
        return Err(CpackError::Record(
            "a table grid is not strictly increasing",
        ));
    }
    Ok(())
}

fn check_values(vs: &[f32]) -> Result<(), CpackError> {
    if vs.iter().any(|v| !v.is_finite()) {
        return Err(CpackError::Record("a table value is not finite"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shapes_are_checked() {
        assert!(
            Curve::from_points(&[(0.0, 1.0), (1.0, 2.0)])
                .validate()
                .is_ok()
        );
        assert!(Curve::from_points(&[]).validate().is_err());
        assert!(
            Curve::from_points(&[(1.0, 1.0), (1.0, 2.0)])
                .validate()
                .is_err()
        );
        assert!(Curve::from_points(&[(0.0, f32::NAN)]).validate().is_err());
        let g = Grid {
            xs: vec![0.0, 1.0],
            ys: vec![0.0, 1.0, 2.0],
            zs: vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0],
        };
        assert!(g.validate().is_ok());
        assert_eq!(g.row(1), &[3.0, 4.0]);
        let mut bad = g;
        bad.zs.pop();
        assert!(bad.validate().is_err());
    }
}
