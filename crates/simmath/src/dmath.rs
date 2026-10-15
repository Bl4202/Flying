//! Deterministic maths (ADR-10).
//!
//! WebAssembly guarantees IEEE-754 results for `+ - * /` and `sqrt`, but not for transcendental
//! functions, which the host `libm` computes differently per platform. Every transcendental in the
//! simulation goes through these wrappers over the pure-Rust `libm` crate. clippy forbids calling
//! the `f64` methods directly (see `clippy.toml`).

/// Sine of `x` radians.
#[must_use]
#[inline]
pub fn sin(x: f64) -> f64 {
    libm::sin(x)
}

/// Cosine of `x` radians.
#[must_use]
#[inline]
pub fn cos(x: f64) -> f64 {
    libm::cos(x)
}

/// Tangent of `x` radians.
#[must_use]
#[inline]
pub fn tan(x: f64) -> f64 {
    libm::tan(x)
}

/// Arcsine, result in radians.
#[must_use]
#[inline]
pub fn asin(x: f64) -> f64 {
    libm::asin(x)
}

/// Arccosine, result in radians.
#[must_use]
#[inline]
pub fn acos(x: f64) -> f64 {
    libm::acos(x)
}

/// Arctangent, result in radians.
#[must_use]
#[inline]
pub fn atan(x: f64) -> f64 {
    libm::atan(x)
}

/// Four-quadrant arctangent of `y / x`, result in radians.
#[must_use]
#[inline]
pub fn atan2(y: f64, x: f64) -> f64 {
    libm::atan2(y, x)
}

/// `e^x`.
#[must_use]
#[inline]
pub fn exp(x: f64) -> f64 {
    libm::exp(x)
}

/// Natural logarithm.
#[must_use]
#[inline]
pub fn ln(x: f64) -> f64 {
    libm::log(x)
}

/// `x^y`.
#[must_use]
#[inline]
pub fn pow(x: f64, y: f64) -> f64 {
    libm::pow(x, y)
}

/// Square root. IEEE-754 requires this to be correctly rounded, on every target including Wasm.
#[must_use]
#[inline]
pub fn sqrt(x: f64) -> f64 {
    x.sqrt()
}

/// Hyperbolic tangent.
#[must_use]
#[inline]
pub fn tanh(x: f64) -> f64 {
    libm::tanh(x)
}

/// `e^x - 1`, accurate for small `x`.
#[must_use]
#[inline]
pub fn exp_m1(x: f64) -> f64 {
    libm::expm1(x)
}

/// `ln(1 + x)`, accurate for small `x`.
#[must_use]
#[inline]
pub fn ln_1p(x: f64) -> f64 {
    libm::log1p(x)
}

/// `sqrt(x^2 + y^2)` without undue overflow or underflow.
#[must_use]
#[inline]
pub fn hypot(x: f64, y: f64) -> f64 {
    libm::hypot(x, y)
}

/// Cube root.
#[must_use]
#[inline]
pub fn cbrt(x: f64) -> f64 {
    libm::cbrt(x)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_values() {
        assert_eq!(sin(0.0), 0.0);
        assert_eq!(cos(0.0), 1.0);
        assert_eq!(exp(0.0), 1.0);
        assert_eq!(ln(1.0), 0.0);
        assert_eq!(pow(2.0, 10.0), 1024.0);
        assert_eq!(sqrt(9.0), 3.0);
        assert!((atan2(1.0, 1.0) - core::f64::consts::FRAC_PI_4).abs() < 1e-15);
        assert!((asin(1.0) - core::f64::consts::FRAC_PI_2).abs() < 1e-15);
    }

    #[test]
    fn added_wrappers_match_their_definitions() {
        assert_eq!(tanh(0.0), 0.0);
        // tanh(1) = (e^2 - 1) / (e^2 + 1)
        let e2 = exp(2.0);
        assert!((tanh(1.0) - (e2 - 1.0) / (e2 + 1.0)).abs() < 1e-15);
        assert!((tanh(40.0) - 1.0).abs() < 1e-15);
        // exp_m1 and ln_1p keep their precision where the naive forms lose it.
        // Series: e^x - 1 = x + x^2/2 + ..., ln(1 + x) = x - x^2/2 + ...; the naive forms
        // `exp(x) - 1` and `ln(1 + x)` would be wrong in the seventh significant digit here.
        let x = 1e-10;
        assert!((exp_m1(x) - (x + 0.5 * x * x)).abs() < 1e-25);
        assert!((ln_1p(x) - (x - 0.5 * x * x)).abs() < 1e-25);
        assert!((exp_m1(1.0) - (exp(1.0) - 1.0)).abs() < 1e-15);
        assert!((ln_1p(ln_1p(0.0) + exp_m1(0.5)) - 0.5).abs() < 1e-15);
        assert_eq!(hypot(3.0, 4.0), 5.0);
        assert!(hypot(1e300, 1e300).is_finite());
        assert_eq!(cbrt(27.0), 3.0);
        assert_eq!(cbrt(-8.0), -2.0);
    }
}
