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
fn missing_dpi_warns_until_dpr_is_given() {
    // Without a pHYs chunk dpr falls back to 1, which draws everything half as wide as intended
    // on a Retina capture saved without DPI, so the report asks for an explicit dpr. A `dpr`
    // in the spec (as in every resolved spec) or `--dpr` answers it.
    let dir = work_dir("missing_dpi_warns_until_dpr_is_given");
    let src = common::fixture(600, 400);
    let input = dir.join("input.png");
    fs::write(
        &input,
        encode_png(src.width, src.height, &src.rgba, None, None).unwrap(),
    )
    .unwrap();
    let out = dir.join("out.png");
    let line = json!({"type": "line", "from": [40, 60], "to": [560, 340]});
    let warned = json!([{
        "id": "image",
        "message": "the PNG has no DPI, so dpr is 1; set \"dpr\" (2 for a Retina or other \
                    HiDPI capture) to confirm it"
    }]);
    let cases: [(Value, &[&str], &Value); 3] = [
        (json!({"annotations": [line]}), &[], &warned),
        (json!({"dpr": 1, "annotations": [line]}), &[], &json!([])),
        (json!({"annotations": [line]}), &["--dpr", "2"], &json!([])),
    ];
    for (spec, extra, want) in cases {
        let report = report(&render(&input, &out, &spec, extra));
        assert_eq!(&report["warnings"], want, "{spec} {extra:?}");
    }
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

#[test]
fn switching_fill_in_the_report_takes_the_new_fills_opacity() {
    // An agent frames something with an outline, then switches the stdout spec's `fill` to
    // `solid` to redact it: the box must come out opaque, not at an alpha left over from the
    // outline.
    let dir = work_dir("switching_fill_in_the_report_takes_the_new_fills_opacity");
    let input = write_fixture(&dir);
    let out = dir.join("out.png");
    let spec = json!({"annotations": [
        {"type": "rect", "fill": "outline", "color": "#000000", "rect": [100, 20, 200, 80]}
    ]});
    let mut resolved = report(&render(&input, &out, &spec, &[]))["spec"].clone();
    let rect = &mut resolved["annotations"][0];
    assert_eq!(
        rect.get("fill_alpha"),
        None,
        "a default fill_alpha is left out"
    );
    rect["fill"] = json!("solid");
    report(&render(&input, &out, &resolved, &[]));
    assert_eq!(pixel(&read_png(&out), 200, 60), [0, 0, 0, 0xFF]);
}

/// A minimal ICC profile whose `desc` tag (ICC v2 `textDescriptionType`) reads `description`.
fn icc_profile(description: &str) -> Vec<u8> {
    let mut tag = b"desc\0\0\0\0".to_vec();
    tag.extend(u32::try_from(description.len() + 1).unwrap().to_be_bytes());
    tag.extend(description.as_bytes());
    tag.push(0);
    tag.extend([0u8; 79]); // empty Unicode and ScriptCode descriptions
    let mut icc = vec![0u8; 128];
    icc.extend(1u32.to_be_bytes());
    icc.extend(b"desc");
    icc.extend(144u32.to_be_bytes());
    icc.extend(u32::try_from(tag.len()).unwrap().to_be_bytes());
    icc.extend(tag);
    let size = u32::try_from(icc.len()).unwrap();
    icc[..4].copy_from_slice(&size.to_be_bytes());
    icc
}

#[test]
fn other_colour_profiles_warn() {
    // A profile other than sRGB or Display P3 (here a display's own, as macOS tags some
    // captures) is treated as sRGB, with a warning (docs/design.md, "Colour handling"); the
    // profile is kept.
    let dir = work_dir("other_colour_profiles_warn");
    let src = common::fixture(600, 400);
    let spec = json!({"annotations": [{"type": "line", "from": [40, 60], "to": [560, 340]}]});
    for (description, warned) in [
        ("Color LCD", true),
        ("sRGB IEC61966-2.1", false),
        ("Display P3", false),
    ] {
        let icc = icc_profile(description);
        let input = dir.join("input.png");
        let png = encode_png(src.width, src.height, &src.rgba, src.dpi, Some(&icc)).unwrap();
        fs::write(&input, png).unwrap();
        let out = dir.join("out.png");
        let report = report(&render(&input, &out, &spec, &[]));
        let want = if warned {
            json!([{
                "id": "image",
                "message": "the image's colour profile is not sRGB or Display P3; treated as sRGB"
            }])
        } else {
            json!([])
        };
        assert_eq!(report["warnings"], want, "{description}");
        assert_eq!(read_png(&out).icc, Some(icc), "{description}");
    }
}

#[test]
fn crop_writes_only_that_part_and_round_trips() {
    // `"crop": [100, 50, 200, 120]` on the 600×400 fixture writes a 200×120 PNG whose pixels
    // are the full render's from (100, 50), with the DPI kept; the report still describes the
    // whole image, and its resolved spec (with the crop) renders the same bytes again.
    let dir = work_dir("crop_writes_only_that_part_and_round_trips");
    let input = write_fixture(&dir);
    let line = json!({"type": "line", "from": [40, 60], "to": [560, 340]});
    let full = dir.join("full.png");
    report(&render(&input, &full, &json!({"annotations": [line]}), &[]));
    let out = dir.join("out.png");
    let spec = json!({"crop": [100, 50, 200, 120], "annotations": [line]});
    let first = report(&render(&input, &out, &spec, &[]));
    assert_eq!(
        (first["width"].clone(), first["height"].clone()),
        (json!(600), json!(400))
    );
    assert_eq!(first["spec"]["crop"], json!([100.0, 50.0, 200.0, 120.0]));
    let (full, cropped) = (read_png(&full), read_png(&out));
    assert_eq!((cropped.width, cropped.height), (200, 120));
    assert_eq!(cropped.dpi, Some(144.0));
    for (x, y) in [(0, 0), (199, 119), (150, 70), (60, 30)] {
        assert_eq!(
            pixel(&cropped, x, y),
            pixel(&full, x + 100, y + 50),
            "({x}, {y})"
        );
    }
    let again = dir.join("again.png");
    report(&render(&input, &again, &first["spec"], &[]));
    assert!(
        fs::read(&again).unwrap() == fs::read(&out).unwrap(),
        "the PNGs differ"
    );
}
