//! The arrow tool: a filled outline (`shaped`), a stroked shaft with open chevrons (`line`,
//! `double`) or a hand-drawn stroke with a two-wing head (`sketch`), straight or bent through
//! `via`. `variant` picks the `shaped` outline.

mod curve;
mod line_arrow;
mod profiles;
mod shaped;
mod sketch;

use tiny_skia::{BlendMode, FillRule, LineCap, LineJoin};

use crate::canvas::{Canvas, Ink, StrokeStyle};
use crate::geom::{PathData, V2};
use crate::math::{PI, cos};
use crate::resolve::ResolvedArrow;
use crate::spec::ArrowStyle;
use crate::stroke::{Pt, norm};

pub use curve::via_to_mid;
pub use profiles::{ARR_PROFILES, SIZEK, SVG_PROFILES, WIDTHS};

/// Draws `a` on the main surface: `shaped` and `sketch` filled, `line` and `double` stroked, all
/// in the arrow's color. An arrow without length draws nothing.
///
/// # Panics
/// When `a.size` is 10 or more or `a.variant` 4 or more; resolved values are at most 7 and 3.
pub fn draw(c: &mut Canvas, a: &ResolvedArrow) {
    let (p0, p1) = (c.up(a.from), c.up(a.to));
    if p0 == p1 {
        return;
    }
    let g = Geometry::new(p0, p1, a.size, a.via.map(|via| c.up(via)), c.dpr);
    let ink = Ink {
        rgb: a.color,
        alpha: 1.0,
        blend: BlendMode::SourceOver,
    };
    match a.style {
        ArrowStyle::Shaped => fill(c, &shaped::paths(&g, usize::from(a.variant)), ink),
        ArrowStyle::Sketch => fill(c, &sketch::paths(&g, a.seed), ink),
        ArrowStyle::Line | ArrowStyle::Double => {
            let lw = stroke_width(g.len, a.size, g.dpr);
            let style = StrokeStyle {
                width: lw,
                cap: LineCap::Square,
                join: LineJoin::Miter,
                dash: None,
            };
            let path = line_arrow::path(&g, lw, a.style == ArrowStyle::Double);
            c.main().stroke(&path, ink, &style);
        }
    }
}

/// Fills each path on its own (non-zero). In an opaque color this is the same as filling the
/// union of a bent arrow's band and head.
fn fill(c: &mut Canvas, paths: &[PathData], ink: Ink) {
    for path in paths {
        c.main().fill(path, ink, FillRule::Winding);
    }
}

/// The stroke width in px of an arrow `len` px long: the size's width in points, at most a
/// tenth of the length, times dpr, grown on long arrows by `sqrt((len/dpr + 550)/600)` kept
/// within 1–1.5. At dpr 1, widths over 4 px are rounded.
///
/// # Panics
/// When `size` is 10 or more.
#[must_use]
#[expect(clippy::float_cmp, reason = "the rounding applies at exactly dpr 1")]
pub fn stroke_width(len: f64, size: u8, dpr: f64) -> f64 {
    let grow = ((len / dpr + 550.0) / 600.0).sqrt().clamp(1.0, 1.5);
    let width = (len / 10.0).min(WIDTHS[usize::from(size)]) * dpr * grow;
    if width > 4.0 && dpr == 1.0 {
        width.round()
    } else {
        width
    }
}

/// `(kHead, kTail)`, the scales of the head and the tail of an arrow whose ends are `len0` px
/// apart. Past 520 px the head grows (to 1.25×) as the tail shrinks (to 0.8×); under the
/// size's threshold (200 px up to size 5, `112·SIZEK` above) both shrink with the length.
///
/// # Panics
/// When `size` is 10 or more.
#[must_use]
pub fn head_tail_k(len0: f64, size: u8) -> (f64, f64) {
    let mut k = (1.0, 1.0);
    if len0 > 520.0 {
        let tail = (1.0 - (len0 - 520.0) / 1000.0).max(0.8);
        k = (1.0 / tail, tail);
    }
    let sizek = SIZEK[usize::from(size)];
    let short = if 56.0 * sizek >= 100.0 {
        112.0 * sizek
    } else {
        200.0
    };
    if len0 < short {
        k = (len0 / short, len0 / short);
    }
    k
}

/// What every style draws from: the ends (y-up px), the bend and the length factors.
struct Geometry {
    /// The tail.
    p0: V2,
    /// The tip.
    p1: V2,
    /// `|p1 − p0|`.
    len: f64,
    size: usize,
    dpr: f64,
    /// The curve point M: its angle off the chord (0: straight) and its distance from the tail
    /// in chords.
    mid_angle: f64,
    mid_dist: f64,
    k_head: f64,
    k_tail: f64,
}

impl Geometry {
    /// An arrow from `p0` to `p1` (apart), bent through `via` (all y-up px).
    fn new(p0: V2, p1: V2, size: u8, via: Option<V2>, dpr: f64) -> Geometry {
        let len = norm(p1 - p0);
        let (k_head, k_tail) = head_tail_k(len, size);
        let (mid_angle, mid_dist) = via.map_or((0.0, 0.5), |via| via_to_mid(p0, p1, via, dpr));
        Geometry {
            p0,
            p1,
            len,
            size: usize::from(size),
            dpr,
            mid_angle,
            mid_dist,
            k_head,
            k_tail,
        }
    }
}

/// The taper: each point's length along the stroke, `t` = that over the whole length (kept as
/// the weight), and the width `w0 + (w1 − w0)·ease(t)`. One point is left alone.
fn taper(pts: &mut [Pt], w0: f64, w1: f64, ease_kind: u8) {
    if pts.len() < 2 {
        return;
    }
    let (mut prev, mut total) = (pts[0].pos, pts[0].cum_len);
    for p in pts.iter_mut() {
        total += norm(prev - p.pos);
        p.cum_len = total;
        prev = p.pos;
    }
    for p in pts {
        let t = p.cum_len / total;
        p.weight = t;
        p.width = w0 + (w1 - w0) * ease(ease_kind, t);
    }
}

/// The easing curves the arrows use: 1 thin in the middle, 3 thick to thin, 6 thick–thin–thick
/// (thinnest at 0.8); anything else, as 0, is linear. (Kinds 2, 4 and 5 are unused.)
fn ease(kind: u8, t: f64) -> f64 {
    match kind {
        1 => cos(t * (2.0 * PI)) * 0.5 + 0.5,
        3 => t * t - (t + t) + 1.0,
        6 if t < 0.4 => t * t / -0.32 + 1.0,
        6 if t < 0.8 => (t - 0.8) * (t - 0.8) / 0.32,
        6 if t < 0.9 => (t - 0.8) * (t - 0.8) / 0.02,
        6 => (1.0 - t) * ((1.0 - t) * -50.0) + 1.0,
        _ => t,
    }
}

/// A size-2 `#FF0000` arrow of `style` with seed 42.
#[cfg(test)]
fn resolved(style: ArrowStyle, from: [f64; 2], to: [f64; 2]) -> ResolvedArrow {
    ResolvedArrow {
        id: "arrow-1".to_owned(),
        color: crate::color::parse_hex("#FF0000").unwrap(),
        size: 2,
        seed: 42,
        from,
        to,
        style,
        variant: 0,
        via: None,
    }
}

/// The alpha (coverage) of every pixel after drawing `a` on a transparent canvas, by row.
#[cfg(test)]
fn coverage(width: u32, height: u32, dpr: f64, a: &ResolvedArrow) -> Vec<Vec<u8>> {
    use crate::image_io::{ColorSpace, SourceImage, decode_png};
    let src = SourceImage {
        width,
        height,
        rgba: vec![0; (width * height * 4) as usize],
        dpi: None,
        icc: None,
        color_space: ColorSpace::Srgb,
    };
    let mut c = Canvas::new(src, dpr);
    draw(&mut c, a);
    decode_png(&c.into_png().unwrap())
        .unwrap()
        .rgba
        .as_chunks::<4>()
        .0
        .chunks(width as usize)
        .map(|row| row.iter().map(|px| px[3]).collect())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::Seg;
    use crate::image_io::{ColorSpace, SourceImage};

    #[test]
    fn width_formula() {
        assert!((stroke_width(300.0, 2, 2.0) - 6.4807).abs() < 1e-3);
    }

    #[test]
    fn width_growth_is_clamped_and_rounded_at_dpr_one() {
        // At most a tenth of the length, grown by sqrt((len/dpr + 550)/600) kept within 1–1.5.
        assert_eq!(stroke_width(20.0, 2, 2.0), 4.0);
        assert_eq!(stroke_width(2000.0, 2, 2.0), 9.0);
        // At dpr 1, over 4 px is rounded half away from zero: 7.14 → 7 and 4.5 → 5.
        assert_eq!(stroke_width(300.0, 4, 1.0), 7.0);
        assert_eq!(stroke_width(2000.0, 2, 1.0), 5.0);
        assert!((stroke_width(300.0, 2, 1.0) - 3.570_714_214).abs() < 1e-9);
    }

    #[test]
    fn short_and_long_scaling() {
        assert_eq!(head_tail_k(100.0, 2), (0.5, 0.5)); // thr 200
        assert_eq!(head_tail_k(100.0, 6), (100.0 / 224.0, 100.0 / 224.0));
        let (h, t) = head_tail_k(720.0, 2);
        assert!((t - 0.8).abs() < 1e-9 && (h - 1.25).abs() < 1e-9);
        // Between the two: unscaled.
        assert_eq!(head_tail_k(200.0, 2), (1.0, 1.0));
        assert_eq!(head_tail_k(520.0, 2), (1.0, 1.0));
    }

    #[test]
    fn ease_curves() {
        for (kind, t, want) in [
            (0, 0.3, 0.3),
            (1, 0.0, 1.0),
            (1, 0.5, 0.0),
            (3, 0.0, 1.0),
            (3, 0.5, 0.25),
            (3, 1.0, 0.0),
            (6, 0.0, 1.0),
            (6, 0.2, 0.875),
            (6, 0.6, 0.125),
            (6, 0.8, 0.0),
            (6, 0.85, 0.125),
            (6, 0.95, 0.875),
            (6, 1.0, 1.0),
        ] {
            let got = ease(kind, t);
            assert!(
                (got - want).abs() < 1e-12,
                "ease({kind}, {t}) = {got}, want {want}"
            );
        }
    }

    #[test]
    fn taper_spreads_widths_by_length() {
        // Points 0, 1 and 3 px along: t = 0, 1/3, 1 (kept as the weight).
        let mut pts: Vec<Pt> = [0.0, 1.0, 3.0].map(|x| Pt::new(V2 { x, y: 0.0 })).to_vec();
        taper(&mut pts, 2.0, 6.0, 0);
        let got: Vec<(f64, f64)> = pts.iter().map(|p| (p.weight, p.width)).collect();
        let want = [(0.0, 2.0), (1.0 / 3.0, 2.0 + 4.0 / 3.0), (1.0, 6.0)];
        for (g, w) in got.iter().zip(want) {
            assert!(
                (g.0 - w.0).abs() < 1e-12 && (g.1 - w.1).abs() < 1e-12,
                "{got:?}"
            );
        }
    }

    /// `a` drawn on a white 300×100 image at dpr 2, as PNG.
    fn png(a: &ResolvedArrow) -> Vec<u8> {
        let src = SourceImage {
            width: 300,
            height: 100,
            rgba: vec![255; 300 * 100 * 4],
            dpi: None,
            icc: None,
            color_space: ColorSpace::Srgb,
        };
        let mut c = Canvas::new(src, 2.0);
        draw(&mut c, a);
        c.into_png().unwrap()
    }

    #[test]
    fn sketch_is_seed_deterministic() {
        let mut a = resolved(ArrowStyle::Sketch, [20.0, 50.0], [280.0, 50.0]);
        let first = png(&a);
        assert_eq!(png(&a), first);
        a.seed = 43;
        assert_ne!(png(&a), first);
    }

    #[test]
    fn zero_length_arrow_draws_nothing() {
        for style in [
            ArrowStyle::Shaped,
            ArrowStyle::Line,
            ArrowStyle::Double,
            ArrowStyle::Sketch,
        ] {
            let c = coverage(20, 20, 2.0, &resolved(style, [10.0, 10.0], [10.0, 10.0]));
            assert!(c.iter().flatten().all(|&v| v == 0), "{style:?}");
        }
    }

    #[test]
    fn tiny_arrows_stay_finite() {
        // 2 px long, straight and bent: every style shrinks to a sliver (thinner than tiny-skia
        // samples, for the sketch) but stays a real shape, never NaN.
        let (p0, p1) = (V2 { x: 10.0, y: 30.0 }, V2 { x: 12.0, y: 30.0 });
        for via in [None, Some(V2 { x: 11.0, y: 10.0 })] {
            let g = Geometry::new(p0, p1, 2, via, 2.0);
            let lw = stroke_width(g.len, 2, 2.0);
            let paths = [
                shaped::paths(&g, 0),
                sketch::paths(&g, 42),
                vec![line_arrow::path(&g, lw, true)],
            ]
            .concat();
            for path in &paths {
                let points = path.0.iter().flat_map(|seg| match *seg {
                    Seg::Move(p) | Seg::Line(p) => vec![p],
                    Seg::Cubic(c1, c2, p) => vec![c1, c2, p],
                    Seg::Close => vec![],
                });
                let mut points = points.peekable();
                assert!(points.peek().is_some(), "via {via:?}: empty");
                assert!(
                    points.all(|p| p.x.is_finite() && p.y.is_finite()),
                    "via {via:?}: {path:?}"
                );
            }
        }
    }
}
