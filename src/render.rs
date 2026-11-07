//! The render pipeline: resolve the spec, then draw in a fixed z-order (see [`render`]).

use serde::Serialize;

use crate::canvas::Canvas;
use crate::error::Error;
use crate::image_io::{ColorSpace, SourceImage, dpr_from_dpi};
use crate::resolve::{Resolved, Warning, resolve_on};
use crate::spec::Spec;
use crate::tools;
use crate::tools::shape::ShapeKind;

/// What `render` prints for the agent.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Report {
    pub width: u32,
    pub height: u32,
    /// The dpr used.
    pub dpr: f64,
    /// The resolved spec ([`crate::resolve::Resolution::to_spec`]): every annotation drawn,
    /// its fields filled in. Rendering it again draws the same image.
    pub spec: Spec,
    pub warnings: Vec<Warning>,
}

/// Draws `spec` on `src` and encodes the result as PNG, keeping the source's DPI and ICC
/// profile. The order is fixed: ① every blur, ② the spotlight curtain, ③ everything else in
/// spec order, spotlight borders included. With a `crop` only that part is encoded; the report
/// still gives the whole image's size. A profile other than sRGB or Display P3 is treated
/// as sRGB, with a warning. A PNG without DPI also gets a warning when neither `dpr_override`
/// nor the spec gives the dpr, since its default of 1 is wrong for a high-density capture.
///
/// # Errors
/// [`Error::Spec`] when the spec does not resolve, [`Error::Png`] when encoding fails.
///
/// # Panics
/// When `src` is empty or `src.rgba` is not `width × height × 4` bytes.
pub fn render(
    src: SourceImage,
    spec: &Spec,
    dpr_override: Option<f64>,
) -> Result<(Vec<u8>, Report), Error> {
    let (width, height) = (src.width, src.height);
    let resolution = resolve_on(
        spec,
        width,
        height,
        &src.rgba,
        dpr_from_dpi(src.dpi),
        dpr_override,
    )?;
    let mut warnings = Vec::new();
    if src.dpi.is_none() && dpr_override.is_none() && spec.dpr.is_none() {
        warnings.push(Warning {
            id: "image".to_owned(),
            message: "the PNG has no DPI, so dpr is 1; set \"dpr\" (2 for a Retina or other \
                      HiDPI capture) to confirm it"
                .to_owned(),
        });
    }
    if src.color_space == ColorSpace::Other {
        warnings.push(Warning {
            id: "image".to_owned(),
            message: "the image's colour profile is not sRGB or Display P3; treated as sRGB"
                .to_owned(),
        });
    }
    let mut canvas = Canvas::new(src, resolution.dpr);
    let items = &resolution.items;
    for item in items {
        if let Resolved::Blur(b) = item {
            tools::blur::draw(&mut canvas, b);
        }
    }
    let holes: Vec<_> = items
        .iter()
        .filter_map(|item| match item {
            Resolved::Spotlight(s) => Some(tools::spotlight::outline(&canvas, s)),
            Resolved::Rect(s) if s.spotlight => {
                Some(Some(tools::shape::hole(&canvas, s, ShapeKind::Rect)))
            }
            Resolved::Oval(s) if s.spotlight => {
                Some(Some(tools::shape::hole(&canvas, s, ShapeKind::Oval)))
            }
            _ => None,
        })
        .collect();
    tools::spotlight::draw_curtain(&mut canvas, holes, resolution.spotlight_dim);
    for item in items {
        match item {
            Resolved::Blur(_) => {} // Drawn first.
            Resolved::Line(l) => tools::line::draw(&mut canvas, l),
            Resolved::Arrow(a) => tools::arrow::draw(&mut canvas, a),
            Resolved::Rect(s) => tools::shape::draw(&mut canvas, s, ShapeKind::Rect),
            Resolved::Oval(s) => tools::shape::draw(&mut canvas, s, ShapeKind::Oval),
            Resolved::Highlight(h) => tools::highlight::draw(&mut canvas, h),
            Resolved::Spotlight(s) => tools::spotlight::draw_border(&mut canvas, s),
            Resolved::Counter(k) => tools::counter::draw(&mut canvas, k),
            Resolved::Text(t) => tools::label::draw(&mut canvas, t),
        }
    }
    let resolved = resolution.to_spec();
    let png = match resolution.crop {
        Some(crop) => canvas.into_cropped_png(whole_pixels(crop))?,
        None => canvas.into_png()?,
    };
    warnings.extend(resolution.warnings);
    let report = Report {
        width,
        height,
        dpr: resolution.dpr,
        spec: resolved,
        warnings,
    };
    Ok((png, report))
}

/// A resolved crop, whole non-negative pixels, as integers.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a resolved crop is whole pixels within the image"
)]
fn whole_pixels(rect: [f64; 4]) -> [u32; 4] {
    rect.map(|v| v as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image_io::{ColorSpace, decode_png};

    /// A white `width` × `height` image at 144 dpi.
    fn white(width: u32, height: u32) -> SourceImage {
        SourceImage {
            width,
            height,
            rgba: vec![255; (width * height * 4) as usize],
            dpi: Some(144.0),
            icc: None,
            color_space: ColorSpace::Srgb,
        }
    }

    /// A `width` × `height` image at 144 dpi whose pixel `(x, y)` is `color(x, y)`.
    fn image(width: u32, height: u32, color: impl Fn(u32, u32) -> [u8; 3]) -> SourceImage {
        let rgba = (0..height)
            .flat_map(|y| (0..width).map(move |x| (x, y)))
            .flat_map(|(x, y)| {
                let [r, g, b] = color(x, y);
                [r, g, b, 255]
            })
            .collect();
        SourceImage {
            rgba,
            ..white(width, height)
        }
    }

    /// Renders `annotations` (JSON objects) on a white 100×100 image.
    fn render_annotations(annotations: &[&str]) -> Result<(Vec<u8>, Report), Error> {
        let json = format!(r#"{{"annotations": [{}]}}"#, annotations.join(", "));
        render(white(100, 100), &Spec::from_json(&json).unwrap(), None)
    }

    #[test]
    fn blur_rect_clamped_to_image() {
        // Only the 20 × 20 inside the image is erased, between that part's own border pixels.
        // R is 200 in column 99 and G 255 in row 80, so each row ramps R from 0 to 200 in steps
        // of 10 at its own G; past the image it would ramp to black.
        let color = |x: u32, y: u32| [u8::from(x == 99) * 200, u8::from(y == 80) * 255, 0];
        let spec = r#"{"annotations": [
            {"type": "blur", "mode": "erase", "rect": [80, 80, 50, 50]}
        ]}"#;
        let src = image(100, 100, color);
        let (png, report) = render(src, &Spec::from_json(spec).unwrap(), None).unwrap();
        assert_eq!(
            report.warnings,
            [Warning {
                id: "blur-1".to_owned(),
                message: "partly outside the 100×100 image; drawn clipped".to_owned(),
            }]
        );
        let rgba = decode_png(&png).unwrap().rgba;
        for (i, &[r, g, b, a]) in rgba.as_chunks::<4>().0.iter().enumerate() {
            let i = u32::try_from(i).unwrap();
            let (x, y) = (i % 100, i / 100);
            let want = if x >= 80 && y >= 80 {
                [u8::try_from(10 * (x - 80) + 5).unwrap(), color(x, y)[1], 0]
            } else {
                color(x, y)
            };
            assert_eq!([r, g, b, a], [want[0], want[1], want[2], 255], "({x}, {y})");
        }
    }

    #[test]
    fn blur_samples_original_not_annotations() {
        // Blurs are drawn first, from the original image, so a line listed before a blur
        // still shows on top of it.
        let line = r#"{"type": "line", "from": [10, 50], "to": [90, 50]}"#;
        let blur = r#"{"type": "blur", "rect": [20, 30, 60, 40]}"#;
        let (before, _) = render_annotations(&[line, blur]).unwrap();
        let (after, _) = render_annotations(&[blur, line]).unwrap();
        assert!(before == after, "the order of a line and a blur matters");
        let rgba = decode_png(&before).unwrap().rgba;
        let center = (50 * 100 + 50) * 4;
        assert_eq!(rgba[center..center + 4], [255, 12, 1, 255]);
    }

    #[test]
    fn later_annotations_are_drawn_on_top() {
        let red = r##"{"type": "line", "color": "#FF0000", "from": [10, 50], "to": [90, 50]}"##;
        let blue = r##"{"type": "line", "color": "#0000FF", "from": [50, 10], "to": [50, 90]}"##;
        for (annotations, want) in [
            ([red, blue], [0, 0, 255, 255]),
            ([blue, red], [255, 0, 0, 255]),
        ] {
            let (png, _) = render_annotations(&annotations).unwrap();
            let rgba = decode_png(&png).unwrap().rgba;
            let crossing = (50 * 100 + 50) * 4;
            assert_eq!(rgba[crossing..crossing + 4], want, "{annotations:?}");
        }
    }

    #[test]
    fn draws_on_a_one_pixel_image() {
        // Length √2 at dpr 2: a stroke 0.28 px wide (a tenth of the length) along the pixel's
        // diagonal, which partly covers it.
        let json = r#"{"annotations": [{"type": "line", "from": [0, 0], "to": [1, 1]}]}"#;
        let (png, report) = render(white(1, 1), &Spec::from_json(json).unwrap(), None).unwrap();
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);
        let [r, g, b, a] = decode_png(&png).unwrap().rgba[..] else {
            panic!("one pixel");
        };
        assert_eq!((r, a), (255, 255));
        assert!((13..255).contains(&g) && (2..255).contains(&b), "{g} {b}");
    }

    #[test]
    fn zero_size_rect_skipped() {
        // A rect or oval without width or height is left out with a warning, drawing nothing,
        // while the rest of the spec is drawn.
        let (png, report) = render_annotations(&[
            r#"{"type": "rect", "rect": [10, 10, 0, 50]}"#,
            r#"{"type": "oval", "rect": [10, 80, 50, 0]}"#,
            r#"{"type": "rect", "fill": "solid", "rect": [70, 10, 20, 20]}"#,
        ])
        .unwrap();
        let messages: Vec<&str> = report.warnings.iter().map(|w| w.message.as_str()).collect();
        assert_eq!(
            messages, ["zero width or height after rounding to pixels; skipped"; 2],
            "{report:?}"
        );
        assert_eq!(report.spec.annotations.len(), 1);
        let rgba = decode_png(&png).unwrap().rgba;
        let at = |x: usize, y: usize| &rgba[(y * 100 + x) * 4..][..4];
        for (x, y) in [(10, 10), (10, 35), (9, 59), (35, 80), (59, 79)] {
            assert_eq!(at(x, y), [255; 4], "({x}, {y})");
        }
        assert_eq!(at(80, 20), [255, 12, 1, 255]);
    }

    #[test]
    fn dim_applies_over_blur_not_over_line() {
        // ① blurs, ② the curtain, ③ the rest: outside the hole, the curtain at the spec's dim
        // (0.5) halves the erased area (±1), while a line listed before the spotlight is still
        // drawn over the curtain in its own color.
        let color = |x: u32, y: u32| [x, y, 45].map(|v| u8::try_from(2 * v).unwrap());
        let line = r#"{"type": "line", "from": [10, 90], "to": [90, 90]}"#;
        let blur = r#"{"type": "blur", "mode": "erase", "rect": [10, 10, 40, 40]}"#;
        let spotlight = r#"{"type": "spotlight", "rect": [60, 10, 30, 30]}"#;
        let draw = |annotations: &[&str]| {
            let json = format!(
                r#"{{"spotlight_dim": 0.5, "annotations": [{}]}}"#,
                annotations.join(", ")
            );
            let src = image(100, 100, color);
            let (png, _) = render(src, &Spec::from_json(&json).unwrap(), None).unwrap();
            decode_png(&png).unwrap().rgba
        };
        let plain = draw(&[line, blur]);
        let dimmed = draw(&[line, spotlight, blur]);
        let at = |rgba: &[u8], x: usize, y: usize| -> [u8; 4] {
            rgba[(y * 100 + x) * 4..][..4].try_into().unwrap()
        };
        for (x, y) in [(20, 20), (30, 30), (40, 45)] {
            let (before, after) = (at(&plain, x, y), at(&dimmed, x, y));
            assert!(
                before[..3]
                    .iter()
                    .zip(&after[..3])
                    .all(|(&b, &a)| a.abs_diff(b / 2) <= 1),
                "({x}, {y}): {before:?} → {after:?}"
            );
        }
        assert_eq!(at(&dimmed, 50, 90), [255, 12, 1, 255]);
        assert_eq!(at(&dimmed, 50, 90), at(&plain, 50, 90));
    }

    #[test]
    fn spotlight_border_is_drawn_in_spec_order() {
        // The border is drawn with the other annotations, after the curtain: over a line
        // listed before the spotlight, under one listed after. The 6 px border of the left
        // edge, x = 30, crosses the line at (30, 50).
        let line = r##"{"type": "line", "color": "#0000FF", "from": [10, 50], "to": [90, 50]}"##;
        let spotlight = r#"{"type": "spotlight", "rect": [30, 20, 40, 60]}"#;
        for (annotations, want) in [
            ([line, spotlight], [255, 12, 1, 255]),
            ([spotlight, line], [0, 0, 255, 255]),
        ] {
            let (png, _) = render_annotations(&annotations).unwrap();
            let rgba = decode_png(&png).unwrap().rgba;
            let crossing = (50 * 100 + 30) * 4;
            assert_eq!(rgba[crossing..crossing + 4], want, "{annotations:?}");
        }
    }

    #[test]
    fn spotlight_partly_outside_is_drawn_clipped() {
        // The hole is cut where it is inside the image, with a warning.
        let spotlight = r#"{"type": "spotlight", "size": 0, "rect": [60, 60, 80, 80]}"#;
        let (png, report) = render_annotations(&[spotlight]).unwrap();
        assert_eq!(
            report.warnings,
            [Warning {
                id: "spotlight-1".to_owned(),
                message: "partly outside the 100×100 image; drawn clipped".to_owned(),
            }]
        );
        let rgba = decode_png(&png).unwrap().rgba;
        let at = |x: usize, y: usize| &rgba[(y * 100 + x) * 4..][..4];
        assert_eq!(at(80, 80), [255; 4]);
        assert_eq!(at(99, 99), [255; 4]);
        // Outside, black at 0.214 leaves round(255 * (1 − 0.214)) = 200, ±1.
        let dimmed = at(30, 30);
        assert!(
            dimmed[..3].iter().all(|&v| v.abs_diff(200) <= 1) && dimmed[3] == 255,
            "{dimmed:?}"
        );
    }

    #[test]
    fn a_frame_spotlight_meets_its_line() {
        // A frame with `"spotlight": true` cuts its hole along its own line, so the dimming
        // ends under the border: on mid-grey (128, dimmed to round(128 * 200/255) = 100), each
        // row and column through the middle is dimmed up to the first border pixel from the
        // image edge and undimmed up to the first border pixel from the centre. Hand-drawn
        // lines wobble with the seed, so several seeds are checked. With `spotlight` a
        // hand-drawn oval's spiral always runs past its start, so no seed leaves a gap for the
        // hole's closing line to show through.
        let (w, h) = (300_usize, 200_usize);
        let (cx, cy) = (150, 100);
        let ovals = (0..12_u64).map(|seed| {
            format!(r#"{{"type": "oval", "seed": {seed}, "rect": [40, 40, 220, 120]}}"#)
        });
        let rects = (0..8_u64).map(|seed| {
            format!(r#"{{"type": "rect", "seed": {seed}, "rect": [40, 40, 220, 120]}}"#)
        });
        let frames = ovals.chain(rects).chain([
            r#"{"type": "oval", "fill": "outline", "rect": [40, 40, 220, 120]}"#.to_owned(),
            r#"{"type": "rect", "fill": "outline", "rect": [40, 40, 220, 120]}"#.to_owned(),
        ]);
        for frame in frames {
            let frame = frame.replace(r#""rect": ["#, r#""spotlight": true, "rect": ["#);
            let json = format!(r#"{{"annotations": [{frame}]}}"#);
            let src = image(300, 200, |_, _| [128; 3]);
            let (png, _) = render(src, &Spec::from_json(&json).unwrap(), None).unwrap();
            let rgba = decode_png(&png).unwrap().rgba;
            let at = |x: usize, y: usize| -> [u8; 3] {
                rgba[(y * w + x) * 4..][..3].try_into().unwrap()
            };
            let border = |p: [u8; 3]| p[0] != p[1] || p[1] != p[2];
            // Walks from `from` toward `to` until the border, asserting `want` on the way.
            let walk = |path: &mut dyn Iterator<Item = (usize, usize)>, want: u8, what: &str| {
                for (x, y) in path {
                    let p = at(x, y);
                    if border(p) {
                        return;
                    }
                    assert_eq!(p, [want; 3], "{frame}: {what} at ({x}, {y})");
                }
                panic!("{frame}: {what}: no border");
            };
            for y in 70..=130 {
                walk(&mut (0..cx).map(|x| (x, y)), 100, "from the left");
                walk(&mut (cx..w).rev().map(|x| (x, y)), 100, "from the right");
                walk(&mut (0..cx).rev().map(|x| (x, y)), 128, "centre to left");
                walk(&mut (cx..w).map(|x| (x, y)), 128, "centre to right");
            }
            for x in 80..=220 {
                walk(&mut (0..cy).map(|y| (x, y)), 100, "from the top");
                walk(&mut (cy..h).rev().map(|y| (x, y)), 100, "from the bottom");
                walk(&mut (0..cy).rev().map(|y| (x, y)), 128, "centre to top");
                walk(&mut (cy..h).map(|y| (x, y)), 128, "centre to bottom");
            }
        }
    }

    #[test]
    fn frame_and_spotlight_holes_merge() {
        // The frame's hand-drawn hole runs the same way as a spotlight's outline, so where the
        // oval over [40, 40]–[160, 140] and the spotlight over [120, 60]–[240, 140] overlap,
        // around (140, 100), the image is cut once and stays white; outside both it is dimmed
        // to round(255 * 200/255) = 200.
        let json = r#"{"annotations": [
            {"type": "oval", "spotlight": true, "rect": [40, 40, 120, 100]},
            {"type": "spotlight", "size": 0, "rect": [120, 60, 120, 80]}
        ]}"#;
        let (png, _) = render(white(300, 200), &Spec::from_json(json).unwrap(), None).unwrap();
        let rgba = decode_png(&png).unwrap().rgba;
        let at =
            |x: usize, y: usize| -> [u8; 4] { rgba[(y * 300 + x) * 4..][..4].try_into().unwrap() };
        for (x, y) in [(80, 90), (140, 100), (200, 100)] {
            assert_eq!(at(x, y), [255; 4], "({x}, {y})");
        }
        for (x, y) in [(10, 10), (280, 20), (200, 180)] {
            assert_eq!(at(x, y), [200, 200, 200, 255], "({x}, {y})");
        }
    }
}
