//! One module per annotation tool. Each draws a resolved annotation on the [`Canvas`]; the
//! math runs in y-up image pixels.
//!
//! [`Canvas`]: crate::canvas::Canvas

pub mod arrow;
pub mod blur;
pub mod counter;
pub mod highlight;
pub mod label;
pub mod line;
pub mod shape;
pub mod spotlight;

/// The image an annotation is fitted to: its size, its straight-alpha RGBA8 pixels when known,
/// and the part of it the annotation must stay in (`[x0, y0, x1, y1]`, y-down px): the whole
/// image, or the crop that is written.
pub struct Pixels<'a> {
    pub width: u32,
    pub height: u32,
    pub rgba: Option<&'a [u8]>,
    pub area: [f64; 4],
}
