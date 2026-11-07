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

    /// Tight `[min_x, min_y, max_x, max_y]` bounds in the output units and axes of `map`.
    ///
    /// Applies `map` to the control and end points, then bounds the resulting cubics
    /// without allocating a transformed path. For affine maps this is the exact
    /// transformed curve, including rotated or y-down image-pixel coordinates.
    /// Isolated moves count as points; a path with no points returns `None`.
    #[must_use]
    pub fn bounds(&self, map: impl Fn(V2) -> V2) -> Option<[f64; 4]> {
        let mut bounds: Option<[f64; 4]> = None;
        let mut current = None;
        let mut start = None;
        let mut include = |p: V2| {
            if let Some(b) = &mut bounds {
                b[0] = b[0].min(p.x);
                b[1] = b[1].min(p.y);
                b[2] = b[2].max(p.x);
                b[3] = b[3].max(p.y);
            } else {
                bounds = Some([p.x, p.y, p.x, p.y]);
            }
        };
        for seg in &self.0 {
            match *seg {
                Seg::Move(p) => {
                    let p = map(p);
                    include(p);
                    current = Some(p);
                    start = Some(p);
                }
                Seg::Line(p) => {
                    let p = map(p);
                    include(p);
                    current = Some(p);
                    start.get_or_insert(p);
                }
                Seg::Cubic(c1, c2, p) => {
                    let p0 = current.unwrap_or_else(|| map(V2::default()));
                    let (c1, c2, p) = (map(c1), map(c2), map(p));
                    let x = cubic_axis_bounds(p0.x, c1.x, c2.x, p.x);
                    let y = cubic_axis_bounds(p0.y, c1.y, c2.y, p.y);
                    include(V2 { x: x[0], y: y[0] });
                    include(V2 { x: x[1], y: y[1] });
                    current = Some(p);
                    start.get_or_insert(p0);
                }
                // Closing adds only a straight segment, whose endpoints are already
                // included, but subsequent curves resume at this subpath's start.
                Seg::Close => current = start,
            }
        }
        bounds
    }
}

/// One coordinate's cubic bounds, in the same units as its four control points.
fn cubic_axis_bounds(p0: f64, p1: f64, p2: f64, p3: f64) -> [f64; 2] {
    let mut bounds = [p0.min(p3), p0.max(p3)];
    let mut include = |t: f64| {
        if t > 0.0 && t < 1.0 {
            // De Casteljau evaluation avoids cancellation near a segment's endpoints.
            let lerp = |a: f64, b: f64| (1.0 - t) * a + t * b;
            let (a, b, c) = (lerp(p0, p1), lerp(p1, p2), lerp(p2, p3));
            let value = lerp(lerp(a, b), lerp(b, c));
            bounds[0] = bounds[0].min(value);
            bounds[1] = bounds[1].max(value);
        }
    };
    // The derivative divided by three is a*t^2 + b*t + c.
    let (d0, d1, d2) = (p1 - p0, p2 - p1, p3 - p2);
    let (a, b, c) = (d0 - 2.0 * d1 + d2, 2.0 * (d1 - d0), d0);
    if a == 0.0 {
        if b != 0.0 {
            include(-c / b);
        }
    } else {
        // Normalize before squaring, and compute the second root through the root
        // product rather than subtracting nearly equal values.
        let scale = a.abs().max(b.abs()).max(c.abs());
        let (a, b, c) = (a / scale, b / scale, c / scale);
        let discriminant = b * b - 4.0 * a * c;
        if discriminant == 0.0 {
            include(-b / (2.0 * a));
        } else if discriminant > 0.0 {
            let numerator = -0.5 * (b + discriminant.sqrt().copysign(b));
            include(numerator / a);
            include(c / numerator);
        }
    }
    bounds
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

    fn assert_bounds(path: &PathData, map: impl Fn(V2) -> V2, expected: [f64; 4]) {
        let actual = path.bounds(map).expect("a path with points has bounds");
        for (actual, expected) in actual.into_iter().zip(expected) {
            assert!((actual - expected).abs() < 1e-12, "{actual} != {expected}");
        }
    }

    #[test]
    fn bounds_include_cubic_peak_without_control_point_hull() {
        let path = PathData(vec![
            Seg::Move(v(0.0, 0.0)),
            Seg::Cubic(v(0.0, 4.0), v(4.0, 4.0), v(4.0, 0.0)),
        ]);
        assert_bounds(&path, |p| p, [0.0, 0.0, 4.0, 3.0]);
    }

    #[test]
    fn bounds_find_extrema_after_rotation_scale_and_translation() {
        let path = PathData(vec![
            Seg::Move(v(0.0, 0.0)),
            Seg::Cubic(v(0.0, 4.0), v(4.0, 4.0), v(4.0, 0.0)),
        ]);
        // A 3-4-5 rotation and scale: x reaches 22/9 at t=1/3, y reaches 65/4 at t=3/4.
        assert_bounds(
            &path,
            |p| v(3.0 * p.x - 4.0 * p.y + 10.0, 4.0 * p.x + 3.0 * p.y - 4.0),
            [2.444_444_444_444_444_6, -4.0, 22.0, 16.25],
        );
    }

    #[test]
    fn bounds_include_both_cubic_extrema_and_constant_axis() {
        let path = PathData(vec![
            Seg::Move(v(2.0, 0.0)),
            Seg::Cubic(v(2.0, 3.0), v(2.0, -3.0), v(2.0, 0.0)),
        ]);
        assert_bounds(
            &path,
            |p| p,
            [2.0, -0.866_025_403_784_438_6, 2.0, 0.866_025_403_784_438_6],
        );
    }

    #[test]
    fn bounds_handle_linear_and_zero_derivatives() {
        let path = PathData(vec![
            Seg::Move(v(0.0, 0.0)),
            Seg::Cubic(v(1.0, 2.0), v(2.0, 2.0), v(3.0, 0.0)),
        ]);
        assert_bounds(&path, |p| p, [0.0, 0.0, 3.0, 1.5]);
        let point = PathData(vec![
            Seg::Move(v(7.0, -2.0)),
            Seg::Cubic(v(7.0, -2.0), v(7.0, -2.0), v(7.0, -2.0)),
            Seg::Close,
        ]);
        assert_bounds(&point, |p| p, [7.0, -2.0, 7.0, -2.0]);
    }

    #[test]
    fn bounds_resume_at_subpath_start_after_close() {
        let path = PathData(vec![
            Seg::Move(v(0.0, 0.0)),
            Seg::Line(v(4.0, -2.0)),
            Seg::Close,
            Seg::Cubic(v(0.0, 4.0), v(4.0, 4.0), v(4.0, 0.0)),
        ]);
        assert_bounds(&path, |p| p, [0.0, -2.0, 4.0, 3.0]);
    }

    #[test]
    fn bounds_include_disjoint_subpaths_and_isolated_moves() {
        let path = PathData(vec![
            Seg::Move(v(-3.0, 2.0)),
            Seg::Line(v(-1.0, 5.0)),
            Seg::Close,
            Seg::Move(v(8.0, -4.0)),
        ]);
        assert_bounds(&path, |p| p, [-3.0, -4.0, 8.0, 5.0]);
        assert_eq!(PathData::default().bounds(|p| p), None);
        assert_eq!(PathData(vec![Seg::Close]).bounds(|p| p), None);
    }

    #[test]
    fn bounds_keep_small_quadratic_roots_when_the_other_root_is_large() {
        let path = PathData(vec![
            Seg::Move(v(0.0, 0.0)),
            Seg::Cubic(v(1.0, -1.0), v(2.0, -1.0), v(3.0, 1e-15)),
        ]);
        // The tiny endpoint perturbation makes the derivative quadratic rather than
        // linear; its in-range root remains near 1/2 and its minimum near -3/4.
        assert_bounds(&path, |p| p, [0.0, -0.75, 3.0, 1e-15]);
    }
}
