//! Flattening: the Bézier path through the points as a polyline.

use super::{Pt, dot, norm};

/// The Bézier path through the points as a polyline. Unset handles become their anchors.
/// Each segment emits its start point, the cubic sampled at `t = k/N`
/// (`N = ⌈1.2·chord/step⌉`, `k < N`) wherever at least `step` past the last emitted point —
/// weight interpolated, other fields zero, handles unset — and its end point: just the start
/// for a zero chord, both ends for a 1.2·chord under `step`. Joints therefore repeat, and the
/// last point is appended once more.
///
/// `step` must be positive and finite; the output grows as `1.2·chord/step`, so callers keep
/// both the points and `step` bounded (the resolver bounds coordinates and dpr).
#[must_use]
pub fn flatten(pts: &[Pt], step: f64) -> Vec<Pt> {
    debug_assert!(step > 0.0 && step.is_finite(), "flatten step {step}");
    // Fewer than two points pass through as they are.
    if pts.len() < 2 {
        return pts.to_vec();
    }
    let mut out = Vec::new();
    for pair in pts.windows(2) {
        segment(anchored(&pair[0]), anchored(&pair[1]), step, &mut out);
    }
    out.push(anchored(&pts[pts.len() - 1]));
    out
}

/// `p` with unset handles on its anchor.
fn anchored(p: &Pt) -> Pt {
    Pt {
        a: Some(p.ctrl_in()),
        b: Some(p.ctrl_out()),
        ..*p
    }
}

/// One cubic from `from` to `to` (handles already anchored) as polyline points.
fn segment(from: Pt, to: Pt, step: f64, out: &mut Vec<Pt>) {
    out.push(from);
    let chord = norm(to.pos - from.pos) * 1.2;
    if chord == 0.0 {
        return;
    }
    if chord < step {
        out.push(to);
        return;
    }
    let samples = (chord / step).ceil();
    let (c1, c2) = (from.ctrl_out(), to.ctrl_in());
    let mut last = from.pos;
    let mut k = 0.0;
    while k < samples {
        let t = k / samples;
        let mt = 1.0 - t;
        let pos = from.pos * (mt * mt * mt)
            + c1 * (3.0 * mt * mt * t)
            + c2 * (3.0 * mt * t * t)
            + to.pos * (t * t * t);
        let gap = pos - last;
        if dot(gap, gap) >= step * step {
            out.push(Pt {
                weight: to.weight * t + from.weight * mt,
                ..Pt::new(pos)
            });
            last = pos;
        }
        k += 1.0;
    }
    out.push(to);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stroke::{points, v};

    #[test]
    fn flatten_spacing() {
        // A straight cubic (handles unset ⇒ on the anchors): ⌈1.2·100/4⌉ = 30 samples, 20 of
        // them at least 4 past the previous point, between the two ends; the end point is
        // then appended once more.
        let out = flatten(&points(&[(0.0, 0.0), (100.0, 0.0)]), 4.0);
        let n = out.len();
        assert_eq!(n, 23, "{out:?}");
        assert_eq!(out[0].pos, v(0.0, 0.0));
        assert_eq!(
            (out[n - 2].pos, out[n - 1].pos),
            (v(100.0, 0.0), v(100.0, 0.0))
        );
        for pair in out[..n - 2].windows(2) {
            assert!((pair[1].pos - pair[0].pos).len() >= 4.0, "{out:?}");
        }
        assert!(out.iter().all(|p| p.pos.y == 0.0));
    }

    #[test]
    fn flatten_samples_follow_the_cubic_and_lerp_the_weight() {
        let mut p = points(&[(0.0, 0.0), (100.0, 0.0)]);
        p[1].weight = 1.0;
        let out = flatten(&p, 4.0);
        let samples = &out[1..out.len() - 2];
        for s in samples {
            // weight = t, and x(t) = 100·(3t² − 2t³) for controls on the anchors.
            let t = s.weight;
            assert!(t > 0.0 && t < 1.0, "{s:?}");
            assert!(
                (s.pos.x - 100.0 * (3.0 * t * t - 2.0 * t * t * t)).abs() < 1e-9,
                "{s:?}"
            );
            assert_eq!((s.a, s.b, s.width, s.cum_len), (None, None, 0.0, 0.0));
        }
    }

    #[test]
    fn flatten_repeats_joints() {
        // The first segment (chord 1.2 < 4) is just its ends; of the second's (chord 10.8)
        // samples at t = 1/3 and 2/3 only the latter is 4 past the start.
        let out = flatten(&points(&[(0.0, 0.0), (1.0, 0.0), (10.0, 0.0)]), 4.0);
        let xs: Vec<f64> = out.iter().map(|p| p.pos.x).collect();
        assert_eq!(xs.len(), 6, "{xs:?}");
        for (x, want) in xs.iter().zip([0.0, 1.0, 1.0, 7.666_667, 10.0, 10.0]) {
            assert!((x - want).abs() < 1e-6, "{xs:?}");
        }
    }

    #[test]
    fn flatten_degenerate_inputs() {
        let one = points(&[(5.0, 5.0)]);
        assert_eq!(flatten(&one, 4.0), one);
        // A zero-length segment yields only its start; the end is still appended.
        let same = flatten(&points(&[(5.0, 5.0), (5.0, 5.0)]), 4.0);
        assert_eq!(
            same.iter().map(|p| p.pos).collect::<Vec<_>>(),
            [v(5.0, 5.0), v(5.0, 5.0)]
        );
    }
}
