//! §6.4 speed-adaptive smoothing.

use super::{Pt, clamp01, dot, norm};
use crate::math::pow;

/// §6.4 (`a helper`): a running average of positions, each output point being
/// `avg = alpha·avg + (1 − alpha)·P[i]` with `alpha` = `strength` (0 = none) adapted per
/// point: slow points (`res·speed` < 5) lean toward `1 − (1 − strength)²`, the last four ease
/// off as `(alpha/1.5)^1…4`, the average never trails more than `10/res` behind, sharp corners
/// (cos > 0.3 between the ways back to the average and on to the next point) are kept
/// exactly, and straight runs smooth 3% harder. Other fields are copied. One point passes
/// through.
#[must_use]
pub fn smooth(pts: &[Pt], strength: f64, res: f64) -> Vec<Pt> {
    let n = pts.len();
    if n <= 1 {
        return pts.to_vec();
    }
    let slow_strength = 1.0 - (1.0 - strength) * (1.0 - strength);
    let max_lag = 10.0 / res;
    let mut avg = pts[0].pos;
    let mut out = Vec::with_capacity(n);
    for (i, p) in pts.iter().enumerate() {
        let mut alpha = strength;
        let slow = res * p.speed;
        if p.speed != 0.0 && slow < 5.0 {
            let k = 0.5 * clamp01(1.666_666 - slow / 3.0);
            alpha = slow_strength * k + strength * (1.0 - k);
        }
        if i + 5 > n {
            #[expect(clippy::cast_precision_loss, reason = "the exponent is 1–4")]
            let power = (i + 5 - n) as f64;
            alpha = pow(alpha / 1.5, power);
        }
        let lag = norm(avg - p.pos);
        if max_lag < lag {
            alpha = alpha * 10.0 / res / lag;
        }
        if let Some(next) = pts.get(i + 1) {
            let (back, ahead) = (avg - p.pos, next.pos - p.pos);
            let (back_len, ahead_len) = (norm(back), norm(ahead));
            alpha = if back_len * ahead_len == 0.0 {
                alpha * 1.03
            } else {
                let cos = dot(back, ahead) / back_len / ahead_len;
                if cos > 0.3 {
                    0.0
                } else if cos < -0.966 {
                    alpha * 1.03
                } else {
                    alpha
                }
            };
        }
        avg = avg * alpha + p.pos * (1.0 - alpha);
        out.push(Pt { pos: avg, ..*p });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stroke::{assert_close, metrics, points, v};

    /// Points `step` apart along the x axis, with metrics.
    fn straight(n: u32, step: f64) -> Vec<Pt> {
        let xy: Vec<(f64, f64)> = (0..n).map(|i| (f64::from(i) * step, 0.0)).collect();
        let mut p = points(&xy);
        metrics(&mut p);
        p
    }

    #[test]
    fn smooth_trails_a_fast_straight_stroke() {
        // 10 px steps at res 1, strength 0.5: straight runs smooth 3% harder (×1.03), the lag
        // is capped at 10/res (so it settles at 0.5·1.03·10 = 5.15 behind), and the last four
        // points ease off with (a/1.5)^1…4 (the very last without the straight boost).
        let s = smooth(&straight(10, 10.0), 0.5, 1.0);
        assert_close(
            &s.iter().map(|p| p.pos.x).collect::<Vec<_>>(),
            &[
                0.0, 4.85, 14.85, 24.85, 34.85, 44.85, 56.566_667, 68.855_556, 79.618_519,
                89.876_543,
            ],
        );
        assert!(s.iter().all(|p| p.pos.y == 0.0));
    }

    #[test]
    fn smooth_blends_slow_points_harder_and_keeps_other_fields() {
        // 2 px steps: res·speed < 5, so alpha = 0.75·k + 0.5·(1 − k) with
        // k = 0.5·(1.666666 − 2/3) ≈ 0.5, i.e. ≈ 0.625, ×1.03 → x₁ = 2·(1 − 0.64375).
        let mut p = straight(8, 2.0);
        p[1].weight = 0.25;
        let s = smooth(&p, 0.5, 1.0);
        assert!((s[1].pos.x - 0.712_500_2).abs() < 1e-6, "{s:?}");
        assert_eq!((s[1].weight, s[1].speed, s[1].cum_len), (0.25, 2.0, 2.0));
    }

    #[test]
    fn smooth_keeps_a_sharp_corner() {
        // The turn at (20, 0) doubles back (cos > 0.3): it is kept exactly.
        let mut p = points(&[
            (0.0, 0.0),
            (10.0, 0.0),
            (20.0, 0.0),
            (11.0, 1.0),
            (2.0, 2.0),
            (-7.0, 3.0),
            (-16.0, 4.0),
            (-25.0, 5.0),
        ]);
        metrics(&mut p);
        let s = smooth(&p, 0.5, 1.0);
        assert_eq!(s[2].pos, v(20.0, 0.0));
        assert!((s[3].pos - p[3].pos).len() > 1.0, "{s:?}");
    }

    #[test]
    fn smooth_boosts_at_a_repeated_point() {
        // (10, 0) repeats, so there is no way on to measure: alpha 0.5 still gets ×1.03.
        let mut p = points(&[
            (0.0, 0.0),
            (10.0, 0.0),
            (10.0, 0.0),
            (20.0, 0.0),
            (30.0, 0.0),
            (40.0, 0.0),
            (50.0, 0.0),
        ]);
        metrics(&mut p);
        assert!((smooth(&p, 0.5, 1.0)[1].pos.x - 4.85).abs() < 1e-9);
    }

    #[test]
    fn smooth_passes_a_single_point_through() {
        let p = points(&[(3.0, 4.0)]);
        assert_eq!(smooth(&p, 0.9, 1.0), p);
    }
}
