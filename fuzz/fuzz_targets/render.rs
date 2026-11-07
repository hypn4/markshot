//! Parses arbitrary bytes as a JSON spec and renders it on a fixed 64x48 image: any `Ok` or
//! `Err` is fine, a panic, abort or hang is a bug.

#![no_main]

use libfuzzer_sys::fuzz_target;
use markshot::image_io::{ColorSpace, SourceImage};
use markshot::spec::Spec;

const WIDTH: u32 = 64;
const HEIGHT: u32 = 48;

/// A light top half over a dark bottom half, so blend modes that depend on the background's
/// brightness both run.
fn source() -> SourceImage {
    let rgba = (0..HEIGHT)
        .flat_map(|y| {
            let v = if y < HEIGHT / 2 { 240 } else { 30 };
            (0..WIDTH).flat_map(move |_| [v, v, v, 255])
        })
        .collect();
    SourceImage {
        width: WIDTH,
        height: HEIGHT,
        rgba,
        dpi: None,
        icc: None,
        color_space: ColorSpace::Srgb,
    }
}

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let Ok(spec) = Spec::from_json(text) else {
        return;
    };
    let _ = markshot::render::render(source(), &spec, None);
});
