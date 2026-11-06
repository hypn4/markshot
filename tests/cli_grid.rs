//! `markshot grid`: the input with a labeled pixel grid drawn over it, same size and
//! metadata; a bad `--step` or file fails with exit 1 and a located message on stderr.

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;

use assert_cmd::cargo::cargo_bin_cmd;
use markshot::grid::grid;
use markshot::image_io::{SourceImage, decode_png, encode_png};

/// A fresh, empty directory for one test's files.
fn work_dir(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("cli_grid")
        .join(test);
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Writes the `width` × `height` fixture as `input.png` in `dir`.
fn write_fixture(dir: &Path, width: u32, height: u32) -> PathBuf {
    let src = common::fixture(width, height);
    let png = encode_png(src.width, src.height, &src.rgba, src.dpi, None).unwrap();
    let path = dir.join("input.png");
    fs::write(&path, png).unwrap();
    path
}

/// Runs `markshot grid <input> -o <out> <extra…>`.
fn run_grid(input: &Path, out: &Path, extra: &[&str]) -> Output {
    cargo_bin_cmd!("markshot")
        .arg("grid")
        .arg(input)
        .arg("-o")
        .arg(out)
        .args(extra)
        .output()
        .expect("markshot runs")
}

/// The RGBA of pixel (`x`, `y`), y down.
fn pixel(image: &SourceImage, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * image.width + x) * 4) as usize;
    image.rgba[i..i + 4].try_into().unwrap()
}

/// Whether `[r, g, b, _]` is the gray fixture tinted magenta: red and blue well above green.
fn magenta([r, g, b, _]: [u8; 4]) -> bool {
    r > g.saturating_add(60) && b > g.saturating_add(60)
}

#[test]
fn grid_draws_lines_on_every_step_and_keeps_size_and_dpi() {
    let dir = work_dir("grid_draws_lines_on_every_step_and_keeps_size_and_dpi");
    let input = write_fixture(&dir, 1000, 500);
    let out = dir.join("out.png");
    let output = run_grid(&input, &out, &["--step", "100"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let image = decode_png(&fs::read(&out).unwrap()).unwrap();
    assert_eq!((image.width, image.height), (1000, 500));
    assert_eq!(image.dpi, Some(144.0));

    // Away from the labels on the top and left edges.
    let (row, col) = (250, 550);
    let vertical: Vec<u32> = (0..1000)
        .filter(|&x| magenta(pixel(&image, x, row)))
        .collect();
    assert_eq!(vertical, (1..10).map(|i| i * 100).collect::<Vec<_>>());
    let horizontal: Vec<u32> = (0..500)
        .filter(|&y| magenta(pixel(&image, col, y)))
        .collect();
    assert_eq!(horizontal, [100, 200, 300, 400]);
    // A line covers its whole column, and nothing next to it away from the labels.
    for y in [0, 60, 499] {
        assert!(magenta(pixel(&image, 300, y)), "(300, {y})");
    }
    for y in [60, 499] {
        assert_eq!(pixel(&image, 301, y), pixel(&image, 302, y), "(301, {y})");
    }
}

#[test]
fn zero_step_exits_1_with_location() {
    let dir = work_dir("zero_step_exits_1_with_location");
    let input = write_fixture(&dir, 100, 100);
    let out = dir.join("out.png");
    let output = run_grid(&input, &out, &["--step", "0"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.starts_with("error: --step: "), "{stderr}");
    assert!(!out.exists());
}

#[test]
fn file_errors_name_the_file() {
    let dir = work_dir("file_errors_name_the_file");
    let input = write_fixture(&dir, 100, 100);
    let missing = dir.join("missing.png");
    let no_dir = dir.join("no-such-dir/out.png");
    for (input, out, culprit) in [
        (&missing, &dir.join("out.png"), &missing),
        (&input, &no_dir, &no_dir),
    ] {
        let output = run_grid(input, out, &[]);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        let want = format!("error: {}: ", culprit.display());
        assert!(stderr.starts_with(&want), "{stderr}");
    }
}

/// The default step on the 1000×500 fixture (dpr 2) is 50. Regenerate with
/// `UPDATE_GOLDEN=1 cargo test --test cli_grid` and look at the image before committing it.
#[test]
fn golden_grid_1000x500() {
    let png = grid(common::fixture(1000, 500), None).expect("draws");
    common::assert_golden("grid-1000x500", &png);
}
