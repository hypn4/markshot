//! The variable-width outline polygon, with round joins and end caps.

use super::{Pt, dot, handles, norm, resample};
use crate::geom::V2;
use crate::math::{PI, PI_TRUNCATED, atan2, cos, sin};

/// End caps by style: `(angle°, radius factor)` around the end's outward direction, from the
/// right side to the left. Style 3 is the highlighter's default.
const CAPS: [&[(f64, f64)]; 7] = [
    // 0: round
    &[
        (-89.0, 1.0),
        (-66.0, 1.01),
        (-35.0, 1.0),
        (0.0, 1.0),
        (35.0, 1.0),
        (66.0, 1.01),
        (89.0, 1.0),
    ],
    // 1: flattened round
    &[
        (-89.0, 1.0),
        (-66.0, 1.03),
        (-35.0, 1.0),
        (35.0, 1.0),
        (66.0, 1.03),
        (89.0, 1.0),
    ],
    // 2: notched
    &[
        (-89.0, 1.0),
        (-66.0, 1.06),
        (-45.0, 1.0),
        (0.0, 0.65),
        (45.0, 1.0),
        (66.0, 1.06),
        (89.0, 1.0),
    ],
    // 3: double lobe
    &[
        (-89.0, 1.0),
        (-66.0, 1.08),
        (-49.0, 0.993),
        (-39.0, 0.666),
        (0.0, 0.693),
        (39.0, 0.666),
        (49.0, 0.993),
        (66.0, 1.08),
        (89.0, 1.0),
    ],
    // 4: butt
    &[
        (-90.0, 0.99),
        (-90.0, 0.98),
        (0.0, 0.0),
        (90.0, 0.98),
        (90.0, 0.99),
    ],
    // 5: deep notch
    &[
        (-84.0, 0.95),
        (-80.0, 0.8),
        (0.0, 0.15),
        (80.0, 0.8),
        (84.0, 0.945),
        (87.0, 0.986),
        (89.9, 1.0),
    ],
    // 6: chisel
    &[(-20.0, 1.0), (20.0, 1.0)],
];

/// The outline polygon of a stroke whose points carry widths (see
/// [`assign_widths`](super::assign_widths)). Each point is offset by half its width to both
/// sides, along the normal of its neighbours' chord; a turn sharper than cos −0.3 adds six
/// points fanned around its outer side; each side is thinned by [`resample()`] with spacing
/// `w`. The polygon runs along the right side, around the end cap, back along the left side
/// and around the start cap (`cap` 0–6, others round), each cap facing away from its end's
/// inner handle; [`handles()`] then make it a smooth closed curve for
/// [`bezier_path`](super::bezier_path). NaN weights count as 0.5. One point passes through.
#[must_use]
pub fn outline(pts: &[Pt], cap: u8, w: f64) -> Vec<Pt> {
    let n = pts.len();
    if n <= 1 {
        return pts.to_vec();
    }
    let (first, last) = (&pts[0], &pts[n - 1]);
    let (start_dir, end_dir) = (pts[1].pos - first.pos, last.pos - pts[n - 2].pos);
    let at = |i: usize| {
        let mut p = pts[i];
        if p.weight.is_nan() {
            p.weight = 0.5;
        }
        p
    };
    let (mut left, mut right) = (Vec::new(), Vec::new());
    push_sides(&at(0), start_dir, &mut left, &mut right);
    for i in 1..n - 1 {
        let p = at(i);
        push_sides(&p, pts[i + 1].pos - pts[i - 1].pos, &mut left, &mut right);
        push_join(pts[i - 1].pos, &p, pts[i + 1].pos, &mut left, &mut right);
    }
    push_sides(&at(n - 1), end_dir, &mut left, &mut right);
    let start_out = end_handle(first.b, first.pos, first.pos + third(start_dir));
    let end_in = end_handle(last.a, last.pos, last.pos - third(end_dir));
    let mut out = resample(&right, w);
    push_cap(&mut out, last, end_in, cap);
    left.reverse();
    out.extend(resample(&left, w));
    push_cap(&mut out, first, start_out, cap);
    handles(&mut out);
    out
}

/// `d/3`, dividing each axis (not multiplying by 1/3, which rounds differently).
fn third(d: V2) -> V2 {
    V2 {
        x: d.x / 3.0,
        y: d.y / 3.0,
    }
}

/// An end point's inner handle, per axis: a component equal to the anchor's counts as unset,
/// and unset components come from `fallback` (a third of the end segment from the anchor).
#[expect(
    clippy::float_cmp,
    reason = "each handle component is compared to the anchor exactly"
)]
fn end_handle(h: Option<V2>, anchor: V2, fallback: V2) -> V2 {
    let Some(h) = h else {
        return fallback;
    };
    V2 {
        x: if h.x == anchor.x { fallback.x } else { h.x },
        y: if h.y == anchor.y { fallback.y } else { h.y },
    }
}

/// Offsets `p` by half its width along the right-hand normal of `t`: `p − n` joins the left
/// side, `p + n` the right. Nothing for a (near) zero `t`.
fn push_sides(p: &Pt, t: V2, left: &mut Vec<Pt>, right: &mut Vec<Pt>) {
    if t.x.abs() < 1e-6 && t.y.abs() < 1e-6 {
        return;
    }
    let len = norm(t).max(1e-6);
    let hw = p.width * 0.5;
    let n = V2 {
        x: hw * t.y / len,
        y: -(hw * t.x) / len,
    };
    let side = |pos| Pt {
        weight: p.weight,
        speed: p.speed,
        width: p.width,
        ..Pt::new(pos)
    };
    left.push(side(p.pos - n));
    right.push(side(p.pos + n));
}

/// At a turn sharper than cos −0.3, six copies of `p` half its width out, fanned over
/// ±5/14 of a right angle around the way in, onto the outer side: the right one for a left
/// turn (y-up), else the left one, in the order that side is built.
fn push_join(prev: V2, p: &Pt, next: V2, left: &mut Vec<Pt>, right: &mut Vec<Pt>) {
    let (back, ahead) = (prev - p.pos, next - p.pos);
    let (back_len, ahead_len) = (norm(back), norm(ahead));
    if back_len * ahead_len == 0.0 || dot(back, ahead) / back_len / ahead_len <= -0.3 {
        return;
    }
    let way_in = p.pos - prev;
    let theta = atan2(way_in.y, way_in.x);
    let left_turn = ahead.x * way_in.y <= ahead.y * way_in.x;
    for k in 1..=6 {
        let offset = f64::from(k) / 7.0 - 0.5;
        let spread = if left_turn { offset } else { -offset };
        // Intentional: the full π here, not the truncated one the caps use.
        let phi = theta + spread * PI * 0.5;
        let point = Pt {
            pos: V2 {
                x: p.width * cos(phi) * 0.5 + p.pos.x,
                y: p.width * sin(phi) * 0.5 + p.pos.y,
            },
            ..*p
        };
        if left_turn {
            right.push(point);
        } else {
            left.push(point);
        }
    }
}

/// The cap points around end point `p`, facing away from its inner handle `handle`.
fn push_cap(out: &mut Vec<Pt>, p: &Pt, handle: V2, style: u8) {
    let theta = atan2(p.pos.y - handle.y, p.pos.x - handle.x);
    // Intentional: through degrees and back with the truncated π.
    let deg = theta * 180.0 / PI_TRUNCATED;
    let r = p.width * 0.5;
    let shape = CAPS.get(usize::from(style)).copied().unwrap_or(CAPS[0]);
    out.extend(shape.iter().map(|&(phi, k)| {
        let angle = (deg + phi) * PI_TRUNCATED / 180.0;
        Pt::new(V2 {
            x: p.pos.x + r * k * cos(angle),
            y: p.pos.y + r * k * sin(angle),
        })
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stroke::{assert_close, assign_widths, flatten, points, v};
    use std::f64::consts::FRAC_1_SQRT_2;

    /// A raw stroke whose points are all `width` wide.
    fn wide(xy: &[(f64, f64)], width: f64) -> Vec<Pt> {
        let mut p = points(xy);
        for q in &mut p {
            q.width = width;
        }
        p
    }

    fn contains(o: &[Pt], want: V2) -> bool {
        o.iter().any(|q| (q.pos - want).len() < 1e-5)
    }

    /// The outline of a straight 200 px stroke flattened at step 4, 56 wide, cap 3.
    fn straight_outline() -> Vec<Pt> {
        let p = assign_widths(
            &flatten(&points(&[(0.0, 0.0), (200.0, 0.0)]), 4.0),
            56.0,
            0.0,
        );
        outline(&p, 3, 56.0)
    }

    /// Where the end cap starts: cap points are the first of width 0.
    fn end_cap(o: &[Pt]) -> usize {
        o.iter().position(|q| q.width == 0.0).expect("an end cap")
    }

    #[test]
    fn outline_straight_width() {
        let o = straight_outline();
        let (xs, ys): (Vec<f64>, Vec<f64>) = o.iter().map(|q| (q.pos.x, q.pos.y)).unzip();
        let span = |v: &[f64]| {
            v.iter().copied().fold(f64::MIN, f64::max) - v.iter().copied().fold(f64::MAX, f64::min)
        };
        assert!((span(&ys) - 56.0).abs() <= 0.56, "height {}", span(&ys));
        assert!(span(&xs) >= 200.0, "width {}", span(&xs));
    }

    #[test]
    fn outline_runs_out_along_the_right_side_and_back_along_the_left() {
        // Heading +x (y-up), the right side is y = −28 and comes first; after the end cap
        // the left side, y = +28, runs backwards.
        let o = straight_outline();
        let cap = end_cap(&o);
        assert!(
            o[..cap].iter().all(|q| (q.pos.y + 28.0).abs() < 1e-9),
            "{o:?}"
        );
        let left: Vec<&Pt> = o[cap..].iter().filter(|q| q.width > 0.0).collect();
        assert!(left.len() > 2, "{o:?}");
        assert!(left.iter().all(|q| (q.pos.y - 28.0).abs() < 1e-9), "{o:?}");
        assert!(left.windows(2).all(|w| w[1].pos.x < w[0].pos.x), "{o:?}");
    }

    #[test]
    fn outline_caps_reach_past_the_ends() {
        // How far each cap style reaches beyond its end point, in half-widths
        // (styles 7 and up are round, like 0).
        let reach = [
            1.0,
            0.819_152,
            FRAC_1_SQRT_2,
            0.693,
            0.0,
            0.15,
            0.939_693,
            1.0,
        ];
        let p = wide(&[(0.0, 0.0), (100.0, 0.0)], 20.0);
        for (style, want) in (0u8..).zip(reach) {
            let xs: Vec<f64> = outline(&p, style, 20.0).iter().map(|q| q.pos.x).collect();
            let max = xs.iter().copied().fold(f64::MIN, f64::max);
            let min = xs.iter().copied().fold(f64::MAX, f64::min);
            assert!(
                (max - (100.0 + 10.0 * want)).abs() < 1e-5,
                "style {style}: {xs:?}"
            );
            assert!((min + 10.0 * want).abs() < 1e-5, "style {style}: {xs:?}");
        }
    }

    #[test]
    fn outline_rounds_sharp_turns_on_the_outside() {
        // Turns at (100, 0) sharper than cos −0.3 (hairpins, right angles) get six points half
        // a width out, fanned within ±5/14 of a right angle around the way in (+x) and rising
        // in angle along the outline. A left turn puts them on the right side, before the end
        // cap; a right turn on the left side, after it.
        let fan = [
            -0.560_998_7,
            -0.336_599_2,
            -0.112_199_7,
            0.112_199_7,
            0.336_599_2,
            0.560_998_7,
        ];
        let turns = [
            ((0.0, 10.0), true),
            ((0.0, -10.0), false),
            ((100.0, 100.0), true),
            ((100.0, -100.0), false),
        ];
        for (end, left_turn) in turns {
            let p = wide(&[(0.0, 0.0), (100.0, 0.0), end], 20.0);
            // A tiny spacing keeps every side point.
            let o = outline(&p, 0, 0.001);
            let cap = end_cap(&o);
            let fanned: Vec<(usize, f64)> = o
                .iter()
                .map(|q| q.pos - v(100.0, 0.0))
                .enumerate()
                .filter(|(_, d)| {
                    (d.len() - 10.0).abs() < 1e-9 && (0.05..0.6).contains(&d.angle().abs())
                })
                .map(|(i, d)| (i, d.angle()))
                .collect();
            let angles: Vec<f64> = fanned.iter().map(|&(_, a)| a).collect();
            assert_close(&angles, &fan);
            assert!(
                fanned.iter().all(|&(i, _)| (i < cap) == left_turn),
                "{end:?}: {fanned:?}, end cap at {cap}"
            );
        }
    }

    #[test]
    fn outline_thins_each_side_to_a_third_of_the_width() {
        // The flattened stroke has points 4–7 px apart; the outline's right side (up to the
        // end cap) keeps only points over 0.33·56 apart, bar its last one.
        let o = straight_outline();
        let right = &o[..end_cap(&o)];
        assert!(right.len() > 2, "{right:?}");
        for pair in right[..right.len() - 1].windows(2) {
            assert!((pair[1].pos - pair[0].pos).len() > 0.33 * 56.0, "{right:?}");
        }
    }

    #[test]
    fn outline_adds_no_join_points_on_gentle_bends() {
        let p = wide(&[(0.0, 0.0), (100.0, 0.0), (200.0, 10.0)], 20.0);
        // 3 right-side points, 7 end-cap points, 3 left-side points, 7 start-cap points.
        assert_eq!(outline(&p, 0, 0.001).len(), 20);
    }

    #[test]
    fn outline_caps_face_away_from_the_inner_handles() {
        let mut p = wide(&[(0.0, 0.0), (100.0, 0.0)], 20.0);
        // The round cap's middle point sits half a width out along P0 − P0.b …
        p[0].b = Some(v(10.0, 10.0));
        assert!(contains(&outline(&p, 0, 20.0), v(-7.071_068, -7.071_068)));
        // … and along P1 − P1.a at the other end.
        p[1].a = Some(v(90.0, 10.0));
        assert!(contains(&outline(&p, 0, 20.0), v(107.071_068, -7.071_068)));
        // A component equal to the anchor's counts as unset: b = (0 + 100/3, 10).
        p[0].b = Some(v(0.0, 10.0));
        assert!(contains(&outline(&p, 0, 20.0), v(-9.578_263, -2.873_479)));
    }

    #[test]
    fn outline_treats_nan_weights_as_half() {
        let mut p = wide(&[(0.0, 0.0), (50.0, 0.0), (100.0, 0.0)], 20.0);
        for q in &mut p {
            q.weight = f64::NAN;
        }
        let o = outline(&p, 0, 20.0);
        // Side points carry the weight; cap points have width 0 and weight 0.
        let sides: Vec<f64> = o
            .iter()
            .filter(|q| q.width > 0.0)
            .map(|q| q.weight)
            .collect();
        assert_eq!(sides, [0.5; 6], "{o:?}");
        let finite = |h: Option<V2>| h.is_some_and(|h| h.x.is_finite() && h.y.is_finite());
        assert!(o.iter().all(|q| finite(q.a) && finite(q.b)), "{o:?}");
    }

    #[test]
    fn outline_passes_a_single_point_through() {
        let p = wide(&[(1.0, 1.0)], 20.0);
        assert_eq!(outline(&p, 3, 20.0), p);
    }
}
