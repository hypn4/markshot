//! the reference app's Line tool (Line.md §3): a segment stroked with round caps, with an optional
//! arrowhead or dot at each end, or dashed and then without them.

use tiny_skia::{BlendMode, LineCap, LineJoin};

use crate::canvas::{Canvas, Ink, StrokeStyle};
use crate::geom::{PathData, Seg, V2};
use crate::math::PI_TRUNCATED;
use crate::resolve::ResolvedLine;
use crate::spec::Cap;

/// Stroke widths in points, by size.
pub const WIDTHS_PT: [f64; 10] = [1.0, 2.0, 3.0, 4.0, 6.0, 8.0, 11.0, 15.0, 21.0, 30.0];

/// From this length (px) on, a line gets round joins and 50° arrowheads; shorter, miter joins
/// and 45°.
const LONG_LINE: f64 = 20.0;

/// Draws `l` on the main surface: the segment, then the end shapes, stroked together as one
/// path. (the reference app evens the x extent when zoomed out; zoom is fixed at 1 here.)
///
/// # Panics
/// When `l.size` is 10 or more; resolved sizes are 0 to 7.
pub fn draw(c: &mut Canvas, l: &ResolvedLine) {
    let dpr = c.dpr;
    let wpt = WIDTHS_PT[usize::from(l.size)];
    let from = c.up(l.from);
    let span = c.up(l.to) - from;
    let len = span.len();
    let lw = stroke_width(len, wpt, dpr);
    let off = pixel_offset(lw, dpr);
    let start = from + V2 { x: off, y: off };
    let end = start + span;
    let mut path = PathData(vec![Seg::Move(start), Seg::Line(end)]);
    if !l.dash {
        // `theta` points away from the line at its end.
        let mut cap = |tip: V2, theta: f64, kind: Cap| match kind {
            Cap::None => {}
            Cap::Arrow => {
                // An open chevron: wings at θ + π ∓ α.
                let alpha = half_angle(len);
                let head = head_len(len, lw, dpr);
                let t1 = PI_TRUNCATED - alpha + theta;
                let t2 = t1 + 2.0 * alpha;
                path.0.extend([
                    Seg::Move(tip + V2::dir(t1) * head),
                    Seg::Line(tip),
                    Seg::Line(tip + V2::dir(t2) * head),
                ]);
            }
            Cap::Bullet => path.add_circle(tip, bullet_radius(len, lw)),
        };
        cap(start, (-span).angle(), l.start_cap);
        cap(end, span.angle(), l.end_cap);
    }
    let style = StrokeStyle {
        width: lw,
        cap: LineCap::Round,
        join: if len >= LONG_LINE {
            LineJoin::Round
        } else {
            LineJoin::Miter
        },
        dash: l.dash.then(|| dash(wpt, dpr).to_vec()),
    };
    let ink = Ink {
        rgb: l.color,
        alpha: 1.0,
        blend: BlendMode::SourceOver,
    };
    c.main().stroke(&path, ink, &style);
}

/// The stroke width in px: the size's width in points, but at most a tenth of the length.
#[must_use]
pub fn stroke_width(len: f64, wpt: f64, dpr: f64) -> f64 {
    wpt.min(len / 10.0) * dpr
}

/// The length of each arrowhead wing in px, at most a quarter of the line.
#[must_use]
pub fn head_len(len: f64, lw: f64, dpr: f64) -> f64 {
    (0.25 * len).min(0.7 * lw + 0.2 * dpr + 7.0 * lw.sqrt())
}

/// The angle between the line and each arrowhead wing, in radians: 50° from 20 px on, else
/// 45°, both from the reference app's truncated π.
#[must_use]
pub fn half_angle(len: f64) -> f64 {
    if len >= LONG_LINE {
        PI_TRUNCATED * 5.0 / 18.0
    } else {
        PI_TRUNCATED / 4.0
    }
}

/// The dash's on and off lengths in px, from the size's width in points (not the shortened
/// stroke width).
#[must_use]
pub fn dash(wpt: f64, dpr: f64) -> [f64; 2] {
    [1.5 * dpr, (1.5 * wpt + 2.0) * dpr]
}

/// The radius in px of the bullet's circle, which is stroked like the line.
#[must_use]
pub fn bullet_radius(len: f64, lw: f64) -> f64 {
    (len / 10.0).min(0.5 * lw + 1.0)
}

/// the reference app's pixel alignment at dpr 1: half a pixel when the width rounds to an odd number,
/// plus half the width's fraction in hundredths. None at other dprs.
#[expect(
    clippy::float_cmp,
    reason = "the reference app tests dpr == 1 exactly, and the rounded width is a whole number"
)]
fn pixel_offset(lw: f64, dpr: f64) -> f64 {
    if dpr != 1.0 {
        return 0.0;
    }
    let odd = if lw.round() % 2.0 == 1.0 { 0.5 } else { 0.0 };
    odd + (lw * 100.0).round() % 100.0 / 100.0 * 0.5
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::parse_hex;
    use crate::image_io::{ColorSpace, SourceImage, decode_png};
    use crate::spec::Point;

    /// A size-2 `#FF0000` line without caps.
    fn line(from: Point, to: Point) -> ResolvedLine {
        ResolvedLine {
            id: "line-1".to_owned(),
            color: parse_hex("#FF0000").unwrap(),
            size: 2,
            from,
            to,
            dash: false,
            start_cap: Cap::None,
            end_cap: Cap::None,
        }
    }

    /// The alpha (coverage) of every pixel after drawing `l` on a transparent canvas, by row.
    fn coverage(width: u32, height: u32, dpr: f64, l: &ResolvedLine) -> Vec<Vec<u8>> {
        let src = SourceImage {
            width,
            height,
            rgba: vec![0; (width * height * 4) as usize],
            dpi: None,
            icc: None,
            color_space: ColorSpace::Srgb,
        };
        let mut c = Canvas::new(src, dpr);
        draw(&mut c, l);
        let png = decode_png(&c.into_png().unwrap()).unwrap();
        png.rgba
            .as_chunks::<4>()
            .0
            .chunks(width as usize)
            .map(|row| row.iter().map(|px| px[3]).collect())
            .collect()
    }

    #[test]
    fn line_widths() {
        assert_eq!(WIDTHS_PT, [1., 2., 3., 4., 6., 8., 11., 15., 21., 30.]);
    }

    #[test]
    fn line_stroke_width() {
        assert_eq!(stroke_width(100.0, 3.0, 2.0), 6.0);
        assert_eq!(stroke_width(20.0, 3.0, 2.0), 4.0);
    }

    #[test]
    fn line_head() {
        assert!((head_len(200.0, 6.0, 2.0) - 21.7464).abs() < 1e-3);
        assert_eq!(head_len(40.0, 6.0, 2.0), 10.0);
    }

    #[test]
    fn line_half_angle() {
        // the reference app's truncated π: 3.1415926 / 4 and 3.1415926 · 5 / 18, not exactly 45° and 50°.
        assert_eq!(half_angle(19.9), 0.785_398_15);
        assert!((half_angle(20.0) - 0.872_664_611_1).abs() < 1e-10);
        assert!((half_angle(500.0) - 0.872_664_611_1).abs() < 1e-10);
    }

    #[test]
    fn line_dash() {
        assert_eq!(dash(3.0, 2.0), [3.0, 13.0]);
        assert_eq!(dash(1.0, 1.0), [1.5, 3.5]);
    }

    #[test]
    fn line_bullet() {
        assert_eq!(bullet_radius(200.0, 6.0), 4.0);
        assert_eq!(bullet_radius(30.0, 6.0), 3.0);
    }

    #[test]
    fn pixel_offset_applies_only_at_dpr_one() {
        for (lw, dpr, want) in [
            (3.0, 1.0, 0.5),
            (2.0, 1.0, 0.0),
            // round(1.5) = 2 is even; half the fraction .5 adds 0.25.
            (1.5, 1.0, 0.25),
            (2.5, 1.0, 0.75),
            (2.34, 1.0, 0.17),
            (3.0, 2.0, 0.0),
        ] {
            assert_eq!(pixel_offset(lw, dpr), want, "lw {lw}, dpr {dpr}");
        }
    }

    #[test]
    fn draws_a_round_capped_stroke_of_the_size_width() {
        // Size 2 is 3 pt: 6 px at dpr 2, so rows 7–12 around y = 10.
        let a = coverage(50, 20, 2.0, &line([10.0, 10.0], [40.0, 10.0]));
        let column = |x: usize| a.iter().map(|row| row[x]).collect::<Vec<_>>();
        let mut want = vec![0; 20];
        want[7..13].fill(255);
        assert_eq!(column(25), want);
        // The round cap reaches 3 px past the end: full near the axis, partial in the corner.
        assert_eq!(a[9][8], 255);
        assert!(a[7][7] > 0 && a[7][7] < 255, "{}", a[7][7]);
        assert_eq!(column(6), [0; 20]);
        assert_eq!(column(43), [0; 20]);
    }

    #[test]
    fn dpr_one_moves_odd_widths_onto_whole_pixels() {
        // 3 px wide centred on y = 10 would half-cover rows 8 and 11; the half-pixel offset
        // (up in y-up, so to 9.5 here) makes it exactly rows 8–10.
        let a = coverage(60, 20, 1.0, &line([10.0, 10.0], [50.0, 10.0]));
        let column: Vec<u8> = a.iter().map(|row| row[30]).collect();
        let mut want = vec![0; 20];
        want[8..11].fill(255);
        assert_eq!(column, want);
    }

    #[test]
    fn arrow_caps_open_back_along_the_line() {
        // Length 80: head 20 px (a quarter of the length), half-angle 50°. Each wing's midpoint
        // is 10 px from its tip at 50° off the line, (6.43, 7.66) px away.
        let l = ResolvedLine {
            start_cap: Cap::Arrow,
            end_cap: Cap::Arrow,
            ..line([20.0, 50.0], [100.0, 50.0])
        };
        let a = coverage(120, 100, 2.0, &l);
        for (x, y) in [(93, 42), (93, 57), (26, 42), (26, 57)] {
            assert_eq!(a[y][x], 255, "wing at ({x}, {y})");
        }
        for (x, y) in [(106, 42), (106, 57), (13, 42), (13, 57)] {
            assert_eq!(a[y][x], 0, "outside the tips at ({x}, {y})");
        }
    }

    #[test]
    fn bullet_cap_is_a_solid_dot() {
        // Radius 4 stroked 6 px wide: a ring from 1 to 7 px around the tip, its hole covered by
        // the line's round end.
        let l = ResolvedLine {
            end_cap: Cap::Bullet,
            ..line([20.0, 50.0], [100.0, 50.0])
        };
        let a = coverage(120, 100, 2.0, &l);
        for (x, y) in [(99, 45), (99, 54), (105, 49), (100, 49)] {
            assert_eq!(a[y][x], 255, "dot at ({x}, {y})");
        }
        for (x, y) in [(99, 41), (99, 58), (108, 49)] {
            assert_eq!(a[y][x], 0, "outside the dot at ({x}, {y})");
        }
    }

    #[test]
    fn dashed_lines_have_no_caps() {
        // Dash [3, 13] px with round caps: pills over x 7–16, 23–32, … along y = 30.
        let dashed = ResolvedLine {
            dash: true,
            ..line([10.0, 30.0], [90.0, 30.0])
        };
        let plain = coverage(100, 60, 2.0, &dashed);
        assert_eq!(plain[30][11], 255);
        assert_eq!(plain[30][19], 0);
        assert_eq!(plain[30][27], 255);
        let capped = ResolvedLine {
            start_cap: Cap::Arrow,
            end_cap: Cap::Bullet,
            ..dashed
        };
        assert_eq!(coverage(100, 60, 2.0, &capped), plain);
    }

    #[test]
    fn zero_length_line_draws_nothing() {
        let a = coverage(20, 20, 2.0, &line([10.0, 10.0], [10.0, 10.0]));
        assert!(a.iter().flatten().all(|&v| v == 0));
    }
}
