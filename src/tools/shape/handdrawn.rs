//! The hand-drawn rect and oval: a stroke of varying width around the shape whose outline is
//! filled in the shape's color.

use crate::geom::{PathData, Seg, V2};
use crate::math::{PI_TRUNCATED, cos, sin};
use crate::stroke::{
    Pt, assign_widths, bezier_path, blur_weight, flatten, handles, norm, norm_weight, outline,
    simplify,
};

use super::JITTER;
use super::rounded_rect::elements;

/// The hand-drawn rect's corner size in points for a stroke `wpt` points wide:
/// `max(1.1k − 0.004k², 10)` with `k = 6 * wpt` (12 for no stroke).
#[must_use]
pub fn hd_corner(wpt: f64) -> f64 {
    let k = if wpt == 0.0 { 12.0 } else { wpt * 6.0 };
    (1.1 * k - 0.004 * k * k).max(10.0)
}

/// The hand-drawn rect on `rect` (y-up px, corner first) as a closed outline, for a
/// `wpt`-point stroke drawn `stroke` px wide: its width varies by up to ±30% (less on thick
/// strokes), widest at the corners.
pub(super) fn rect(
    rect: (f64, f64, f64, f64),
    wpt: f64,
    stroke: f64,
    dpr: f64,
    rnd: &[f64; 100],
) -> PathData {
    let thinning = (0.3 - stroke / 150.0).clamp(0.08, 0.3);
    let line = flatten(&wobbly_rect(rect, wpt, dpr, rnd), 2.0 * dpr);
    outline_path(line, stroke, thinning, true, rnd)
}

/// The hand-drawn oval on `rect` (y-up px, corner first) as a closed outline, for a stroke
/// `stroke` px wide, 0.75–1.25 times that.
pub(super) fn oval(
    rect: (f64, f64, f64, f64),
    stroke: f64,
    dpr: f64,
    rnd: &[f64; 100],
) -> PathData {
    let mut line = simplify(&spiral(rect, stroke, rnd), 0.5 * dpr);
    handles(&mut line);
    outline_path(flatten(&line, 2.0 * dpr), stroke, 0.25, false, rnd)
}

/// The hand-drawn rect's centre line: the jittered rounded rect with corners of
/// [`hd_corner`], its inner points moved up to half a point each way by draws 27 and 28; the
/// ends stay, so the start shows a small kink.
fn wobbly_rect(rect: (f64, f64, f64, f64), wpt: f64, dpr: f64, rnd: &[f64; 100]) -> [Pt; 9] {
    // The rounded rect underneath gets the hand-drawn jitter too, as the plain rect of a small
    // sketch does.
    let mut line = elements(rect, JITTER, hd_corner(wpt), dpr, rnd);
    let off = V2 {
        x: (rnd[27] - 0.5) * dpr,
        y: (rnd[28] - 0.5) * dpr,
    };
    for p in &mut line[1..8] {
        p.pos = p.pos + off;
        p.a = p.a.map(|a| a + off);
        p.b = p.b.map(|b| b + off);
    }
    line
}

/// The hand-drawn oval's centre line: 1000 points on the curve `m/4` outside the ellipse
/// inscribed `m/4` inside the rect (`m` its shorter side), which touches the rect's sides,
/// from the angle `6.28 * rnd[37]` on. Under a stroke a fifth of `m` the curve turns 0.98–1.04
/// times (draw 8) while its distance drifts from 1.3 strokes inside to 1.3 outside: a spiral
/// that ends past or short of its start. Thicker strokes get the closed ring.
fn spiral((left, bottom, w, h): (f64, f64, f64, f64), stroke: f64, rnd: &[f64; 100]) -> Vec<Pt> {
    let short = w.min(h);
    let center = V2 {
        x: (left + w + left) * 0.5,
        y: (bottom + h + bottom) * 0.5,
    };
    let inner = w - short * 0.5;
    // The inner ellipse's semi-axes: `semi` and `semi * squash`.
    let (semi, squash) = (inner * 0.5, (h - short * 0.5) / inner);
    let (drift, turn) = if stroke * 5.0 < short {
        let draw = rnd[8];
        (
            stroke * 2.6,
            (draw * 0.06 + 0.98 + draw * 0.06 + 0.98) * PI_TRUNCATED,
        )
    } else {
        (0.0, 2.0 * PI_TRUNCATED)
    };
    #[expect(clippy::approx_constant, reason = "6.28, not 2π, is deliberate")]
    let start = rnd[37] * 6.28;
    let gap = short * 0.5 * 0.5;
    (0..1000_u16)
        .map(|i| {
            let t = f64::from(i) / 999.0;
            let theta = start + turn * t;
            let (sin_t, cos_t) = (sin(theta), cos(theta));
            let dist = gap + drift * (t - 0.5);
            // `(cos_t, sin_t/squash)/len` is the ellipse's outward unit normal there.
            let len = (cos_t * cos_t + sin_t * sin_t / (squash * squash)).sqrt();
            Pt::new(V2 {
                x: center.x + semi * cos_t + dist * (cos_t / len),
                y: center.y + semi * squash * sin_t + dist * (sin_t / (squash * len)),
            })
        })
        .collect()
}

/// The varying width and the path around it: the points' widths vary by up to `±thinning`
/// around `stroke` with their weights ([`weigh`]: corners when `corner_boost`, a slow wave and
/// the draws), blurred four times and normalized; the closed outline of that stroke with round
/// ends. Fewer than two points give an empty path.
fn outline_path(
    mut pts: Vec<Pt>,
    stroke: f64,
    thinning: f64,
    corner_boost: bool,
    rnd: &[f64; 100],
) -> PathData {
    if pts.len() < 2 {
        return PathData::default();
    }
    weigh(&mut pts, corner_boost, rnd);
    for _ in 0..4 {
        blur_weight(&mut pts, 41);
    }
    norm_weight(&mut pts, 0.0, 1.0, 1.0);
    let pts = assign_widths(&pts, stroke, thinning);
    let mut path = bezier_path(&outline(&pts, 0, stroke), V2::default());
    path.0.push(Seg::Close);
    path
}

/// The raw weights: `corner + cos(9.42 * i/n) + 1 + 12 * rnd[(84 + i) % 100]`, the corner term
/// being `4000 * (1 − |cos|)` of the turn at each point when `corner_boost`.
fn weigh(pts: &mut [Pt], corner_boost: bool, rnd: &[f64; 100]) {
    #[expect(
        clippy::cast_precision_loss,
        reason = "point counts are far below 2^52"
    )]
    let n = pts.len() as f64;
    // Positions stay put, so each turn reads the neighbours as given.
    for i in 0..pts.len() {
        let corner = if corner_boost { turn(pts, i) } else { 0.0 };
        #[expect(
            clippy::cast_precision_loss,
            reason = "point counts are far below 2^52"
        )]
        let wave = cos(i as f64 * 9.42 / n);
        pts[i].weight = corner + wave + 1.0 + rnd[(84 + i) % 100] * 12.0;
    }
}

/// `4000 * (1 − |cos θ|)`, θ the angle at point `i` between the ways to its neighbours: 0 on a
/// straight run, 4000 at a right angle. An end, missing a neighbour, and a point next to a
/// copy of itself count as straight.
fn turn(pts: &[Pt], i: usize) -> f64 {
    let p = pts[i].pos;
    let prev = if i == 0 { p } else { pts[i - 1].pos };
    let next = pts.get(i + 1).map_or(p, |q| q.pos);
    let (back, ahead) = (prev - p, next - p);
    let (back_len, ahead_len) = (norm(back), norm(ahead));
    let cos = if back_len * ahead_len == 0.0 {
        -1.0
    } else {
        (back.x * ahead.x + back.y * ahead.y) / back_len / ahead_len
    };
    (1.0 - cos.abs()) * 4000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    const NO_DRAWS: [f64; 100] = [0.5; 100];
    const ZERO_DRAWS: [f64; 100] = [0.0; 100];

    fn at(xy: &[(f64, f64)]) -> Vec<Pt> {
        xy.iter().map(|&(x, y)| Pt::new(V2 { x, y })).collect()
    }

    fn assert_near(got: V2, want: (f64, f64), tolerance: f64) {
        assert!(
            (got.x - want.0).abs() < tolerance && (got.y - want.1).abs() < tolerance,
            "{got:?} != {want:?}"
        );
    }

    #[test]
    fn hd_corner_value() {
        assert!((hd_corner(3.0) - 18.504).abs() < 1e-9);
        // 6 pt: 1.1 * 36 − 0.004 * 36² = 34.416; 1 pt: 6.456, raised to 10; none: k = 12.
        assert!((hd_corner(6.0) - 34.416).abs() < 1e-9);
        assert_eq!(hd_corner(1.0), 10.0);
        assert!((hd_corner(0.0) - 12.624).abs() < 1e-9);
    }

    #[test]
    fn hand_drawn_rect_is_jittered_with_its_own_corners() {
        // 300 × 200 at 3 pt, dpr 2: corners of min(200/3, 18.504 * 2) = 37.008 px, and the
        // bottom-left corner's x draw at 1 moves the start by 200 * 1.3 * 0.5/400 = 0.325 px (the
        // hand-drawn rect is built with jitter 1.3, as the plain one on small shapes).
        let mut rnd = NO_DRAWS;
        rnd[56] = 1.0;
        let e = wobbly_rect((0.0, 0.0, 300.0, 200.0), 3.0, 2.0, &rnd);
        assert_near(e[0].pos, (37.333, 0.0), 1e-9);
    }

    #[test]
    fn hand_drawn_rect_shifts_all_but_its_ends() {
        // Draws 27 and 28 at 1 and 0: the inner points move (+1, −1) * dpr/2.
        let mut rnd = NO_DRAWS;
        (rnd[27], rnd[28]) = (1.0, 0.0);
        let rect = (10.0, 20.0, 300.0, 200.0);
        let base = elements(rect, 1.3, hd_corner(3.0), 2.0, &rnd);
        let shifted = wobbly_rect(rect, 3.0, 2.0, &rnd);
        let off = V2 { x: 1.0, y: -1.0 };
        assert_eq!((shifted[0], shifted[8]), (base[0], base[8]));
        for (s, b) in shifted[1..8].iter().zip(&base[1..8]) {
            assert_eq!(s.pos, b.pos + off);
            assert_eq!(s.a, b.a.map(|a| a + off));
            assert_eq!(s.b, b.b.map(|b| b + off));
        }
    }

    #[test]
    fn a_thick_stroke_makes_a_ring_spanning_the_rect() {
        // 200 × 100 with a 20 px stroke (5 * 20 ≥ 100): no drift, a full turn from the right
        // mid-edge (draw 37 at 0), reaching each side to within sampling error.
        let pts = spiral((10.0, 20.0, 200.0, 100.0), 20.0, &ZERO_DRAWS);
        assert_eq!(pts.len(), 1000);
        assert_near(pts[0].pos, (210.0, 70.0), 1e-9);
        let (xs, ys): (Vec<f64>, Vec<f64>) = pts.iter().map(|p| (p.pos.x, p.pos.y)).unzip();
        let extent = |v: &[f64]| {
            v.iter()
                .fold((f64::MAX, f64::MIN), |(lo, hi), &x| (lo.min(x), hi.max(x)))
        };
        let ((x0, x1), (y0, y1)) = (extent(&xs), extent(&ys));
        for (got, want) in [(x0, 10.0), (x1, 210.0), (y0, 20.0), (y1, 120.0)] {
            assert!((got - want).abs() < 2e-3, "{got} != {want}");
        }
        // Draw 37 at 0.25 starts it at 6.28/4 rad: near the top mid-edge.
        let mut rnd = ZERO_DRAWS;
        rnd[37] = 0.25;
        let top = spiral((10.0, 20.0, 200.0, 100.0), 20.0, &rnd)[0].pos;
        assert_near(top, (110.066_360_561_098_07, 119.999_991_192_550_58), 1e-9);
    }

    #[test]
    fn a_thin_stroke_spirals_out_past_its_start() {
        // A 6 px stroke: the distance drifts by 2.6 * 6 = 15.6 px over 0.98 turns (draw 8 at 0),
        // starting 7.8 px inside the right mid-edge and ending 7.8 px outside.
        let pts = spiral((10.0, 20.0, 200.0, 100.0), 6.0, &ZERO_DRAWS);
        assert_near(pts[0].pos, (202.2, 70.0), 1e-9);
        assert_near(
            pts[999].pos,
            (215.079_793_065_421_82, 55.242_639_078_898_364),
            1e-9,
        );
    }

    #[test]
    fn weights_peak_at_turns() {
        // 4000 * (1 − |cos 90°|) at the corner, plus cos(9.42 * i/3) + 1 and 12 * 0 from the draws.
        let mut pts = at(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)]);
        weigh(&mut pts, true, &ZERO_DRAWS);
        let got: Vec<f64> = pts.iter().map(|p| p.weight).collect();
        let want = [2.0, 4_000.000_001_268_272_6, 1.999_994_926_913_375_2];
        assert!(
            got.iter().zip(want).all(|(g, w)| (g - w).abs() < 1e-9),
            "{got:?}"
        );
        // Without the boost, and with draw 85 at 0.5: cos(3.14) + 1 + 6.
        let mut rnd = ZERO_DRAWS;
        rnd[85] = 0.5;
        weigh(&mut pts, false, &rnd);
        assert!(
            (pts[1].weight - 6.000_001_268_272_46).abs() < 1e-9,
            "{pts:?}"
        );
    }

    #[test]
    fn a_repeated_point_is_no_turn() {
        // Each of the joint's two copies has a zero-length side: no corner weight.
        let pts = at(&[(0.0, 0.0), (10.0, 0.0), (10.0, 0.0), (10.0, 10.0)]);
        let (mut boosted, mut plain) = (pts.clone(), pts);
        weigh(&mut boosted, true, &ZERO_DRAWS);
        weigh(&mut plain, false, &ZERO_DRAWS);
        assert_eq!(boosted, plain);
    }

    #[test]
    fn one_point_has_no_outline() {
        let path = outline_path(at(&[(5.0, 5.0)]), 6.0, 0.25, false, &ZERO_DRAWS);
        assert_eq!(path, PathData::default());
    }
}
