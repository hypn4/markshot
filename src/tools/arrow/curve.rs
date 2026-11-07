//! The circle a bent arrow follows, and the rule for bending it through a point.

use crate::geom::{V2, circumcenter};
use crate::math::{PI, PI_TRUNCATED, atan2, cos, sin};
use crate::stroke::{Pt, norm};

/// The circle through the tail, the curve point M and the tip.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(super) struct Arc {
    pub center: V2,
    /// 0 when not finite.
    pub radius: f64,
    /// The tail's and the tip's angle seen from the center.
    pub start: f64,
    pub end: f64,
}

impl Arc {
    /// The circle through `p0`, `p1` and the curve point M, `dist` chords from `p0` at `angle`
    /// off the chord. When the three are (nearly) on one line its center is their centroid.
    pub fn new(p0: V2, p1: V2, angle: f64, dist: f64) -> Arc {
        let d = p1 - p0;
        let a = angle + atan2(d.y, d.x);
        let r = norm(d) * dist;
        let m = V2 {
            x: p0.x + r * cos(a),
            y: p0.y + r * sin(a),
        };
        let center = circumcenter(p0, m, p1).unwrap_or(V2 {
            x: (p0.x + m.x + p1.x) / 3.0,
            y: (p0.y + m.y + p1.y) / 3.0,
        });
        let radius = norm(p0 - center);
        Arc {
            center,
            radius: if radius.is_finite() { radius } else { 0.0 },
            start: atan2(p0.y - center.y, p0.x - center.x),
            end: atan2(p1.y - center.y, p1.x - center.x),
        }
    }

    /// 51 points on the circle from angle `from` to `to`, clockwise (angles falling) when `sgn`
    /// is positive, else counter-clockwise, going round less than a full turn.
    pub fn points(&self, from: f64, to: f64, sgn: f64) -> Vec<Pt> {
        let sweep = if sgn > 0.0 {
            from - if from >= to { to } else { to - 2.0 * PI }
        } else {
            (if to >= from { to } else { to + 2.0 * PI }) - from
        };
        (0..=50)
            .map(|i| {
                let a = from + sweep * (sgn * (f64::from(i) / -50.0));
                Pt::new(V2 {
                    x: self.center.x + self.radius * cos(a),
                    y: self.center.y + self.radius * sin(a),
                })
            })
            .collect()
    }
}

/// The bend of an arrow from `p0` to `p1` (y-up px, apart) through `via`:
/// `(mid_angle, mid_dist)`, the angle of `via` off the chord and its distance from `p0` in
/// chords, so that the arc passes through `via`. Within 7 pt of the chord's line the arrow
/// snaps straight: the angle is 0 and the distance is kept within 0.05–0.95.
#[must_use]
pub fn via_to_mid(p0: V2, p1: V2, via: V2, dpr: f64) -> (f64, f64) {
    let chord = norm(p1 - p0);
    let along = norm(via - p0) / chord;
    let off_chord = ((p0.y - via.y) * (p1.x - p0.x) - (p0.x - via.x) * (p1.y - p0.y)).abs() / chord;
    if off_chord < dpr * 7.0 {
        return (0.0, along.clamp(0.05, 0.95));
    }
    let angle = atan2(via.y - p0.y, via.x - p0.x) - atan2(p1.y - p0.y, p1.x - p0.x);
    (wrap(angle, PI_TRUNCATED), along)
}

/// `a` wrapped into (−π, π] with the given π: callers use both the full and the truncated one.
pub(super) fn wrap(a: f64, pi: f64) -> f64 {
    let a = if a < -pi { a + 2.0 * pi } else { a };
    if a > pi { a - 2.0 * pi } else { a }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::PI_TRUNCATED;

    fn v(x: f64, y: f64) -> V2 {
        V2 { x, y }
    }

    /// Asserts that `p` is on `arc`'s circle.
    fn assert_on_circle(arc: &Arc, p: V2) {
        let d = p - arc.center;
        let off = (d.x * d.x + d.y * d.y).sqrt() - arc.radius;
        assert!(off.abs() < 1e-9, "{p:?} is {off} off {arc:?}");
    }

    #[test]
    fn via_near_chord_is_straight() {
        // 5 px off a 200 px chord at dpr 2 snaps straight, still keeping the curve point's
        // distance from the tail, |via − p0| = 100.125 over the chord.
        let (angle, dist) = via_to_mid(v(0.0, 0.0), v(200.0, 0.0), v(100.0, 5.0), 2.0);
        assert_eq!(angle, 0.0);
        assert!((dist - 0.500_624_609_862_519_7).abs() < 1e-12, "{dist}");
    }

    #[test]
    fn via_off_chord_is_curved() {
        let (p0, p1, via) = (v(0.0, 0.0), v(200.0, 0.0), v(100.0, 60.0));
        let (angle, dist) = via_to_mid(p0, p1, via, 2.0);
        // The angle of `via` off the chord, and its distance from `p0` over the chord's length.
        assert!((angle - 0.540_419_500_270_584).abs() < 1e-12, "{angle}");
        assert!((dist - 0.583_095_189_484_53).abs() < 1e-12, "{dist}");
        let arc = Arc::new(p0, p1, angle, dist);
        for p in [p0, via, p1] {
            assert_on_circle(&arc, p);
        }
    }

    #[test]
    fn via_snaps_within_seven_points_of_the_chord() {
        let (p0, p1) = (v(0.0, 0.0), v(200.0, 0.0));
        // 7 pt is 14 px at dpr 2 and 7 px at dpr 1; exactly that far already bends.
        assert_eq!(via_to_mid(p0, p1, v(100.0, 13.9), 2.0).0, 0.0);
        assert_ne!(via_to_mid(p0, p1, v(100.0, 14.0), 2.0).0, 0.0);
        assert_ne!(via_to_mid(p0, p1, v(100.0, -7.0), 1.0).0, 0.0);
        assert_eq!(via_to_mid(p0, p1, v(100.0, -6.9), 1.0).0, 0.0);
    }

    #[test]
    fn via_depth_is_clamped_only_when_snapping() {
        let p0 = v(0.0, 0.0);
        // Snapped straight, the distance is kept within 0.05–0.95 chords: here 1.5 chords past
        // the tip and 0.018 from the tail.
        assert_eq!(
            via_to_mid(p0, v(200.0, 0.0), v(300.0, 5.0), 2.0),
            (0.0, 0.95)
        );
        assert_eq!(via_to_mid(p0, v(200.0, 0.0), v(2.0, 3.0), 2.0), (0.0, 0.05));
        // Bent, it is kept whole, so the arc passes through `via` however far or near it is:
        // 2.06 chords past the tip, or 0.02 chords from the tail.
        for (p1, via, want) in [
            (v(200.0, 0.0), v(400.0, 100.0), 2.061_552_812_808_830_3),
            (v(1000.0, 0.0), v(0.0, 20.0), 0.02),
        ] {
            let (angle, dist) = via_to_mid(p0, p1, via, 2.0);
            assert!((dist - want).abs() < 1e-12, "{via:?}: {dist}");
            assert_on_circle(&Arc::new(p0, p1, angle, dist), via);
        }
    }

    #[test]
    fn via_angle_is_wrapped() {
        // The chord points almost along −x and `via` is down-left of the tail: the raw angle
        // difference is −5.7378, which wraps (with the truncated π) to a slight left bend.
        let (mid_angle, _) = via_to_mid(v(0.0, 0.0), v(-200.0, 1.0), v(-100.0, -60.0), 2.0);
        assert!((mid_angle - 0.545_419_351_4).abs() < 1e-9, "{mid_angle}");
    }

    #[test]
    fn wrap_maps_into_minus_pi_to_pi() {
        assert_eq!(wrap(-4.0, PI_TRUNCATED), -4.0 + 2.0 * PI_TRUNCATED);
        assert_eq!(wrap(4.0, PI_TRUNCATED), 4.0 - 2.0 * PI_TRUNCATED);
        assert_eq!(wrap(1.0, PI_TRUNCATED), 1.0);
        assert_eq!(wrap(PI_TRUNCATED, PI_TRUNCATED), PI_TRUNCATED);
    }

    #[test]
    fn collinear_points_fall_back_to_the_centroid() {
        // A mid angle of π puts M behind the tail, on the chord's line: the circle is then
        // centred on the three points' centroid.
        let arc = Arc::new(v(0.0, 0.0), v(200.0, 0.0), std::f64::consts::PI, 0.5);
        assert!((arc.center.x - 100.0 / 3.0).abs() < 1e-9, "{arc:?}");
        assert!(arc.center.y.abs() < 1e-9, "{arc:?}");
        assert!((arc.radius - 100.0 / 3.0).abs() < 1e-9, "{arc:?}");
    }
}
