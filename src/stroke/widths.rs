//! §6.7–6.8 weight filters and stroke widths.

use super::{Pt, clamp01, extremes};

/// §6.7 (`a helper`): a running average of the weights, `w = k·w_prev + (1 − k)·w`,
/// seeded with the first weight. Two points or fewer are left alone.
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

/// §6.7 (`a helper`): blends each weight with its min–max normalization onto `lo..hi`,
/// `w = (1 − mix)·w + mix·(lo + (hi − lo)·(w − min)/(max − min))`. Equal weights all become
/// `(lo + hi)/2`, whatever `mix`.
#[expect(clippy::float_cmp, reason = "the reference app tests max == min exactly")]
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

/// §6.8 (`a helper`): `width = w·(1 − v + 2v·weight)` for variability `v`, which is
/// damped on strokes shorter than five widths: `v·clamp((L/w − 1)/4, 0, 1)` with `L` the last
/// point's `cum_len` (skipped when `L` is 0 and for two points of equal weight). One point is
/// left alone.
#[must_use]
#[expect(clippy::float_cmp, reason = "the reference app compares the two weights exactly")]
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
