//! the reference app's Spotlight tool (Spotlight.md): a black curtain over the image with a hole for
//! every spotlight, and each spotlight's border. The curtain is drawn once, after the blurs
//! and before every other annotation, so it dims the image and the blurs but no annotation;
//! the borders are drawn with the annotations, in spec order. A hole is the outline its
//! border is stroked along: the reference app's rounded rect (RectOval.md §3.2) or squircle (§3.3).

use tiny_skia::{BlendMode, FillRule, LineCap, LineJoin};

use crate::canvas::{Canvas, Ink, StrokeStyle};
use crate::color::Rgb;
use crate::geom::{PathData, Seg, V2};
use crate::resolve::ResolvedSpotlight;
use crate::spec::SpotShape;
use crate::tools::shape::{rounded_rect, squircle, stroke_px};

/// Border widths in points, by size: the shape tools' widths, except that size 0 has no
/// border.
pub const WIDTHS: [f64; 10] = [0.0, 2.0, 3.0, 4.0, 6.0, 8.0, 11.0, 15.0, 21.0, 30.0];

const BLACK: Rgb = Rgb {
    r: 0.0,
    g: 0.0,
    b: 0.0,
};

/// The outline of `s` in y-up px (Spotlight.md §3.3): the edge of its hole and the centre line
/// of its border. A rect gets unjittered corners of `min(cp·dpr, md/3)` px, `cp` being 12 at
/// size 0 and twice the border width in points otherwise; an oval is the squircle. `None` for
/// an empty rect, which the reference app gives no hole.
///
/// # Panics
/// When `s.size` is 10 or more; resolved sizes are 0 to 7.
#[must_use]
pub fn outline(c: &Canvas, s: &ResolvedSpotlight) -> Option<PathData> {
    let (left, bottom, width, height) = c.up_rect(s.rect);
    if width == 0.0 && height == 0.0 {
        return None;
    }
    Some(match s.shape {
        SpotShape::Rect => {
            let w0 = WIDTHS[usize::from(s.size)];
            let cp = if w0 == 0.0 { 12.0 } else { 2.0 * w0 };
            // A spotlight is never hand-drawn: without jitter the draws go unused.
            rounded_rect(left, bottom, width, height, 0.0, cp, c.dpr, &[0.0; 100])
        }
        SpotShape::Oval => squircle(left, bottom, width, height),
    })
}

/// Draws the curtain (Spotlight.md §3.1–3.2): black at `dim` over the whole image, its corners
/// square as without a backdrop, with the union of the spotlights' outlines cut out. Without
/// spotlights there is none.
///
/// # Panics
/// When a spotlight's size is 10 or more; resolved sizes are 0 to 7.
pub fn draw_curtain(c: &mut Canvas, spots: &[&ResolvedSpotlight], dim: f64) {
    if spots.is_empty() {
        return;
    }
    let mut curtain = c.new_layer();
    let ink = Ink {
        rgb: BLACK,
        alpha: dim,
        blend: BlendMode::SourceOver,
    };
    // reference: the layer keeps 8-bit alpha, round(dim·255)/255, as the reference app saves it: 55/255
    // for the default 0.214, so that outside the holes every value v becomes round(v·200/255)
    // (reference/reference-app/cases/spotlight-*.png against base.png).
    curtain.fill(&whole(c), ink, FillRule::Winding);
    // Rects and squircles both run counter-clockwise, so where holes overlap they wind twice
    // and the nonzero rule cuts their union, as the reference app does on macOS 13 and later.
    let holes = spots.iter().filter_map(|s| outline(c, s));
    let clear = Ink {
        alpha: 1.0,
        blend: BlendMode::Clear,
        ..ink
    };
    curtain.fill(
        &PathData(holes.flat_map(|hole| hole.0).collect()),
        clear,
        FillRule::Winding,
    );
    c.composite(&curtain);
}

/// Draws the border of `s` (Spotlight.md §3.3): its outline stroked in its color with butt caps
/// and round joins, `min(w0, md/10)·dpr` px wide like a shape's, so none at size 0.
///
/// # Panics
/// When `s.size` is 10 or more; resolved sizes are 0 to 7.
pub fn draw_border(c: &mut Canvas, s: &ResolvedSpotlight) {
    let Some(path) = outline(c, s) else {
        return;
    };
    let (_, _, w, h) = c.up_rect(s.rect);
    let ink = Ink {
        rgb: s.color,
        alpha: 1.0,
        blend: BlendMode::SourceOver,
    };
    let style = StrokeStyle {
        width: stroke_px(w, h, WIDTHS[usize::from(s.size)], c.dpr),
        cap: LineCap::Butt,
        join: LineJoin::Round,
        dash: None,
    };
    c.main().stroke(&path, ink, &style);
}

/// The whole canvas as a closed path.
fn whole(c: &Canvas) -> PathData {
    let (w, h) = (f64::from(c.width), f64::from(c.height));
    let v = |x, y| V2 { x, y };
    PathData(vec![
        Seg::Move(v(0.0, 0.0)),
        Seg::Line(v(w, 0.0)),
        Seg::Line(v(w, h)),
        Seg::Line(v(0.0, h)),
        Seg::Close,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::parse_hex;
    use crate::image_io::{ColorSpace, SourceImage, decode_png};

    const WHITE: [u8; 4] = [255; 4];
    const RED: [u8; 4] = [255, 12, 1, 255];
    /// The width of [`white`].
    const WIDTH: usize = 200;

    /// A size-`size` `#FF0C01` spotlight over the y-down `rect`.
    fn spot(rect: [f64; 4], size: u8, shape: SpotShape) -> ResolvedSpotlight {
        ResolvedSpotlight {
            id: "spotlight-1".to_owned(),
            color: parse_hex("#FF0C01").unwrap(),
            size,
            rect,
            shape,
        }
    }

    /// A `width` × `height` canvas at dpr 2 whose pixel `(x, y)` is the grey `value(x, y)`.
    fn grey(width: u32, height: u32, value: impl Fn(u32, u32) -> u8) -> Canvas {
        let rgba = (0..height)
            .flat_map(|y| (0..width).map(move |x| (x, y)))
            .flat_map(|(x, y)| {
                let v = value(x, y);
                [v, v, v, 255]
            })
            .collect();
        let src = SourceImage {
            width,
            height,
            rgba,
            dpi: None,
            icc: None,
            color_space: ColorSpace::Srgb,
        };
        Canvas::new(src, 2.0)
    }

    /// A white 200 × 120 canvas at dpr 2.
    fn white() -> Canvas {
        grey(200, 120, |_, _| 255)
    }

    /// `c`'s pixels as saved, rows top-down.
    fn pixels(c: Canvas) -> Vec<[u8; 4]> {
        let rgba = decode_png(&c.into_png().unwrap()).unwrap().rgba;
        rgba.as_chunks::<4>().0.to_vec()
    }

    /// Asserts that `px` is white under black at 0.214: round(255·(1 − 0.214)) = 200, ±1.
    fn assert_dimmed(px: [u8; 4], at: (usize, usize)) {
        assert!(
            px[..3].iter().all(|&v| v.abs_diff(200) <= 1) && px[3] == 255,
            "{at:?}: {px:?} is not dimmed white"
        );
    }

    #[test]
    fn hole_is_undimmed_and_outside_is_dimmed() {
        // A rect spotlight over [40, 30]–[140, 90] on white: its centre stays white, and all
        // around it black at 0.214 dims the image, corners included.
        let s = spot([40.0, 30.0, 100.0, 60.0], 2, SpotShape::Rect);
        let mut c = white();
        draw_curtain(&mut c, &[&s], 0.214);
        let px = pixels(c);
        assert_eq!(px[60 * WIDTH + 90], WHITE);
        for (x, y) in [(20, 60), (160, 60), (90, 10), (90, 110), (0, 0), (199, 119)] {
            assert_dimmed(px[y * WIDTH + x], (x, y));
        }
    }

    #[test]
    fn overlapping_holes_merge() {
        // Rects over [20, 20]–[100, 80] and [60, 50]–[140, 110] overlap in [60, 50]–[100, 80];
        // an oval over [120, 10]–[180, 70] overlaps the second rect around (135, 55). Each part
        // of the union is cut once, overlaps included; between and around the holes the image
        // stays dimmed.
        let spots = [
            spot([20.0, 20.0, 80.0, 60.0], 2, SpotShape::Rect),
            spot([60.0, 50.0, 80.0, 60.0], 2, SpotShape::Rect),
            spot([120.0, 10.0, 60.0, 60.0], 2, SpotShape::Oval),
        ];
        let mut c = white();
        draw_curtain(&mut c, &spots.iter().collect::<Vec<_>>(), 0.214);
        let px = pixels(c);
        for (x, y) in [(40, 35), (80, 65), (100, 95), (135, 55), (160, 30)] {
            assert_eq!(px[y * WIDTH + x], WHITE, "({x}, {y})");
        }
        for (x, y) in [(10, 10), (110, 30), (190, 5), (10, 110), (190, 110)] {
            assert_dimmed(px[y * WIDTH + x], (x, y));
        }
    }

    #[test]
    fn size_zero_has_no_border() {
        // At size 1 the border of the left edge of [40, 30]–[140, 90] is 4 px wide (2 pt at
        // dpr 2), over columns 38–41. At size 0 there is none: every pixel stays as the curtain
        // left it, the hole still cut (white in column 41, dimmed in column 38).
        let rect = [40.0, 30.0, 100.0, 60.0];
        let curtain = |s: &ResolvedSpotlight| {
            let mut c = white();
            draw_curtain(&mut c, &[s], 0.214);
            c
        };
        let thin = spot(rect, 1, SpotShape::Rect);
        let mut c = curtain(&thin);
        draw_border(&mut c, &thin);
        let px = pixels(c);
        for x in 38..=41 {
            assert_eq!(px[60 * WIDTH + x], RED, "size 1, column {x}");
        }
        let none = spot(rect, 0, SpotShape::Rect);
        let mut c = curtain(&none);
        draw_border(&mut c, &none);
        let px = pixels(c);
        assert!(px == pixels(curtain(&none)), "size 0 has a border");
        assert_eq!(px[60 * WIDTH + 41], WHITE);
        assert_dimmed(px[60 * WIDTH + 38], (38, 60));
    }

    #[test]
    fn rect_radius() {
        // Corners of min(((w0 == 0) ? 12 : 2·w0)·dpr, md/3) px (Spotlight.md §6) at dpr 2: a
        // 200 × 100 rect at size 2 (w0 = 3 pt) gets min(12, 33.3) = 12, at size 0 min(24, 33.3)
        // = 24, and a 200 × 30 rect at size 2 gets 30/3 = 10. The outline starts on the bottom
        // edge (y-up), a corner in from the left.
        let c = white();
        for (size, h, r) in [(2, 100.0, 12.0), (0, 100.0, 24.0), (2, 30.0, 10.0)] {
            let s = spot([0.0, 10.0, 200.0, h], size, SpotShape::Rect);
            let start = outline(&c, &s).map(|path| path.0[0]);
            let Some(Seg::Move(V2 { x, y })) = start else {
                panic!("size {size}, height {h}: starts with {start:?}");
            };
            assert!(
                (x - r).abs() < 1e-9 && (y - (110.0 - h)).abs() < 1e-9,
                "size {size}, height {h}: starts at ({x}, {y})"
            );
        }
    }

    #[test]
    fn empty_rect_has_no_outline() {
        // the reference app cuts no hole for a spotlight without width and height.
        let c = white();
        for shape in [SpotShape::Rect, SpotShape::Oval] {
            let s = spot([50.0, 50.0, 0.0, 0.0], 2, shape);
            assert_eq!(outline(&c, &s), None, "{shape:?}");
        }
    }

    #[test]
    fn dims_as_reference_saves() {
        // reference: the reference app saves the default dim as black at 55/255, so outside the holes
        // every value v becomes round(v·200/255): 255 → 200, 128 → 100, 30 → 24
        // (reference/reference-app/cases/spotlight-*.png against base.png). One column per
        // value; the hole is below the image.
        let mut c = grey(256, 1, |x, _| u8::try_from(x).unwrap());
        let s = spot([0.0, 10.0, 20.0, 20.0], 2, SpotShape::Rect);
        draw_curtain(&mut c, &[&s], 0.214);
        for (v, px) in (0_u32..).zip(pixels(c)) {
            // round(v·200/255): v·200/255 is never halfway between two integers.
            let want = u8::try_from((v * 200 + 127) / 255).unwrap();
            assert_eq!(px, [want, want, want, 255], "value {v}");
        }
    }
}
