//! `markshot render`: writes the annotated PNG, prints the report as JSON, and fails with
//! exit 1 and a located message on stderr.

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;

use assert_cmd::cargo::cargo_bin_cmd;
use markshot::image_io::{SourceImage, decode_png, encode_png};
use serde_json::{Value, json};

/// `#FF0C01`, the default color, opaque.
const RED: [u8; 4] = [0xFF, 0x0C, 0x01, 0xFF];

/// A fresh, empty directory for one test's files.
fn work_dir(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("cli_render")
        .join(test);
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Writes the 600×400 fixture as `input.png` in `dir`.
fn write_fixture(dir: &Path) -> PathBuf {
    let src = common::fixture(600, 400);
    let png = encode_png(src.width, src.height, &src.rgba, src.dpi, None).unwrap();
    let path = dir.join("input.png");
    fs::write(&path, png).unwrap();
    path
}

/// Runs `markshot render <input> --spec - -o <out> <extra…>` with `spec` on stdin.
fn render(input: &Path, out: &Path, spec: &Value, extra: &[&str]) -> Output {
    cargo_bin_cmd!("markshot")
        .arg("render")
        .arg(input)
        .args(["--spec", "-", "-o"])
        .arg(out)
        .args(extra)
        .write_stdin(spec.to_string())
        .output()
        .expect("markshot runs")
}

/// The report printed by a successful run.
fn report(output: &Output) -> Value {
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    serde_json::from_slice(&output.stdout).expect("stdout is JSON")
}

fn read_png(path: &Path) -> SourceImage {
    decode_png(&fs::read(path).unwrap()).unwrap()
}

/// The RGBA of pixel (`x`, `y`).
fn pixel(image: &SourceImage, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * image.width + x) * 4) as usize;
    image.rgba[i..i + 4].try_into().unwrap()
}

/// A PNG of `color` type and 8-bit depth, with `configure` adding chunks.
fn encode(
    width: u32,
    height: u32,
    color: png::ColorType,
    configure: impl FnOnce(&mut png::Info<'static>),
    data: &[u8],
) -> Vec<u8> {
    let mut info = png::Info::with_size(width, height);
    info.color_type = color;
    info.bit_depth = png::BitDepth::Eight;
    configure(&mut info);
    let mut out = Vec::new();
    let mut writer = png::Encoder::with_info(&mut out, info)
        .unwrap()
        .write_header()
        .unwrap();
    writer.write_image_data(data).unwrap();
    writer.finish().unwrap();
    out
}

#[test]
fn render_writes_png_and_report() {
    let dir = work_dir("render_writes_png_and_report");
    let input = write_fixture(&dir);
    let out = dir.join("out.png");
    let spec = json!({"annotations": [{"type": "line", "from": [40, 60], "to": [560, 340]}]});
    let report = report(&render(&input, &out, &spec, &[]));
    assert_eq!(report["width"], 600);
    assert_eq!(report["height"], 400);
    assert_eq!(report["dpr"], 2.0);
    assert_eq!(report["spec"]["dpr"], 2.0);
    assert_eq!(report["spec"]["annotations"][0]["id"], "line-1");
    assert_eq!(report["warnings"], json!([]));
    let image = read_png(&out);
    assert_eq!((image.width, image.height), (600, 400));
    assert_eq!(image.dpi, Some(144.0));
    assert_eq!(pixel(&image, 300, 200), RED, "the line's midpoint");
    assert_eq!(pixel(&image, 300, 20), [0xFF; 4], "the white band");
}

#[test]
fn rerender_from_report_is_byte_identical() {
    let dir = work_dir("rerender_from_report_is_byte_identical");
    let input = write_fixture(&dir);
    let spec = json!({
        "coords": {"space": "normalized"},
        "annotations": [
            {"type": "line", "from": [100, 150], "to": [900, 850], "end_cap": "arrow"},
            {"type": "line", "from": [100, 850], "to": [900, 150], "dash": true, "size": 4,
             "color": "#2E7CF6"}
        ]
    });
    let first_out = dir.join("first.png");
    let first = report(&render(&input, &first_out, &spec, &[]));

    let resolved = dir.join("resolved.json");
    fs::write(&resolved, first["spec"].to_string()).unwrap();
    let second_out = dir.join("second.png");
    let output = cargo_bin_cmd!("markshot")
        .arg("render")
        .arg(&input)
        .arg("--spec")
        .arg(&resolved)
        .arg("-o")
        .arg(&second_out)
        .output()
        .expect("markshot runs");
    let second = report(&output);

    assert_eq!(second["spec"], first["spec"]);
    assert!(
        fs::read(&first_out).unwrap() == fs::read(&second_out).unwrap(),
        "the PNGs differ"
    );
}

#[test]
fn bad_spec_exits_1_with_location() {
    let dir = work_dir("bad_spec_exits_1_with_location");
    let input = write_fixture(&dir);
    let out = dir.join("out.png");
    let spec =
        json!({"annotations": [{"type": "line", "from": [0, 0], "to": [10, 10], "size": 9}]});
    let output = render(&input, &out, &spec, &[]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.starts_with("error: annotations[0].size: "),
        "{stderr}"
    );
    assert!(!out.exists());
}

#[test]
fn file_errors_name_the_file() {
    let dir = work_dir("file_errors_name_the_file");
    let input = write_fixture(&dir);
    let spec = dir.join("spec.json");
    fs::write(&spec, r#"{"annotations": []}"#).unwrap();
    let out = dir.join("out.png");
    let missing = dir.join("missing.png");
    let no_dir = dir.join("no-such-dir/out.png");
    for (args, culprit) in [
        ([&missing, &spec, &out], &missing),
        ([&input, &missing, &out], &missing),
        ([&input, &spec, &no_dir], &no_dir),
    ] {
        let [input, spec, out] = args;
        let output = cargo_bin_cmd!("markshot")
            .arg("render")
            .arg(input)
            .arg("--spec")
            .arg(spec)
            .arg("-o")
            .arg(out)
            .output()
            .expect("markshot runs");
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        let want = format!("error: {}: ", culprit.display());
        assert!(stderr.starts_with(&want), "{stderr}");
    }
}

#[test]
fn dpr_option_overrides_the_png_dpi() {
    let dir = work_dir("dpr_option_overrides_the_png_dpi");
    let input = write_fixture(&dir);
    let out = dir.join("out.png");
    let spec =
        json!({"dpr": 3, "annotations": [{"type": "line", "from": [40, 60], "to": [560, 340]}]});
    let report = report(&render(&input, &out, &spec, &["--dpr", "1"]));
    assert_eq!(report["dpr"], 1.0);
    assert_eq!(report["spec"]["dpr"], 1.0);
}

#[test]
fn grayscale_and_palette_inputs_render() {
    let dir = work_dir("grayscale_and_palette_inputs_render");
    let spec = json!({"annotations": [{"type": "line", "from": [5, 20], "to": [55, 20]}]});
    let icc = b"not really a profile".to_vec();

    // Mid-gray, 144 dpi (dpr 2: a 6 px line over rows 17–22), with an ICC profile.
    let gray = dir.join("gray.png");
    let gray_icc = icc.clone();
    let configure = move |info: &mut png::Info<'static>| {
        info.pixel_dims = Some(png::PixelDimensions {
            xppu: 5669,
            yppu: 5669,
            unit: png::Unit::Meter,
        });
        info.icc_profile = Some(gray_icc.into());
    };
    let data = [0x80; 60 * 40];
    fs::write(
        &gray,
        encode(60, 40, png::ColorType::Grayscale, configure, &data),
    )
    .unwrap();
    let out = dir.join("gray-out.png");
    assert_eq!(report(&render(&gray, &out, &spec, &[]))["dpr"], 2.0);
    let image = read_png(&out);
    assert_eq!((image.width, image.height), (60, 40));
    assert_eq!(image.dpi, Some(144.0));
    assert_eq!(image.icc, Some(icc));
    assert_eq!(pixel(&image, 30, 5), [0x80, 0x80, 0x80, 0xFF]);
    assert_eq!(pixel(&image, 30, 20), RED);

    // Index 0 transparent (left half), index 1 opaque green (right half), no pHYs (dpr 1: a
    // 3 px line over rows 18–20).
    let palette = dir.join("palette.png");
    let configure = |info: &mut png::Info<'static>| {
        info.palette = Some(vec![0, 0, 0xFF, 0, 0xFF, 0].into());
        info.trns = Some(vec![0].into());
    };
    let data: Vec<u8> = (0..40)
        .flat_map(|_| (0..60).map(|x| u8::from(x >= 30)))
        .collect();
    fs::write(
        &palette,
        encode(60, 40, png::ColorType::Indexed, configure, &data),
    )
    .unwrap();
    let out = dir.join("palette-out.png");
    assert_eq!(report(&render(&palette, &out, &spec, &[]))["dpr"], 1.0);
    let image = read_png(&out);
    assert_eq!(pixel(&image, 10, 5)[3], 0, "transparent stays transparent");
    assert_eq!(pixel(&image, 50, 5), [0, 0xFF, 0, 0xFF]);
    assert_eq!(pixel(&image, 10, 20), RED, "over transparent");
    assert_eq!(pixel(&image, 50, 20), RED, "over green");
}

#[test]
fn zero_length_line_warns_and_succeeds() {
    let dir = work_dir("zero_length_line_warns_and_succeeds");
    let input = write_fixture(&dir);
    let out = dir.join("out.png");
    let spec = json!({"annotations": [{"type": "line", "from": [100, 100], "to": [100, 100]}]});
    let output = render(&input, &out, &spec, &[]);
    let report = report(&output);
    let warnings = report["warnings"].as_array().unwrap();
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert_eq!(warnings[0]["id"], "line-1");
    assert_eq!(report["spec"]["annotations"], json!([]));
    assert!(output.stderr.is_empty(), "warnings go only in the report");
    assert!(
        read_png(&out).rgba == common::fixture(600, 400).rgba,
        "nothing is drawn"
    );
}

#[test]
fn line_off_image_is_clipped() {
    let dir = work_dir("line_off_image_is_clipped");
    let input = write_fixture(&dir);
    let out = dir.join("out.png");
    let spec = json!({"annotations": [{"type": "line", "from": [300, 200], "to": [5000, 5000]}]});
    let report = report(&render(&input, &out, &spec, &[]));
    let warnings = report["warnings"].as_array().unwrap();
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert_eq!(warnings[0]["id"], "line-1");
    let image = read_png(&out);
    assert_eq!((image.width, image.height), (600, 400));
    assert_eq!(pixel(&image, 350, 251), RED);
    // The line leaves the image through the bottom edge near x = 495.
    assert_eq!(pixel(&image, 495, 399), RED);
}
