//! Weight filters, the weight blur and stroke widths.

use super::{Pt, clamp01, extremes};

/// A running average of the weights, `w = k * w_prev + (1 − k) * w`, seeded with the first
/// weight. Two points or fewer are left alone.
pub fn ema_weight(pts: &mut [Pt], k: f64) {
    if pts.len() <= 2 {
        return;
    }
    let mut w = pts[0].weight;
    for p in pts {
        w = k * w + (1.0 - k) * p.weight;
        p.weight = w;
    }
}

/// Blends each weight with its min–max normalization onto `lo..hi`,
/// `w = (1 − mix) * w + mix * (lo + (hi − lo) * (w − min)/(max − min))`. Equal weights all become
/// `(lo + hi)/2`, whatever `mix`.
#[expect(clippy::float_cmp, reason = "equal extremes are tested exactly")]
pub fn norm_weight(pts: &mut [Pt], lo: f64, hi: f64, mix: f64) {
    let Some((min, max)) = extremes(pts.iter().map(|p| p.weight)) else {
        return;
    };
    if max == min {
        for p in pts {
            p.weight = f64::midpoint(lo, hi);
        }
        return;
    }
    for p in pts {
        p.weight = (1.0 - mix) * p.weight + mix * (lo + (hi - lo) * (p.weight - min) / (max - min));
    }
}

/// A circular box blur of the weights. Each becomes the mean of the `2 * half + 1` weights
/// centred on it, the window wrapping around the ends, with `half = ⌊min(n − 1, span)/2⌋`.
pub fn blur_weight(pts: &mut [Pt], span: usize) {
    let n = pts.len();
    if n == 0 {
        return;
    }
    let half = (n - 1).min(span) / 2;
    let window = 2 * half + 1;
    #[expect(
        clippy::cast_precision_loss,
        reason = "the window is at most span + 1 points"
    )]
    let count = window as f64;
    let weights: Vec<f64> = pts.iter().map(|p| p.weight).collect();
    for (i, p) in pts.iter_mut().enumerate() {
        // Intentional: summed afresh for each point, from the window's first weight on, not as
        // a sliding sum, whose rounding differs.
        let sum = (0..window).fold(0.0, |sum, k| sum + weights[(i + n - half + k) % n]);
        p.weight = sum / count;
    }
}

/// `width = w * (1 − v + 2v * weight)` for variability `v`, which is damped on strokes shorter
/// than five widths: `v * clamp((L/w − 1)/4, 0, 1)` with `L` the last point's `cum_len`
/// (skipped when `L` is 0 and for two points of equal weight). One point is left alone.
#[must_use]
#[expect(clippy::float_cmp, reason = "the two weights are compared exactly")]
pub fn assign_widths(pts: &[Pt], w: f64, variability: f64) -> Vec<Pt> {
    let n = pts.len();
    if n <= 1 {
        return pts.to_vec();
    }
    let mut v = variability;
    let len = pts[n - 1].cum_len;
    if !(n == 2 && pts[0].weight == pts[1].weight) && len != 0.0 {
        v *= clamp01((len / w - 1.0) * 0.25);
    }
    pts.iter()
        .map(|p| Pt {
            width: w * (1.0 - v + v * p.weight + v * p.weight),
            ..*p
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stroke::{assert_close, points};

    fn weighted(weights: &[f64]) -> Vec<Pt> {
        let mut p = points(&vec![(0.0, 0.0); weights.len()]);
        for (q, &w) in p.iter_mut().zip(weights) {
            q.weight = w;
        }
        p
    }

    fn weights(p: &[Pt]) -> Vec<f64> {
        p.iter().map(|q| q.weight).collect()
    }

    fn widths(p: &[Pt]) -> Vec<f64> {
        p.iter().map(|q| q.width).collect()
    }

    #[test]
    fn ema_weight_runs_from_the_first_weight() {
        let mut p = weighted(&[1.0, 0.0, 0.0, 1.0]);
        ema_weight(&mut p, 0.65);
        assert_close(&weights(&p), &[1.0, 0.65, 0.4225, 0.624_625]);
        let mut two = weighted(&[1.0, 0.0]);
        ema_weight(&mut two, 0.65);
        assert_eq!(weights(&two), [1.0, 0.0]);
    }

    #[test]
    fn norm_weight_mixes_in_the_normalized_weight() {
        let mut p = weighted(&[0.2, 0.6, 1.0]);
        norm_weight(&mut p, 0.0, 1.0, 0.3);
        assert_close(&weights(&p), &[0.14, 0.57, 1.0]);
        let mut q = weighted(&[0.2, 0.6, 1.0]);
        norm_weight(&mut q, 0.2, 0.6, 1.0);
        assert_close(&weights(&q), &[0.2, 0.4, 0.6]);
    }

    #[test]
    fn norm_weight_sets_equal_weights_to_the_middle() {
        let mut p = weighted(&[0.4, 0.4, 0.4]);
        norm_weight(&mut p, 0.0, 1.0, 0.3);
        assert_eq!(weights(&p), [0.5, 0.5, 0.5]);
    }

    #[test]
    fn blur_weight_averages_a_window_that_wraps_around() {
        // span 2: three weights each, the ends' windows reaching around to the other end.
        let mut p = weighted(&[3.0, 0.0, 0.0, 0.0, 0.0, 0.0, 6.0]);
        blur_weight(&mut p, 2);
        assert_eq!(weights(&p), [3.0, 1.0, 0.0, 0.0, 0.0, 2.0, 3.0]);
    }

    #[test]
    fn blur_weight_window_stays_shorter_than_the_stroke() {
        // span 41 over 4 points: half = ⌊min(3, 41)/2⌋ = 1, three weights each.
        let mut p = weighted(&[4.0, 0.0, 0.0, 0.0]);
        blur_weight(&mut p, 41);
        assert_close(&weights(&p), &[4.0 / 3.0, 4.0 / 3.0, 0.0, 4.0 / 3.0]);
        let mut one = weighted(&[5.0]);
        blur_weight(&mut one, 41);
        assert_eq!(weights(&one), [5.0]);
    }

    #[test]
    fn assign_widths_varies_with_weight() {
        // Without a stroke length (cum_len 0) the variability is used as is.
        let p = assign_widths(&weighted(&[0.0, 0.5, 1.0]), 10.0, 0.25);
        assert_close(&widths(&p), &[7.5, 10.0, 12.5]);
    }

    #[test]
    fn assign_widths_damps_variability_on_short_strokes() {
        // L = 30 = 3 widths: variability × (3 − 1)/4.
        let mut p = weighted(&[0.0, 0.5, 1.0]);
        p[2].cum_len = 30.0;
        assert_close(
            &widths(&assign_widths(&p, 10.0, 0.25)),
            &[8.75, 10.0, 11.25],
        );
        // Under one width long: no variation at all.
        p[2].cum_len = 5.0;
        assert_close(&widths(&assign_widths(&p, 10.0, 0.25)), &[10.0, 10.0, 10.0]);
        // Two equal weights skip the damping.
        let mut two = weighted(&[1.0, 1.0]);
        two[1].cum_len = 30.0;
        assert_close(&widths(&assign_widths(&two, 10.0, 0.25)), &[12.5, 12.5]);
    }

    #[test]
    fn assign_widths_leaves_a_single_point_alone() {
        let p = weighted(&[1.0]);
        assert_eq!(assign_widths(&p, 10.0, 0.25), p);
    }
}
