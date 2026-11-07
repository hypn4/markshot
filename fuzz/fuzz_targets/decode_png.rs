//! Decodes arbitrary bytes as a PNG: any `Ok` or `Err` is fine, a panic, abort or hang is a bug.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = markshot::image_io::decode_png(data);
});
