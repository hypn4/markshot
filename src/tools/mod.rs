//! One module per annotation tool. Each draws a resolved annotation on the [`Canvas`]; the
//! math runs in y-up image pixels.
//!
//! [`Canvas`]: crate::canvas::Canvas

pub mod arrow;
pub mod blur;
pub mod counter;
pub mod highlight;
pub mod line;
pub mod shape;
pub mod spotlight;
