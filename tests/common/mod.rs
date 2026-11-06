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
