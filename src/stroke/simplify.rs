//! Point simplification (Ramer–Douglas–Peucker).

use super::{Pt, norm};
use crate::geom::V2;

/// Ramer–Douglas–Peucker with a distance floor. The ends are kept; within each span between
/// kept points, the point farthest from the span is kept when farther than `eps`, and both
/// halves are searched again. A point's distance is the larger of its distance to the span's
/// segment and a fiftieth of its distance to the nearer end, so long straight runs keep points
/// too; of equally far points the first wins. Points are copied whole. One point passes
/// through.
#[must_use]
pub fn simplify(pts: &[Pt], eps: f64) -> Vec<Pt> {
    let n = pts.len();
    if n <= 1 {
        return pts.to_vec();
    }
    let mut keep = vec![false; n];
    (keep[0], keep[n - 1]) = (true, true);
    split(pts, 0, n - 1, eps, &mut keep);
    pts.iter()
        .zip(keep)
        .filter_map(|(p, kept)| kept.then_some(*p))
        .collect()
}

/// Marks the points to keep strictly between `lo` and `hi`: the farthest, then those on either
/// side of it (recursing on the left part and looping on the right).
fn split(pts: &[Pt], mut lo: usize, hi: usize, eps: f64, keep: &mut [bool]) {
    while hi - lo >= 2 {
        let (from, to) = (pts[lo].pos, pts[hi].pos);
        let chord = to - from;
        let len2 = chord.x * chord.x + chord.y * chord.y;
        let (mut far, mut max) = (lo, 0.0);
        for (i, p) in pts.iter().enumerate().take(hi).skip(lo + 1) {
            let p = p.pos;
            let mut nearest = from;
            if len2 != 0.0 {
                let t = (chord.x * (p.x - from.x) + chord.y * (p.y - from.y)) / len2;
                if t > 1.0 {
                    nearest = to;
                } else if t >= 0.0 {
                    nearest = V2 {
                        x: from.x + chord.x * t,
                        y: from.y + chord.y * t,
                    };
                }
            }
            let to_end = norm(from - p).min(norm(to - p));
            let dist = norm(p - nearest).max(to_end / 50.0);
            if max < dist {
                (far, max) = (i, dist);
            }
        }
        if eps >= max {
            return;
        }
        keep[far] = true;
        split(pts, lo, far, eps, keep);
        lo = far;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stroke::{points, v};

    fn positions(pts: &[Pt]) -> Vec<V2> {
        pts.iter().map(|p| p.pos).collect()
    }

    #[test]
    fn simplify_keeps_points_off_the_chord() {
        // (2, 2) is 2 off the chord and kept. Then (1, 0.3) is 0.495 off (0, 0)–(2, 2), under
        // 0.5, and (3, −0.2) 0.849 off (2, 2)–(4, 0).
        let pts = points(&[(0.0, 0.0), (1.0, 0.3), (2.0, 2.0), (3.0, -0.2), (4.0, 0.0)]);
        assert_eq!(
            positions(&simplify(&pts, 0.5)),
            [v(0.0, 0.0), v(2.0, 2.0), v(3.0, -0.2), v(4.0, 0.0)]
        );
        // At most 0.3 off the chord: only the ends.
        let flat = points(&[(0.0, 0.0), (1.0, 0.3), (2.0, 0.0), (3.0, -0.2), (4.0, 0.0)]);
        assert_eq!(positions(&simplify(&flat, 0.5)), [v(0.0, 0.0), v(4.0, 0.0)]);
    }

    #[test]
    fn simplify_keeps_the_middle_of_a_long_straight_run() {
        // On the chord but 500 from either end: 500/50 = 10 counts as its distance.
        let long = points(&[(0.0, 0.0), (500.0, 0.0), (1000.0, 0.0)]);
        assert_eq!(simplify(&long, 1.0), long);
        // 20 from either end: 0.4, dropped.
        let short = points(&[(0.0, 0.0), (20.0, 0.0), (40.0, 0.0)]);
        assert_eq!(
            positions(&simplify(&short, 1.0)),
            [v(0.0, 0.0), v(40.0, 0.0)]
        );
    }

    #[test]
    fn simplify_copies_kept_points_whole() {
        let mut pts = points(&[(0.0, 0.0), (5.0, 3.0), (10.0, 0.0)]);
        pts[1].weight = 0.25;
        pts[1].b = Some(v(6.0, 3.0));
        assert_eq!(simplify(&pts, 1.0), pts);
    }

    #[test]
    fn simplify_passes_one_or_two_points_through() {
        let one = points(&[(1.0, 1.0)]);
        assert_eq!(simplify(&one, 1.0), one);
        let two = points(&[(0.0, 0.0), (1.0, 1.0)]);
        assert_eq!(simplify(&two, 1.0), two);
    }
}
