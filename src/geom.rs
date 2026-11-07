//! 2-D vectors and path data in y-up image-pixel space.

use std::ops::{Add, Mul, Neg, Sub};

use crate::math::{atan2, cos, hypot, sin};

/// Control-point distance, as a fraction of the radius, for a quarter circle as one cubic:
/// `4/3 * (√2 − 1)`.
const CIRCLE_KAPPA: f64 = 0.552_284_749_830_793_6;

#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct V2 {
    pub x: f64,
    pub y: f64,
}

impl V2 {
    /// The unit vector at angle `a` (radians, counter-clockwise from +x in y-up space).
    #[must_use]
    pub fn dir(a: f64) -> V2 {
        V2 {
            x: cos(a),
            y: sin(a),
        }
    }

    #[must_use]
    pub fn len(self) -> f64 {
        hypot(self.x, self.y)
    }

    /// `atan2(y, x)`.
    #[must_use]
    pub fn angle(self) -> f64 {
        atan2(self.y, self.x)
    }

    /// Rotated by `a` radians, counter-clockwise in y-up space.
    #[must_use]
    pub fn rot(self, a: f64) -> V2 {
        let (s, c) = (sin(a), cos(a));
        V2 {
            x: self.x * c - self.y * s,
            y: self.x * s + self.y * c,
        }
    }
}

impl Add for V2 {
    type Output = V2;
    fn add(self, o: V2) -> V2 {
        V2 {
            x: self.x + o.x,
            y: self.y + o.y,
        }
    }
}

impl Sub for V2 {
    type Output = V2;
    fn sub(self, o: V2) -> V2 {
        V2 {
            x: self.x - o.x,
            y: self.y - o.y,
        }
    }
}

impl Neg for V2 {
    type Output = V2;
    fn neg(self) -> V2 {
        V2 {
            x: -self.x,
            y: -self.y,
        }
    }
}

impl Mul<f64> for V2 {
    type Output = V2;
    fn mul(self, k: f64) -> V2 {
        V2 {
            x: self.x * k,
            y: self.y * k,
        }
    }
}

/// The center of the circle through `a`, `b`, `c`, or `None` when the points are (nearly)
/// collinear or the result is not finite.
#[must_use]
pub fn circumcenter(a: V2, b: V2, c: V2) -> Option<V2> {
    let d = 2.0 * (a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y));
    if !d.is_finite() || d.abs() < 1e-9 {
        return None;
    }
    let (a2, b2, c2) = (
        a.x * a.x + a.y * a.y,
        b.x * b.x + b.y * b.y,
        c.x * c.x + c.y * c.y,
    );
    let center = V2 {
        x: (a2 * (b.y - c.y) + b2 * (c.y - a.y) + c2 * (a.y - b.y)) / d,
        y: (a2 * (c.x - b.x) + b2 * (a.x - c.x) + c2 * (b.x - a.x)) / d,
    };
    (center.x.is_finite() && center.y.is_finite()).then_some(center)
}

/// One path command; all points are absolute.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Seg {
    Move(V2),
    Line(V2),
    /// Two control points, then the end point.
    Cubic(V2, V2, V2),
    Close,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct PathData(pub Vec<Seg>);

impl PathData {
    /// Appends a closed circle as four cubics, starting at `c + (r, 0)` and running
    /// counter-clockwise in y-up space.
    pub fn add_circle(&mut self, c: V2, r: f64) {
        let k = r * CIRCLE_KAPPA;
        let p = |x: f64, y: f64| V2 {
            x: c.x + x,
            y: c.y + y,
        };
        self.0.extend([
            Seg::Move(p(r, 0.0)),
            Seg::Cubic(p(r, k), p(k, r), p(0.0, r)),
            Seg::Cubic(p(-k, r), p(-r, k), p(-r, 0.0)),
            Seg::Cubic(p(-r, -k), p(-k, -r), p(0.0, -r)),
            Seg::Cubic(p(k, -r), p(r, -k), p(r, 0.0)),
            Seg::Close,
        ]);
    }

    /// Appends the closed rect with lower-left corner `(left, bottom)` and size `width` by
    /// `height` (y-up px)
    /// whose corners are quarter circles of radius `r`, at most half the shorter side: four
    /// edges and four cubics, starting at the bottom edge's left end and running
    /// counter-clockwise.
    pub fn add_rounded_rect(&mut self, left: f64, bottom: f64, width: f64, height: f64, r: f64) {
        let r = r.min(width.min(height) / 2.0).max(0.0);
        let k = r * CIRCLE_KAPPA;
        let (right, top) = (left + width, bottom + height);
        let at = |x: f64, y: f64| V2 { x, y };
        self.0.extend([
            Seg::Move(at(left + r, bottom)),
            Seg::Line(at(right - r, bottom)),
            Seg::Cubic(
                at(right - r + k, bottom),
                at(right, bottom + r - k),
                at(right, bottom + r),
            ),
            Seg::Line(at(right, top - r)),
            Seg::Cubic(
                at(right, top - r + k),
                at(right - r + k, top),
                at(right - r, top),
            ),
            Seg::Line(at(left + r, top)),
            Seg::Cubic(
                at(left + r - k, top),
                at(left, top - r + k),
                at(left, top - r),
            ),
            Seg::Line(at(left, bottom + r)),
            Seg::Cubic(
                at(left, bottom + r - k),
                at(left + r - k, bottom),
                at(left + r, bottom),
            ),
            Seg::Close,
        ]);
    }

    /// The same path with `f` applied to every point.
    #[must_use]
    pub fn map(&self, f: impl Fn(V2) -> V2) -> PathData {
        PathData(
            self.0
                .iter()
                .map(|s| match *s {
                    Seg::Move(p) => Seg::Move(f(p)),
                    Seg::Line(p) => Seg::Line(f(p)),
                    Seg::Cubic(c1, c2, p) => Seg::Cubic(f(c1), f(c2), f(p)),
                    Seg::Close => Seg::Close,
                })
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::PI;

    fn v(x: f64, y: f64) -> V2 {
        V2 { x, y }
    }

    fn assert_near(a: V2, b: V2) {
        assert!((a - b).len() < 1e-12, "{a:?} != {b:?}");
    }

    #[test]
    fn vector_ops() {
        assert_eq!(v(1.0, 2.0) + v(3.0, -1.0), v(4.0, 1.0));
        assert_eq!(v(1.0, 2.0) - v(3.0, -1.0), v(-2.0, 3.0));
        assert_eq!(-v(1.0, -2.0), v(-1.0, 2.0));
        assert_eq!(v(1.0, -2.0) * 3.0, v(3.0, -6.0));
        assert_eq!(v(3.0, 4.0).len(), 5.0);
        assert_eq!(v(0.0, 2.0).angle(), PI / 2.0);
        assert_near(V2::dir(PI / 2.0), v(0.0, 1.0));
        // Counter-clockwise in y-up space.
        assert_near(v(1.0, 0.0).rot(PI / 2.0), v(0.0, 1.0));
        assert_near(v(1.0, 1.0).rot(PI), v(-1.0, -1.0));
    }

    #[test]
    fn circumcenter_basic() {
        assert_eq!(
            circumcenter(
                V2 { x: 0., y: 0. },
                V2 { x: 2., y: 0. },
                V2 { x: 1., y: 1. }
            ),
            Some(V2 { x: 1., y: 0. })
        );
    }

    #[test]
    fn circumcenter_of_collinear_or_non_finite_points_is_none() {
        assert_eq!(circumcenter(v(0.0, 0.0), v(1.0, 1.0), v(2.0, 2.0)), None);
        assert_eq!(circumcenter(v(3.0, 3.0), v(3.0, 3.0), v(3.0, 3.0)), None);
        assert_eq!(
            circumcenter(v(f64::NAN, 0.0), v(1.0, 0.0), v(0.0, 1.0)),
            None
        );
    }

    #[test]
    fn add_circle_is_four_closed_counter_clockwise_cubics() {
        let mut p = PathData::default();
        p.add_circle(v(10.0, 20.0), 5.0);
        assert_eq!(p.0.len(), 6);
        assert_eq!(p.0[0], Seg::Move(v(15.0, 20.0)));
        let ends: Vec<V2> = p.0[1..5]
            .iter()
            .map(|s| match s {
                Seg::Cubic(_, _, e) => *e,
                other => panic!("expected a cubic, got {other:?}"),
            })
            .collect();
        assert_eq!(
            ends,
            [v(10.0, 25.0), v(5.0, 20.0), v(10.0, 15.0), v(15.0, 20.0)]
        );
        let Seg::Cubic(c1, c2, _) = p.0[1] else {
            unreachable!()
        };
        let k = 5.0 * CIRCLE_KAPPA;
        assert_eq!((c1, c2), (v(15.0, 20.0 + k), v(10.0 + k, 25.0)));
        assert_eq!(p.0[5], Seg::Close);
    }

    #[test]
    fn map_transforms_every_point() {
        let p = PathData(vec![
            Seg::Move(v(1.0, 2.0)),
            Seg::Line(v(3.0, 4.0)),
            Seg::Cubic(v(5.0, 6.0), v(7.0, 8.0), v(9.0, 10.0)),
            Seg::Close,
        ]);
        let q = p.map(|p| p * 2.0 + v(1.0, 0.0));
        assert_eq!(
            q,
            PathData(vec![
                Seg::Move(v(3.0, 4.0)),
                Seg::Line(v(7.0, 8.0)),
                Seg::Cubic(v(11.0, 12.0), v(15.0, 16.0), v(19.0, 20.0)),
                Seg::Close,
            ])
        );
    }
}
