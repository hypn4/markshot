//! One module per the reference app tool. Each draws a resolved annotation on the [`Canvas`]; the math
//! runs in y-up image pixels, as in the the reference app notes.
//!
//! [`Canvas`]: crate::canvas::Canvas

pub mod arrow;
pub mod highlight;
pub mod line;
pub mod shape;
