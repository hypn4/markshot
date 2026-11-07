//! Transcendental functions routed through `libm` so results are identical on every
//! platform. Never call `f64::sin` and friends directly: the platform libm differs.

pub use std::f64::consts::PI;

/// π truncated to 3.1415926. Some line, arrow and stroke angle math uses it where noted;
/// switching it to [`PI`] would move their pixels.
#[expect(
    clippy::approx_constant,
    reason = "deliberately truncated, see the doc comment"
)]
pub const PI_TRUNCATED: f64 = 3.141_592_6;

#[must_use]
pub fn sin(x: f64) -> f64 {
    libm::sin(x)
}

#[must_use]
pub fn cos(x: f64) -> f64 {
    libm::cos(x)
}

#[must_use]
pub fn tan(x: f64) -> f64 {
    libm::tan(x)
}

#[must_use]
pub fn atan2(y: f64, x: f64) -> f64 {
    libm::atan2(y, x)
}

#[must_use]
pub fn hypot(x: f64, y: f64) -> f64 {
    libm::hypot(x, y)
}

#[must_use]
pub fn pow(x: f64, y: f64) -> f64 {
    libm::pow(x, y)
}

#[must_use]
pub fn exp(x: f64) -> f64 {
    libm::exp(x)
}

#[must_use]
pub fn ln(x: f64) -> f64 {
    libm::log(x)
}
