//! §6.5 minimum-spacing resampling.

use super::{Pt, dot};

/// §6.5 (`a helper`): thins the stroke to points more than `0.33·spacing` apart. Keeps
/// the first point and each inner one that far from the last kept; the last point is then
/// appended, or replaces the last kept one when closer than that (unless that is the first).
/// Points are copied whole. Two points or fewer pass through.
#[must_use]
pub fn resample(pts: &[Pt], spacing: f64) -> Vec<Pt> {
    let n = pts.len();
    if n <= 2 {
        return pts.to_vec();
    }
    let min = spacing * 0.33;
    let min2 = min * min;
    let mut out = vec![pts[0]];
    let mut last = pts[0].pos;
    // the reference app: the walk stops before the last point, which is handled below.
    for p in &pts[1..n - 1] {
        let d = last - p.pos;
        if min2 < dot(d, d) {
            out.push(*p);
            last = p.pos;
        }
    }
    let end = pts[n - 1];
    let d = last - end.pos;
    if out.len() < 2 || dot(d, d) >= min2 {
        out.push(end);
    } else {
        let k = out.len() - 1;
        out[k] = end;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stroke::points;

    #[test]
    fn resample_keeps_points_over_a_third_of_the_spacing_apart() {
        let xy: Vec<(f64, f64)> = (0..=10).map(|i| (f64::from(i), 0.0)).collect();
        let mut p = points(&xy);
        for q in &mut p {
            q.weight = q.pos.x / 10.0;
        }
        // Spacing 10 keeps 0, 4, 8 (each > 3.3 past the last kept); the end, only 2 past 8,
        // replaces it. Whole points are copied.
        let r = resample(&p, 10.0);
        assert_eq!(
            r.iter().map(|q| q.pos.x).collect::<Vec<_>>(),
            [0.0, 4.0, 10.0]
        );
        assert_eq!(
            r.iter().map(|q| q.weight).collect::<Vec<_>>(),
            [0.0, 0.4, 1.0]
        );
    }

    #[test]
    fn resample_appends_the_end_to_a_lone_start() {
        let r = resample(&points(&[(0.0, 0.0), (1.0, 0.0), (2.0, 0.0)]), 10.0);
        assert_eq!(r.iter().map(|q| q.pos.x).collect::<Vec<_>>(), [0.0, 2.0]);
    }

    #[test]
    fn resample_passes_two_points_through() {
        let p = points(&[(0.0, 0.0), (0.1, 0.0)]);
        assert_eq!(resample(&p, 10.0), p);
    }
}
