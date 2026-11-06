//! Helpers shared by the integration tests: the fixture screenshot and golden PNGs.

#![allow(dead_code, reason = "each test crate uses only some of the helpers")]

use std::fs;
use std::path::PathBuf;

use markshot::image_io::{ColorSpace, SourceImage};

/// A `width` × `height` stand-in screenshot at 144 dpi (dpr 2): white, `#808080` and `#1E1E1E`
/// bands from the top, a third of the height each.
pub fn fixture(width: u32, height: u32) -> SourceImage {
    let rgba = (0..height)
        .flat_map(|y| {
            let v = match y * 3 / height {
                0 => 0xFF,
                1 => 0x80,
                _ => 0x1E,
            };
            (0..width).flat_map(move |_| [v, v, v, 0xFF])
        })
        .collect();
    SourceImage {
        width,
        height,
        rgba,
        dpi: Some(144.0),
        icc: None,
        color_space: ColorSpace::Srgb,
    }
}

/// [`fixture`] with something to hide: its bands tinted by eight vertical colour bars, and
/// rows of "words" (dark on the white band, light on the others).
pub fn screen(width: u32, height: u32) -> SourceImage {
    const BARS: [[u32; 3]; 8] = [
        [0xFF, 0x0C, 0x01],
        [0xFF, 0x95, 0x00],
        [0xFF, 0xDC, 0x2C],
        [0x34, 0xC7, 0x59],
        [0x2E, 0x7C, 0xF6],
        [0x58, 0x56, 0xD6],
        [0xAF, 0x52, 0xDE],
        [0xFF, 0xFF, 0xFF],
    ];
    let mut src = fixture(width, height);
    for (i, px) in src.rgba.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let i = u32::try_from(i).unwrap();
        let (x, y) = (i % width, i / width);
        let level = u32::from(px[0]);
        let word = (14..26).contains(&(y % 40)) && x % 16 < 11 && (x / 16 + y / 40) % 4 != 0;
        let rgb = match (word, level) {
            (true, 0xFF) => [20; 3],
            (true, _) => [235; 3],
            (false, _) => BARS[(x * 8 / width) as usize].map(|c| c * level / 255),
        };
        for (dst, c) in px.iter_mut().zip(rgb) {
            *dst = u8::try_from(c).unwrap();
        }
    }
    src
}

/// Asserts that `png` is byte for byte `tests/golden/<name>.png`. With `UPDATE_GOLDEN=1` it
/// writes the file instead; look at the image before committing it.
///
/// # Panics
/// When the bytes differ or the golden file cannot be read or written.
pub fn assert_golden(name: &str, png: &[u8]) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(format!("{name}.png"));
    if std::env::var_os("UPDATE_GOLDEN").is_some_and(|v| v == "1") {
        fs::write(&path, png).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        return;
    }
    let want = fs::read(&path)
        .unwrap_or_else(|e| panic!("{}: {e}; create it with UPDATE_GOLDEN=1", path.display()));
    assert!(
        want == png,
        "{name} differs from {}; if the change is intended, rerun with UPDATE_GOLDEN=1 and \
         look at the image",
        path.display()
    );
}
