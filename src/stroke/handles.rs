//! §6.6 Bézier handles through the points.

use super::{Pt, dot, norm};
use crate::geom::V2;

/// §6.6 (`a helper`): Catmull-Rom-like Bézier handles. Each inner point gets handles
/// parallel to its neighbours' chord, `t`·(its segment length) long on each side, with
/// tension `t = 0.3 + 0.04·(1 − weight)`, or 0.1 at turns sharper than cos −0.1. The first
/// point's `a` and the last's `b` sit on their anchors; their other handles lie 20% of the
/// way to the neighbour's (both at the midpoint for two points). One point is left alone.
pub fn handles(pts: &mut [Pt]) {
    let n = pts.len();
    if n <= 1 {
        return;
    }
    for i in 1..n - 1 {
        let (prev, cur, next) = (pts[i - 1].pos, pts[i].pos, pts[i + 1].pos);
        let cos = dot(prev - cur, next - cur) / norm(cur - prev) / norm(cur - next);
        let t = if cos > -0.1 {
            0.1
        } else {
            (1.0 - pts[i].weight) * 0.04 + 0.3
        };
        let chord = next - prev;
        let chord_len = norm(chord);
        let out_len = norm(next - cur) * t;
        let in_len = norm(prev - cur) * t;
        pts[i].b = Some(V2 {
            x: cur.x + chord.x * out_len / chord_len,
            y: cur.y + chord.y * out_len / chord_len,
        });
        pts[i].a = Some(V2 {
            x: cur.x - chord.x * in_len / chord_len,
            y: cur.y - chord.y * in_len / chord_len,
        });
    }
    pts[0].a = Some(pts[0].pos);
    pts[n - 1].b = Some(pts[n - 1].pos);
    if n == 2 {
        let mid = (pts[0].pos + pts[1].pos) * 0.5;
        pts[0].b = Some(mid);
        pts[1].a = Some(mid);
    } else {
        pts[0].b = Some(pts[0].pos * 0.8 + pts[1].ctrl_in() * 0.2);
        pts[n - 1].a = Some(pts[n - 1].pos * 0.8 + pts[n - 2].ctrl_out() * 0.2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stroke::{points, v};
    use std::f64::consts::FRAC_1_SQRT_2;

    fn assert_near(got: Option<V2>, want: V2) {
        let got = got.expect("handle set");
        assert!((got - want).len() < 1e-6, "{got:?} != {want:?}");
    }

    #[test]
    fn handles_two_points_meet_halfway() {
        let mut p = points(&[(0.0, 0.0), (10.0, 4.0)]);
        handles(&mut p);
        assert_eq!(
            (p[0].a, p[0].b, p[1].a, p[1].b),
            (
                Some(v(0.0, 0.0)),
                Some(v(5.0, 2.0)),
                Some(v(5.0, 2.0)),
                Some(v(10.0, 4.0))
            )
        );
    }

    #[test]
    fn handles_tighten_at_turns() {
        // A right angle (cos 0 > −0.1): tension 0.1 of each 10 px segment, i.e. 1 px along the
        // neighbours' chord (1, 1)/√2. The ends take 20% of their neighbour's handle.
        let mut p = points(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)]);
        handles(&mut p);
        assert_near(p[1].b, v(10.0 + FRAC_1_SQRT_2, FRAC_1_SQRT_2));
        assert_near(p[1].a, v(10.0 - FRAC_1_SQRT_2, -FRAC_1_SQRT_2));
        assert_eq!(p[0].a, Some(v(0.0, 0.0)));
        assert_near(p[0].b, v(1.858_579, -0.141_421));
        assert_near(p[2].a, v(10.141_421, 8.141_421));
        assert_eq!(p[2].b, Some(v(10.0, 10.0)));
    }

    #[test]
    fn handles_lengthen_as_weight_falls() {
        // Straight runs: tension 0.3 + 0.04·(1 − weight) of each 10 px segment.
        let mut p = points(&[(0.0, 0.0), (10.0, 0.0), (20.0, 0.0)]);
        p[1].weight = 0.5;
        handles(&mut p);
        assert_near(p[1].b, v(13.2, 0.0));
        assert_near(p[1].a, v(6.8, 0.0));
        p[1].weight = 0.0;
        handles(&mut p);
        assert_near(p[1].b, v(13.4, 0.0));
    }
}
