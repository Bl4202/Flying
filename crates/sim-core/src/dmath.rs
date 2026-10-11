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
}
