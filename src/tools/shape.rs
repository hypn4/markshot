//! The rect and oval tools: a rounded rect or a squircle, stroked and optionally filled, or
//! hand-drawn: a stroke of varying width around the shape whose outline is filled in the
//! color.
//!
//! `fill` picks the style: `sketch` hand-drawn, `outline` the border alone, `semi` and `solid`
//! the border over the color at `fill_alpha`, by default 0.4 and 1.

mod handdrawn;
mod rounded_rect;
mod squircle;

use tiny_skia::{BlendMode, FillRule, LineCap, LineJoin};

use crate::canvas::{Canvas, Ink, StrokeStyle};
use crate::geom::{PathData, Seg, V2};
use crate::resolve::ResolvedShape;
use crate::rng::rnd100;
use crate::spec::Fill;

pub use handdrawn::hd_corner;
pub use rounded_rect::rounded_rect;
pub use squircle::squircle;

/// Stroke widths in points, by size.
pub const WIDTHS: [f64; 10] = [1.0, 2.0, 3.0, 4.0, 6.0, 8.0, 11.0, 15.0, 21.0, 30.0];

/// The rect's jitter strength while hand-drawn: in the hand-drawn outline and in the plain
/// rect drawn instead on small shapes.
const JITTER: f64 = 1.3;

/// Hand-drawn needs a shorter side over this many px; smaller shapes are drawn plain.
const MIN_HANDDRAWN_SIDE: f64 = 40.0;

/// The opacity of the `semi` and `solid` fills when the spec gives no `fill_alpha`.
const SEMI_FILL_ALPHA: f64 = 0.4;
const SOLID_FILL_ALPHA: f64 = 1.0;

/// Which outline a [`ResolvedShape`] gets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeKind {
    Rect,
    Oval,
}

/// Draws `s` on the main surface. A `sketch` over 40 px on its shorter side is hand-drawn;
/// anything else is the plain outline, filled at `fill_alpha` (by default 0.4 for `semi` and 1
/// for `solid`), then stroked with round joins.
///
/// # Panics
/// When `s.size` is 10 or more; resolved sizes are 0 to 7.
pub fn draw(c: &mut Canvas, s: &ResolvedShape, kind: ShapeKind) {
    let g = Geometry::new(c, s);
    let ink = Ink {
        rgb: s.color,
        alpha: 1.0,
        blend: BlendMode::SourceOver,
    };
    if g.handdrawn {
        let outline = match kind {
            ShapeKind::Rect => handdrawn::rect(g.rect, g.wpt, g.stroke, g.dpr, &g.rnd),
            ShapeKind::Oval => handdrawn::oval(g.rect, g.stroke, g.dpr, s.spotlight, &g.rnd),
        };
        c.main().fill(&outline, ink, FillRule::Winding);
        return;
    }
    let path = g.plain_path(kind, s.fill);
    let fill_alpha = match s.fill {
        Fill::Sketch | Fill::Outline => None,
        Fill::Semi => Some(s.fill_alpha.unwrap_or(SEMI_FILL_ALPHA)),
        Fill::Solid => Some(s.fill_alpha.unwrap_or(SOLID_FILL_ALPHA)),
    };
    if let Some(alpha) = fill_alpha {
        let background = Ink { alpha, ..ink };
        c.main().fill(&path, background, FillRule::Winding);
    }
    let style = StrokeStyle {
        width: g.stroke,
        cap: LineCap::Butt,
        join: LineJoin::Round,
        dash: None,
    };
    c.main().stroke(&path, ink, &style);
}

/// The spotlight hole of `s` in y-up px: the centre line [`draw`] strokes, closed, running
/// counter-clockwise like a spotlight's outline so that overlapping holes merge. A hand-drawn
/// oval's spiral, which then runs past its start, is closed by a straight line between its
/// ends.
///
/// # Panics
/// When `s.size` is 10 or more; resolved sizes are 0 to 7.
#[must_use]
pub fn hole(c: &Canvas, s: &ResolvedShape, kind: ShapeKind) -> PathData {
    let g = Geometry::new(c, s);
    if !g.handdrawn {
        return g.plain_path(kind, s.fill);
    }
    let line = match kind {
        ShapeKind::Rect => handdrawn::rect_line(g.rect, g.wpt, g.dpr, &g.rnd),
        ShapeKind::Oval => handdrawn::oval_line(g.rect, g.stroke, g.dpr, s.spotlight, &g.rnd),
    };
    let mut pos: Vec<V2> = line.iter().map(|p| p.pos).collect();
    if signed_area(&pos) < 0.0 {
        pos.reverse();
    }
    let mut segs: Vec<Seg> = pos.iter().map(|&p| Seg::Line(p)).collect();
    if let Some(Seg::Line(first)) = segs.first().copied() {
        segs[0] = Seg::Move(first);
        segs.push(Seg::Close);
    }
    PathData(segs)
}

/// What [`draw`] and [`hole`] share about `s`: its y-up rect, widths, randomness and whether
/// it is hand-drawn.
struct Geometry {
    rect: (f64, f64, f64, f64),
    dpr: f64,
    /// The size's width in points.
    wpt: f64,
    /// The stroke width in px.
    stroke: f64,
    rnd: [f64; 100],
    /// A `sketch` over 40 px on its shorter side.
    handdrawn: bool,
}

impl Geometry {
    /// The geometry of `s` on `c`.
    ///
    /// # Panics
    /// When `s.size` is 10 or more; resolved sizes are 0 to 7.
    fn new(c: &Canvas, s: &ResolvedShape) -> Self {
        let rect = c.up_rect(s.rect);
        let (_, _, w, h) = rect;
        let wpt = WIDTHS[usize::from(s.size)];
        Self {
            rect,
            dpr: c.dpr,
            wpt,
            stroke: stroke_px(w, h, wpt, c.dpr),
            rnd: rnd100(s.seed),
            handdrawn: s.fill == Fill::Sketch && w.min(h) > MIN_HANDDRAWN_SIDE,
        }
    }

    /// The outline of a shape that is not hand-drawn. Intentional: a small `sketch` rect keeps
    /// the hand-drawn jitter.
    fn plain_path(&self, kind: ShapeKind, fill: Fill) -> PathData {
        let jitter = if fill == Fill::Sketch { JITTER } else { 0.0 };
        plain_path(kind, self.rect, jitter, 2.0 * self.wpt, self.dpr, &self.rnd)
    }
}

/// Twice the signed area of the polygon `pts` (y-up): positive when it runs
/// counter-clockwise.
fn signed_area(pts: &[V2]) -> f64 {
    pts.iter()
        .zip(pts.iter().cycle().skip(1))
        .map(|(a, b)| a.x * b.y - b.x * a.y)
        .sum()
}

/// The outline of `kind` on `rect` when not hand-drawn: the rounded rect with corners of `cp`
/// points, or the squircle.
fn plain_path(
    kind: ShapeKind,
    (x, y, w, h): (f64, f64, f64, f64),
    jitter: f64,
    cp: f64,
    dpr: f64,
    rnd: &[f64; 100],
) -> PathData {
    match kind {
        ShapeKind::Rect => rounded_rect(x, y, w, h, jitter, cp, dpr, rnd),
        ShapeKind::Oval => squircle(x, y, w, h),
    }
}

/// The stroke width in px of a `w × h` shape whose size is `wpt` points wide: `wpt`, at most a
/// tenth of the shorter side, times dpr.
#[must_use]
pub fn stroke_px(w: f64, h: f64, wpt: f64, dpr: f64) -> f64 {
    // Intentional: a tenth of the shorter side in px caps a width in points.
    (w.abs().min(h.abs()) / 10.0).min(wpt) * dpr
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::parse_hex;
    use crate::image_io::{ColorSpace, SourceImage, decode_png};

    const RED: [u8; 4] = [255, 12, 1, 255];
    const WHITE: [u8; 4] = [255; 4];

    /// A size-2 `#FF0C01` shape with seed 7 and no `fill_alpha`.
    fn shape(fill: Fill, rect: [f64; 4]) -> ResolvedShape {
        ResolvedShape {
            id: "rect-1".to_owned(),
            color: parse_hex("#FF0C01").unwrap(),
            size: 2,
            seed: 7,
            rect,
            fill,
            fill_alpha: None,
            spotlight: false,
        }
    }

    /// A white `width` × `height` canvas at dpr 2.
    fn white(width: u32, height: u32) -> Canvas {
        let src = SourceImage {
            width,
            height,
            rgba: vec![255; (width * height * 4) as usize],
            dpi: None,
            icc: None,
            color_space: ColorSpace::Srgb,
        };
        Canvas::new(src, 2.0)
    }

    /// `s` drawn as `kind` on a white `width` × `height` canvas at dpr 2, as PNG.
    fn png(s: &ResolvedShape, kind: ShapeKind, width: u32, height: u32) -> Vec<u8> {
        let mut c = white(width, height);
        draw(&mut c, s, kind);
        c.into_png().unwrap()
    }

    /// The RGBA pixel at column `x`, row `y` (top-down) of `png`, `width` pixels wide.
    fn pixel(png: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
        let rgba = decode_png(png).unwrap().rgba;
        let i = ((y * width + x) * 4) as usize;
        rgba[i..i + 4].try_into().unwrap()
    }

    /// `target`'s plain outline (the rounded rect with jitter `jitter` and 6-point corners, or
    /// the squircle) stroked `width` px in its color with round joins, as a shape that is not
    /// hand-drawn is drawn; on a white canvas as `png` makes it.
    fn plain(
        target: &ResolvedShape,
        kind: ShapeKind,
        jitter: f64,
        width: f64,
        size: (u32, u32),
    ) -> Vec<u8> {
        let mut canvas = white(size.0, size.1);
        let (x, y, w, h) = canvas.up_rect(target.rect);
        let path = match kind {
            ShapeKind::Rect => rounded_rect(x, y, w, h, jitter, 6.0, 2.0, &rnd100(target.seed)),
            ShapeKind::Oval => squircle(x, y, w, h),
        };
        let ink = Ink {
            rgb: target.color,
            alpha: 1.0,
            blend: BlendMode::SourceOver,
        };
        let style = StrokeStyle {
            width,
            cap: LineCap::Butt,
            join: LineJoin::Round,
            dash: None,
        };
        canvas.main().stroke(&path, ink, &style);
        canvas.into_png().unwrap()
    }

    #[test]
    fn stroke_px_caps_small_shapes() {
        assert_eq!(stroke_px(200., 100., 3.0, 2.0), 6.0);
        assert_eq!(stroke_px(20., 100., 3.0, 2.0), 4.0);
        assert_eq!(stroke_px(100., 20., 1.0, 2.0), 2.0);
    }

    #[test]
    fn sketch_falls_back_when_small() {
        // A shorter side of 40 px is not over 40: the sketch is the plain outline, 6 px wide
        // (min(40/10, 3 pt) * 2), the rect still with the hand-drawn jitter.
        let s = shape(Fill::Sketch, [30.0, 20.0, 40.0, 200.0]);
        for kind in [ShapeKind::Rect, ShapeKind::Oval] {
            assert!(
                png(&s, kind, 100, 240) == plain(&s, kind, 1.3, 6.0, (100, 240)),
                "{kind:?}"
            );
        }
    }

    #[test]
    fn sketch_over_40_px_is_hand_drawn() {
        let s = shape(Fill::Sketch, [30.0, 20.0, 41.0, 200.0]);
        for kind in [ShapeKind::Rect, ShapeKind::Oval] {
            assert!(
                png(&s, kind, 100, 240) != plain(&s, kind, 1.3, 6.0, (100, 240)),
                "{kind:?}"
            );
        }
    }

    #[test]
    fn outline_is_stroked_centred_on_the_edges() {
        // 160 × 60 at size 2: a 6 px stroke, so the rect's left edge at x = 20 covers columns
        // 17–22 (mid-height, below the 12 px corners); the squircle's is round there, so only
        // its inner columns are certain. Nothing inside.
        let s = shape(Fill::Outline, [20.0, 20.0, 160.0, 60.0]);
        for (kind, red, white) in [
            (ShapeKind::Rect, 17..=22, [16, 23, 100]),
            (ShapeKind::Oval, 18..=21, [15, 24, 100]),
        ] {
            let png = png(&s, kind, 200, 100);
            for x in red {
                assert_eq!(pixel(&png, 200, x, 50), RED, "{kind:?} column {x}");
            }
            for x in white {
                assert_eq!(pixel(&png, 200, x, 50), WHITE, "{kind:?} column {x}");
            }
        }
    }

    #[test]
    fn semi_and_solid_fill_inside_the_border() {
        // #FF0C01 at 0.4 over white is (255, 157.8, 153.4), so (255, 158, 153).
        for (fill, want) in [
            (Fill::Outline, WHITE),
            (Fill::Semi, [255, 158, 153, 255]),
            (Fill::Solid, RED),
        ] {
            let s = shape(fill, [20.0, 20.0, 160.0, 60.0]);
            for kind in [ShapeKind::Rect, ShapeKind::Oval] {
                let png = png(&s, kind, 200, 100);
                let got = pixel(&png, 200, 100, 50);
                assert!(
                    got.iter().zip(want).all(|(&g, w)| g.abs_diff(w) <= 1),
                    "{fill:?} {kind:?}: {got:?}"
                );
                assert_eq!(pixel(&png, 200, 20, 50), RED, "{fill:?} {kind:?} border");
            }
        }
    }

    #[test]
    fn fill_alpha_sets_the_background_opacity() {
        // Solid at 0.5: (255, 133.5, 128).
        let mut s = shape(Fill::Solid, [20.0, 20.0, 160.0, 60.0]);
        s.fill_alpha = Some(0.5);
        let got = pixel(&png(&s, ShapeKind::Rect, 200, 100), 200, 100, 50);
        assert!(
            got.iter()
                .zip([255, 134, 128, 255])
                .all(|(&g, w)| g.abs_diff(w) <= 1),
            "{got:?}"
        );
    }

    /// Whether row `y` of `png` (`width` wide) has a pixel of the color between columns
    /// `x0` and `x1`.
    fn red_in_row(png: &[u8], width: u32, y: u32, x0: u32, x1: u32) -> bool {
        (x0..=x1).any(|x| pixel(png, width, x, y) == RED)
    }

    /// Whether column `x` of `png` (`width` wide) has a pixel of the color between rows `y0`
    /// and `y1`.
    fn red_in_column(png: &[u8], width: u32, x: u32, y0: u32, y1: u32) -> bool {
        (y0..=y1).any(|y| pixel(png, width, x, y) == RED)
    }

    #[test]
    fn sketch_rect_is_a_hollow_stroke_along_the_edges() {
        // 300 × 200: a 6 px stroke 4.4–7.6 px wide, shifted at most 1 px, edges bowed under
        // 1 px; corners of 37 px, whose curve passes 37/8 px inside the sharp corner.
        let png = png(
            &shape(Fill::Sketch, [50.0, 50.0, 300.0, 200.0]),
            ShapeKind::Rect,
            400,
            300,
        );
        assert!(red_in_row(&png, 400, 150, 47, 53), "left edge");
        assert!(red_in_row(&png, 400, 150, 347, 353), "right edge");
        assert!(red_in_column(&png, 400, 200, 47, 53), "top edge");
        assert!(red_in_column(&png, 400, 200, 247, 253), "bottom edge");
        for (x, y) in [
            (200, 150),
            (40, 150),
            (60, 150),
            (200, 40),
            (200, 60),
            (49, 49),
        ] {
            assert_eq!(pixel(&png, 400, x, y), WHITE, "({x}, {y})");
        }
    }

    #[test]
    fn sketch_oval_is_a_ring_through_the_mid_edges() {
        // 300 × 200, stroke 6 px under a fifth of 200: a spiral whose distance drifts ±7.8 px
        // around the rect's mid-edges.
        let png = png(
            &shape(Fill::Sketch, [50.0, 50.0, 300.0, 200.0]),
            ShapeKind::Oval,
            400,
            300,
        );
        assert!(red_in_row(&png, 400, 150, 40, 60), "left");
        assert!(red_in_row(&png, 400, 150, 340, 360), "right");
        assert!(red_in_column(&png, 400, 200, 40, 60), "top");
        assert!(red_in_column(&png, 400, 200, 240, 260), "bottom");
        for (x, y) in [(200, 150), (60, 60), (340, 60), (60, 240), (340, 240)] {
            assert_eq!(pixel(&png, 400, x, y), WHITE, "({x}, {y})");
        }
    }

    #[test]
    fn sketch_is_seed_deterministic() {
        for kind in [ShapeKind::Rect, ShapeKind::Oval] {
            let mut s = shape(Fill::Sketch, [50.0, 50.0, 300.0, 200.0]);
            let first = png(&s, kind, 400, 300);
            assert!(png(&s, kind, 400, 300) == first, "{kind:?}");
            s.seed = 8;
            assert!(png(&s, kind, 400, 300) != first, "{kind:?}");
        }
    }
}
