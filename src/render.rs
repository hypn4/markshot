//! The render pipeline: resolve the spec, then draw in the reference app's z-order (spec §5.6).

use serde::Serialize;

use crate::canvas::Canvas;
use crate::error::Error;
use crate::image_io::{SourceImage, dpr_from_dpi};
use crate::resolve::{Resolved, Warning, resolve};
use crate::spec::Spec;
use crate::tools;

/// What `render` prints for the agent.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Report {
    pub width: u32,
    pub height: u32,
    /// The dpr used.
    pub dpr: f64,
    /// The resolved spec: every annotation drawn, with every field filled in. Rendering it
    /// again draws the same image.
    pub spec: Spec,
    pub warnings: Vec<Warning>,
}

/// Draws `spec` on `src` and encodes the result as PNG, keeping the source's DPI and ICC
/// profile. The order is the reference app's: ① every blur, ② the spotlight curtain, ③ everything else
/// in spec order.
///
/// # Errors
/// [`Error::Spec`] when the spec does not resolve, [`Error::Unsupported`] naming an
/// annotation type that cannot be drawn yet, [`Error::Png`] when encoding fails.
///
/// # Panics
/// When `src` is empty or `src.rgba` is not `width × height × 4` bytes.
pub fn render(
    src: SourceImage,
    spec: &Spec,
    dpr_override: Option<f64>,
) -> Result<(Vec<u8>, Report), Error> {
    let (width, height) = (src.width, src.height);
    let resolution = resolve(spec, width, height, dpr_from_dpi(src.dpi), dpr_override)?;
    let mut canvas = Canvas::new(src, resolution.dpr);
    let items = &resolution.items;
    if items.iter().any(|item| matches!(item, Resolved::Blur(_))) {
        return Err(unsupported("blur"));
    }
    draw_spotlight_curtain(&mut canvas, items, resolution.spotlight_dim)?;
    for item in items {
        match item {
            Resolved::Blur(_) => {} // Drawn first.
            Resolved::Line(l) => tools::line::draw(&mut canvas, l),
            Resolved::Arrow(a) => tools::arrow::draw(&mut canvas, a),
            Resolved::Rect(_) => return Err(unsupported("rect")),
            Resolved::Oval(_) => return Err(unsupported("oval")),
            Resolved::Highlight(h) => tools::highlight::draw(&mut canvas, h),
            Resolved::Spotlight(_) => return Err(unsupported("spotlight")),
            Resolved::Counter(_) => return Err(unsupported("counter")),
        }
    }
    let report = Report {
        width,
        height,
        dpr: resolution.dpr,
        spec: resolution.to_spec(),
        warnings: resolution.warnings,
    };
    Ok((canvas.into_png()?, report))
}

/// Stage ②: black at `dim` over the whole image, with a hole for each spotlight. Without
/// spotlights there is no curtain; spotlights themselves cannot be drawn yet.
fn draw_spotlight_curtain(_c: &mut Canvas, items: &[Resolved], _dim: f64) -> Result<(), Error> {
    if items
        .iter()
        .any(|item| matches!(item, Resolved::Spotlight(_)))
    {
        return Err(unsupported("spotlight"));
    }
    Ok(())
}

fn unsupported(kind: &str) -> Error {
    Error::Unsupported(kind.to_owned())
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

    /// Renders `annotations` (JSON objects) on a white 100×100 image.
    fn render_annotations(annotations: &[&str]) -> Result<(Vec<u8>, Report), Error> {
        let json = format!(r#"{{"annotations": [{}]}}"#, annotations.join(", "));
        render(white(100, 100), &Spec::from_json(&json).unwrap(), None)
    }

    #[test]
    fn tools_not_implemented_yet_are_unsupported() {
        let arrow = r#"{"type": "arrow", "from": [10, 10], "to": [60, 40]}"#;
        let rect = r#"{"type": "rect", "rect": [10, 50, 30, 20]}"#;
        let oval = r#"{"type": "oval", "rect": [50, 50, 30, 20]}"#;
        let counter = r#"{"type": "counter", "center": [80, 60]}"#;
        let blur = r#"{"type": "blur", "rect": [10, 75, 20, 10]}"#;
        let spotlight = r#"{"type": "spotlight", "rect": [60, 5, 30, 30]}"#;
        for (annotations, want) in [
            (vec![rect], "rect"),
            (vec![oval], "oval"),
            (vec![counter], "counter"),
            // Blurs are drawn first and the spotlight curtain next, before everything else.
            (vec![arrow, blur], "blur"),
            (vec![arrow, spotlight], "spotlight"),
            (vec![spotlight, blur], "blur"),
        ] {
            match render_annotations(&annotations).map(|_| ()) {
                Err(Error::Unsupported(kind)) => assert_eq!(kind, want),
                other => panic!("{annotations:?}: expected unsupported {want}, got {other:?}"),
            }
        }
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
}
