//! The counter tool: a numbered badge. Its body, a circle or a bubble or pin whose tail points
//! along the counter's angle, is filled in its color; its number goes on top, white Medium on
//! dark colors and black Regular on light ones. The number is laid out with SF Pro's metrics;
//! the embedded Inter draws its digits, scaled to SF's digit height.

use std::sync::LazyLock;

use tiny_skia::{BlendMode, FillRule};

use crate::canvas::{Canvas, Ink};
use crate::color::Rgb;
use crate::geom::{PathData, V2};
use crate::resolve::ResolvedCounter;
use crate::spec::CounterStyle;
use crate::svg_path::parse_mlc;
use crate::text::{self, Weight};

/// Diameters in points, by size.
pub const DIAM: [f64; 10] = [
    22.0, 28.0, 36.0, 48.0, 64.0, 84.0, 112.0, 150.0, 200.0, 266.0,
];

/// The bubble's and the pin's outlines as SVG paths: a circle of radius 25 about (50, 50) with
/// the tail along +y.
pub const TAIL_SVG: [&str; 2] = [
    "M25,50 C25,36.1928813 36.1928813,25 50,25 C63.8071187,25 75,36.1928813 75,50 C75,69.9607329 55,73.75 52.5,85 C52.0565531,86.9955109 51.0150513,87.5 50,87.5 C48.9849487,87.5 47.9371276,86.9670742 47.5,85 C45,73.75 25,69.998403 25,50",
    "M49.9987059,88.7457631 C49.9987059,88.7457631 75.2211928,68.1295281 74.9970444,49.9983385 C74.9970444,36.1921374 63.804907,25 49.9987059,25 C36.1925048,25 25.0003675,36.1921374 25.0003675,49.9983385 C24.7508105,68.2337185 49.9987059,88.7457631 49.9987059,88.7457631",
];

/// A counter placed by the point its tail marks is centered this many radii back from it.
pub const TAIL_OFFSET: f64 = 1.65;

/// π/2 to eight decimals, 1.57079633: the quarter turn that points the outlines' tail (+y)
/// along the counter's angle.
#[expect(
    clippy::approx_constant,
    reason = "π/2 to eight decimals is deliberate, see the doc comment"
)]
const QUARTER_TURN: f64 = 1.570_796_33;

const WHITE: Rgb = Rgb {
    r: 1.0,
    g: 1.0,
    b: 1.0,
};

const BLACK: Rgb = Rgb {
    r: 0.0,
    g: 0.0,
    b: 0.0,
};

/// SF Pro's ascender and descender in em (`hhea` 1980 and 432 of 2048 units in
/// `/System/Library/Fonts/SFNS.ttf`): the line box of the label.
const SF_ASCENDER: f64 = 1980.0 / 2048.0;
const SF_DESCENDER: f64 = 432.0 / 2048.0;

/// Inter's digits drawn at SF Pro's digit height: SF's flat-topped digits are 1443 of 2048
/// units tall, Inter's 1490.
const DIGIT_SCALE: f64 = 1443.0 / 1490.0;

/// SF Pro's digits 0–9 as (advance, ink center) in 1/2048 em (`/System/Library/Fonts/SFNS.ttf`,
/// macOS 27), Regular (`wght` 400) then Medium (`wght` 510): at optical size 28 and up, where
/// they stop changing, and at 17, the smallest. Core Text sets the optical size to the point
/// size; in between, the font varies linearly along its normalized `opsz` axis (see
/// [`SF_OPSZ_AVAR`]).
const SF_DIGITS_28: [[(f64, f64); 10]; 2] = [
    [
        (1242.0, 621.0),
        (908.0, 379.0),
        (1159.0, 588.5),
        (1212.0, 598.0),
        (1238.0, 629.5),
        (1198.0, 622.5),
        (1264.0, 620.5),
        (1120.0, 553.5),
        (1228.0, 614.0),
        (1264.0, 643.5),
    ],
    [
        (1279.033, 639.517),
        (935.5, 395.867),
        (1186.867, 599.317),
        (1240.417, 612.758),
        (1270.267, 644.167),
        (1228.067, 637.9),
        (1290.217, 635.258),
        (1138.883, 563.125),
        (1265.033, 632.517),
        (1290.217, 654.958),
    ],
];
const SF_DIGITS_17: [[(f64, f64); 10]; 2] = [
    [
        (1290.0, 645.0),
        (950.0, 412.5),
        (1236.0, 623.0),
        (1284.0, 647.0),
        (1318.0, 659.0),
        (1266.0, 644.5),
        (1304.0, 652.0),
        (1166.0, 574.5),
        (1308.0, 654.0),
        (1304.0, 652.0),
    ],
    [
        (1331.067, 665.533),
        (985.933, 431.292),
        (1265.7, 635.65),
        (1315.533, 660.842),
        (1350.633, 673.85),
        (1299.917, 665.308),
        (1339.2, 669.233),
        (1192.4, 588.525),
        (1349.433, 674.717),
        (1339.2, 669.967),
    ],
];

/// SF Pro's `avar` map of the `opsz` axis below its default 28 (normalized: −1 at 17).
const SF_OPSZ_AVAR: [(f64, f64); 4] = [
    (-1.0, -1.0),
    (-0.818_176_269_531_25, -0.909_118_652_343_75),
    (-0.363_647_460_937_5, -0.181_823_730_468_75),
    (0.0, 0.0),
];

/// SF Pro's normal tracking (`trak`): (point size, 1/2048 em added after each glyph). Core Text
/// interpolates it linearly by size and truncates it toward zero to whole units.
const SF_TRACKING: [(f64, f64); 21] = [
    (6.0, 82.0),
    (9.0, 38.0),
    (10.0, 24.0),
    (11.0, 12.0),
    (12.0, 0.0),
    (13.0, -12.0),
    (14.0, -22.0),
    (15.0, -32.0),
    (16.0, -40.0),
    (17.0, -52.0),
    (20.0, -46.0),
    (22.0, -24.0),
    (24.0, 6.0),
    (28.0, 28.0),
    (32.0, 26.0),
    (36.0, 21.0),
    (50.0, 14.0),
    (64.0, 7.0),
    (80.0, 0.0),
    (100.0, 0.0),
    (138.0, 0.0),
];

/// [`TAIL_SVG`], parsed once.
static TAILS: LazyLock<[PathData; 2]> =
    LazyLock::new(|| TAIL_SVG.map(|s| parse_mlc(s).expect("the counter outlines parse")));

/// A counter's radius in px: half its diameter, times dpr.
///
/// # Panics
/// When `size` is 10 or more; resolved sizes are 0 to 7.
#[must_use]
pub fn radius(size: u8, dpr: f64) -> f64 {
    DIAM[usize::from(size)] / 2.0 * dpr
}

/// The center of a counter of radius `r` whose tail marks `at` (y-up px): `1.65·r` back from
/// it along the tail, which points `angle_deg_screen` degrees on screen (0 right, 90 down). A
/// circle has no tail and is centered on `at`.
#[must_use]
pub fn center_from_at(at: V2, angle_deg_screen: f64, r: f64, style: CounterStyle) -> V2 {
    let offset = match style {
        CounterStyle::Circle => 0.0,
        CounterStyle::Bubble | CounterStyle::Pin => TAIL_OFFSET * r,
    };
    at - V2::dir(-angle_deg_screen.to_radians()) * offset
}

/// The label's weight, size in px and ink for a counter filled `fill` showing `count` on a
/// diameter of `d` points: white Medium on fills of luminance up to 0.6 and black Regular on
/// lighter ones, `0.55·d·dpr` px for one digit and 10% less for more.
#[must_use]
pub fn label_style(fill: Rgb, count: u32, d: f64, dpr: f64) -> (Weight, f64, Rgb) {
    let px = d * 0.5 * dpr * 1.1 * if count > 9 { 0.9 } else { 1.0 };
    if fill.luminance() > 0.6 {
        (Weight::Regular, px, BLACK)
    } else {
        (Weight::Medium, px, WHITE)
    }
}

/// Draws `k` on the main surface: its body filled in its color, then its number in the
/// [`label_style`], laid out by `label`.
///
/// # Panics
/// When `k.size` is 10 or more; resolved sizes are 0 to 7.
pub fn draw(c: &mut Canvas, k: &ResolvedCounter) {
    let d = DIAM[usize::from(k.size)];
    let center = c.up(k.center);
    let opaque = |rgb| Ink {
        rgb,
        alpha: 1.0,
        blend: BlendMode::SourceOver,
    };
    let outline = body(center, k.angle, radius(k.size, c.dpr), k.style);
    c.main().fill(&outline, opaque(k.color), FillRule::Winding);
    let (weight, px, ink) = label_style(k.color, k.number, d, c.dpr);
    let glyphs = label(k.number, weight, px, center, d, c.dpr);
    c.main().fill(&glyphs, opaque(ink), FillRule::Winding);
}

/// The body's outline in y-up px about `center`: a circle of radius `r`, or the bubble or pin
/// outline scaled to it and turned so that its tail points `angle` degrees on screen.
fn body(center: V2, angle: f64, r: f64, style: CounterStyle) -> PathData {
    let svg = match style {
        CounterStyle::Circle => {
            let mut circle = PathData::default();
            circle.add_circle(center, r);
            return circle;
        }
        CounterStyle::Bubble => &TAILS[0],
        CounterStyle::Pin => &TAILS[1],
    };
    // Each point turns about the SVG's center, in polar form, so that the tail, along +y,
    // points along the tail's y-up angle, and the radius 25 scales to `r`.
    let turn = -angle.to_radians() - QUARTER_TURN;
    svg.map(|p| {
        let q = p - V2 { x: 50.0, y: 50.0 };
        let dist = r * (q.x * q.x + q.y * q.y).sqrt() / 25.0;
        center + V2::dir(turn + q.angle()) * dist
    })
}

/// `number` at `px` px per em on a counter of `d` points at `center` (y-up px). Its box
/// ([`label_box`]) is centered on whole pixels, then moved right by 0.3k and up by 0.5k
/// (k = dpr·d/28), and its baseline runs the ascender below its top. The layout takes SF Pro's
/// metrics at that size, tracking included; each Inter digit, scaled to SF's digit height, is
/// centered on SF's ink.
fn label(number: u32, weight: Weight, px: f64, center: V2, d: f64, dpr: f64) -> PathData {
    let em = px / 2048.0;
    let digits = number.to_string();
    let (w, h) = label_box(&digits, weight, px);
    let k = dpr * d / 28.0;
    let left = (center.x - w / 2.0).round() + 0.3 * k;
    let bottom = (center.y - h / 2.0).round() + 0.5 * k;
    let baseline = bottom + h - SF_ASCENDER * px;
    let glyph_px = DIGIT_SCALE * px;
    let mut path = PathData::default();
    let mut pen = left;
    for ch in digits.bytes() {
        let (sf_advance, sf_ink) = sf_digit(weight, ch, px);
        let (ink_left, ink_right) = text::ink_span(char::from(ch), weight, glyph_px);
        let origin = V2 {
            x: pen + sf_ink * em - f64::midpoint(ink_left, ink_right),
            y: baseline,
        };
        path.0
            .extend(text::glyph_path(char::from(ch), weight, glyph_px, origin).0);
        pen += (sf_advance + sf_tracking(px)) * em;
    }
    path
}

/// The label box for `digits` at `px` px per em, as `CATextLayer.preferredFrameSize` sizes it:
/// the tracked advance plus 1 px wide and the ascender plus descender tall, each rounded up,
/// in SF Pro's metrics. Kerning between digits is left out.
fn label_box(digits: &str, weight: Weight, px: f64) -> (f64, f64) {
    let advance: f64 = digits
        .bytes()
        .map(|ch| sf_digit(weight, ch, px).0 + sf_tracking(px))
        .sum();
    (
        (advance * px / 2048.0 + 1.0).ceil(),
        ((SF_ASCENDER + SF_DESCENDER) * px).ceil(),
    )
}

/// SF Pro's (advance, ink center) in 1/2048 em for the digit `ch` set at `px` px (points to
/// Core Text), between its optical sizes 17 and 28.
fn sf_digit(weight: Weight, ch: u8, px: f64) -> (f64, f64) {
    let w = usize::from(weight == Weight::Medium);
    let i = usize::from(ch - b'0');
    let ((a28, c28), (a17, c17)) = (SF_DIGITS_28[w][i], SF_DIGITS_17[w][i]);
    let toward_17 = -piecewise(&SF_OPSZ_AVAR, (px.clamp(17.0, 28.0) - 28.0) / 11.0);
    (a28 + (a17 - a28) * toward_17, c28 + (c17 - c28) * toward_17)
}

/// Core Text's tracking of SF Pro at `px` px, in 1/2048 em: whole units, truncated toward zero.
fn sf_tracking(px: f64) -> f64 {
    piecewise(&SF_TRACKING, px).trunc()
}

/// `table`'s piecewise-linear value at `x`, held flat beyond its ends.
fn piecewise(table: &[(f64, f64)], x: f64) -> f64 {
    let (first, last) = (table[0], table[table.len() - 1]);
    if x <= first.0 {
        return first.1;
    }
    table
        .windows(2)
        .find(|pair| x <= pair[1].0)
        .map_or(last.1, |pair| {
            let ((x0, y0), (x1, y1)) = (pair[0], pair[1]);
            y0 + (y1 - y0) * (x - x0) / (x1 - x0)
        })
}

#[cfg(test)]
mod tests {
    use std::f64::consts::FRAC_1_SQRT_2;

    use super::*;
    use crate::color::parse_hex;
    use crate::geom::Seg;
    use crate::image_io::{ColorSpace, SourceImage, decode_png};

    /// How far the pin's tip is from the center of its circle of radius 25, in SVG units.
    const PIN_REACH: f64 = 38.745_763_1;

    fn assert_near(got: V2, want: (f64, f64), tolerance: f64) {
        assert!(
            (got.x - want.0).abs() < tolerance && (got.y - want.1).abs() < tolerance,
            "{got:?} is not within {tolerance} of {want:?}"
        );
    }

    /// The on-curve points of `p`: where its segments end.
    fn ends(p: &PathData) -> Vec<V2> {
        p.0.iter()
            .filter_map(|s| match *s {
                Seg::Move(a) | Seg::Line(a) | Seg::Cubic(_, _, a) => Some(a),
                Seg::Close => None,
            })
            .collect()
    }

    /// The on-curve point of `p` furthest from `from`.
    fn furthest(p: &PathData, from: V2) -> V2 {
        ends(p)
            .into_iter()
            .max_by(|a, b| (*a - from).len().total_cmp(&(*b - from).len()))
            .expect("the path has points")
    }

    #[test]
    fn center_offset() {
        // 1.65·R = 59.4 px back from `at` along the tail. At 135° the tail points down-left on
        // screen, (−√½, −√½) with y up, so the center is 59.4·√½ = 42.0021428 up and right.
        let at = V2 { x: 500.0, y: 500.0 };
        for style in [CounterStyle::Bubble, CounterStyle::Pin] {
            assert_near(
                center_from_at(at, 135.0, 36.0, style),
                (542.002_142_8, 542.002_142_8),
                1e-6,
            );
        }
        // At 90° the tail points down on screen: the center is straight above `at`.
        assert_near(
            center_from_at(at, 90.0, 36.0, CounterStyle::Bubble),
            (500.0, 559.4),
            1e-9,
        );
        // A circle has no tail and is centered on `at`.
        assert_eq!(center_from_at(at, 135.0, 36.0, CounterStyle::Circle), at);
    }

    #[test]
    fn label_rules() {
        // #FF0C01 has luminance 0.327: white Medium. #FFDC2C has 0.825: black Regular. One digit
        // is 0.55·d·dpr px, two or more 10% smaller.
        let white = Rgb {
            r: 1.0,
            g: 1.0,
            b: 1.0,
        };
        let black = Rgb {
            r: 0.0,
            g: 0.0,
            b: 0.0,
        };
        let (w, px, ink) = label_style(parse_hex("#FF0C01").unwrap(), 3, 36.0, 2.0);
        assert_eq!(w, Weight::Medium);
        assert!((px - 39.6).abs() < 1e-9, "{px}");
        assert_eq!(ink, white);
        let (w2, px2, ink2) = label_style(parse_hex("#FFDC2C").unwrap(), 12, 36.0, 2.0);
        assert_eq!(w2, Weight::Regular);
        assert!((px2 - 35.64).abs() < 1e-9, "{px2}");
        assert_eq!(ink2, black);
        let px_for = |count| label_style(parse_hex("#FF0C01").unwrap(), count, 22.0, 1.0).1;
        assert!((px_for(9) - 12.1).abs() < 1e-9, "{}", px_for(9));
        assert!((px_for(10) - 10.89).abs() < 1e-9, "{}", px_for(10));
    }

    #[test]
    fn tail_svg_parse() {
        // Each outline is a circle of radius 25 about (50, 50) whose tail reaches furthest
        // along +y: to 1.5 radii for the bubble, 1.55 for the pin.
        let center = V2 { x: 50.0, y: 50.0 };
        for (svg, reach) in TAIL_SVG.iter().zip([37.5, PIN_REACH]) {
            let outline = parse_mlc(svg).unwrap();
            assert_near(furthest(&outline, center), (50.0, 50.0 + reach), 0.002);
        }
    }

    #[test]
    fn tail_points_along_the_angle() {
        // R 36: the bubble's tip is 1.5·R = 54 px from the center, the pin's 1.55·R, toward the
        // screen angle: 135° is down-left, (−√½, −√½) with y up, 0° right and 90° down.
        let center = V2 { x: 100.0, y: 100.0 };
        let directions = [
            (135.0, (-FRAC_1_SQRT_2, -FRAC_1_SQRT_2)),
            (0.0, (1.0, 0.0)),
            (90.0, (0.0, -1.0)),
        ];
        for (style, reach) in [
            (CounterStyle::Bubble, 1.5),
            (CounterStyle::Pin, PIN_REACH / 25.0),
        ] {
            for (angle, (dx, dy)) in directions {
                let tip = furthest(&body(center, angle, 36.0, style), center);
                let len = 36.0 * reach;
                assert_near(tip, (100.0 + len * dx, 100.0 + len * dy), 0.01);
            }
        }
        // A circle has no tail.
        let circle = body(center, 135.0, 36.0, CounterStyle::Circle);
        for p in ends(&circle) {
            assert!(((p - center).len() - 36.0).abs() < 1e-9, "{p:?}");
        }
    }

    #[test]
    fn label_is_laid_out_with_sf_metrics() {
        // "11" in Medium at 39.6 px on d 36 at dpr 2 (k = 72/28). SF's "1" advances 935.5/2048 em
        // plus 19 of tracking (18.456 px), so the box is ceil(2 × 18.456 + 1) = 38 px wide and
        // ceil((1980 + 432)/2048 × 39.6) = 47 px tall. Centered on (398, 702) and rounded half
        // away from zero, then nudged, its left is 379 + 0.3k and its bottom 679 + 0.5k, so the
        // baseline, 38.285 px below its top, is at 689.0006. Each "1" is centered on SF's ink,
        // 395.867/2048 em (7.654 px) after its pen, and is SF's 1443/2048 em (27.902 px) tall.
        let center = V2 { x: 398.0, y: 702.0 };
        let pts = ends(&label(11, Weight::Medium, 39.6, center, 36.0, 2.0));
        let left = 379.0 + 0.3 * 72.0 / 28.0;
        for (pen, keep) in [(left, 0.0..397.0), (left + 18.456_152_3, 397.0..500.0)] {
            let glyph: Vec<V2> = pts
                .iter()
                .copied()
                .filter(|p| keep.contains(&p.x))
                .collect();
            let min = |f: fn(&V2) -> f64| glyph.iter().map(f).fold(f64::INFINITY, f64::min);
            let max = |f: fn(&V2) -> f64| glyph.iter().map(f).fold(f64::NEG_INFINITY, f64::max);
            let (x0, x1, y0, y1) = (min(|p| p.x), max(|p| p.x), min(|p| p.y), max(|p| p.y));
            let ink = V2 {
                x: f64::midpoint(x0, x1),
                y: y0,
            };
            assert_near(ink, (pen + 7.654_459_6, 689.000_558), 1e-6);
            assert!((y1 - y0 - 27.901_757_8).abs() < 1e-6, "{} px tall", y1 - y0);
        }
    }

    #[test]
    fn label_box_is_core_texts() {
        // `CATextLayer.preferredFrameSize` widths of SF Pro labels on macOS 27, which the box
        // must reproduce: digits whose box tracking widens (39.6 px Medium), digits at small
        // optical sizes (19.8 and 12.1 px), two digits, and a digit whose tracking Core Text
        // truncates (46.2 px: 15.9 units, so 15; rounding to 16 gives a 31 px box). The height
        // is ceil(2412/2048 × px).
        for (digits, weight, px, width) in [
            ("0", Weight::Medium, 39.6, 27.0),
            ("1", Weight::Medium, 39.6, 20.0),
            ("2", Weight::Medium, 39.6, 25.0),
            ("3", Weight::Medium, 39.6, 26.0),
            ("7", Weight::Medium, 39.6, 24.0),
            ("1", Weight::Medium, 19.8, 10.0),
            ("3", Weight::Medium, 19.8, 14.0),
            ("7", Weight::Medium, 19.8, 12.0),
            ("1", Weight::Regular, 12.1, 7.0),
            ("7", Weight::Regular, 12.1, 8.0),
            ("12", Weight::Medium, 35.64, 39.0),
            ("12", Weight::Regular, 19.8, 22.0),
            ("4", Weight::Medium, 46.2, 30.0),
        ] {
            let (w, h) = label_box(digits, weight, px);
            assert_eq!(
                (w, h),
                (width, (2412.0 / 2048.0 * px).ceil()),
                "{digits} at {px}"
            );
        }
    }

    #[test]
    fn number_is_drawn_over_the_body_in_its_ink() {
        // A size-2 circle (R 36 at dpr 2) centered at (45, 50) on a white 100 × 100 canvas: its
        // "8" is drawn in the ink around the center, inside the body, which shows its color
        // beside it.
        for (color, body_rgba, ink_rgba) in [
            ("#FF0C01", [255, 12, 1, 255], [255; 4]),
            ("#FFDC2C", [255, 220, 44, 255], [0, 0, 0, 255]),
        ] {
            let k = ResolvedCounter {
                id: "counter-1".to_owned(),
                color: parse_hex(color).unwrap(),
                size: 2,
                center: [45.0, 50.0],
                angle: 135.0,
                style: CounterStyle::Circle,
                number: 8,
            };
            let src = SourceImage {
                width: 100,
                height: 100,
                rgba: vec![255; 100 * 100 * 4],
                dpi: None,
                icc: None,
                color_space: ColorSpace::Srgb,
            };
            let mut c = Canvas::new(src, 2.0);
            draw(&mut c, &k);
            let rgba = decode_png(&c.into_png().unwrap()).unwrap().rgba;
            let px = rgba.as_chunks::<4>().0;
            assert_eq!(
                px[50 * 100 + 17],
                body_rgba,
                "{color} 28 px left of the center"
            );
            let mut ink = Vec::new();
            for y in 0..100_u32 {
                for x in 0..100_u32 {
                    let (cx, cy) = (f64::from(x) + 0.5, f64::from(y) + 0.5);
                    let inside = (cx - 45.0).powi(2) + (cy - 50.0).powi(2) < 34.0_f64.powi(2);
                    if inside && px[(y * 100 + x) as usize] == ink_rgba {
                        ink.push((cx, cy));
                    }
                }
            }
            assert!(ink.len() > 100, "{color}: {} ink pixels", ink.len());
            let n = f64::from(u32::try_from(ink.len()).unwrap());
            let centroid = (
                ink.iter().map(|p| p.0).sum::<f64>() / n,
                ink.iter().map(|p| p.1).sum::<f64>() / n,
            );
            assert!(
                (centroid.0 - 45.0).abs() < 2.5 && (centroid.1 - 50.0).abs() < 2.5,
                "{color}: the number is centered at {centroid:?}"
            );
        }
    }
}
