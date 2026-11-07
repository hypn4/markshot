//! The freehand stroke engine shared by the highlighter, sketch arrows and hand-drawn
//! rectangles and ovals: point filters, widths and the outline. The arithmetic (evaluation
//! order, exact comparisons, NaN cases) is deliberate: the rendered pixels depend on it.

mod flatten;
mod handles;
mod outline;
mod resample;
mod simplify;
mod smooth;
mod widths;

pub use flatten::flatten;
pub use handles::handles;
pub use outline::outline;
pub use resample::resample;
pub use simplify::simplify;
pub use smooth::smooth;
pub use widths::{assign_widths, blur_weight, ema_weight, norm_weight};

use crate::geom::{PathData, Seg, V2};
use crate::math::atan2;

/// 10° rounded to 0.174533 rad: the half-width of [`h_snap`]'s window.
const SNAP_WINDOW: f64 = 0.174_533;

/// One stroke point.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Pt {
    pub pos: V2,
    /// 0–1, slow ⇒ heavy. Arrow stores its taper parameter `t` here.
    pub weight: f64,
    /// Distance from the previous raw point ([`metrics`]); slows the smoothing.
    pub speed: f64,
    /// Stroke width here ([`assign_widths`]).
    pub width: f64,
    /// Incoming Bézier handle (second control of the segment ending here); `None` is unset.
    pub a: Option<V2>,
    /// Outgoing Bézier handle (first control of the next segment); `None` is unset.
    pub b: Option<V2>,
    /// Distance from the previous point ([`metrics`]).
    pub len: f64,
    /// Length along the stroke up to here ([`metrics`]).
    pub cum_len: f64,
}

impl Pt {
    /// A raw input point: everything but the position zero, handles unset.
    #[must_use]
    pub fn new(pos: V2) -> Pt {
        Pt {
            pos,
            weight: 0.0,
            speed: 0.0,
            width: 0.0,
            a: None,
            b: None,
            len: 0.0,
            cum_len: 0.0,
        }
    }

    /// The incoming control point: `a`, or the anchor when unset.
    fn ctrl_in(&self) -> V2 {
        self.a.unwrap_or(self.pos)
    }

    /// The outgoing control point: `b`, or the anchor when unset.
    fn ctrl_out(&self) -> V2 {
        self.b.unwrap_or(self.pos)
    }
}

/// Each point's distance from the previous one (`len` and `speed`), the running length
/// `cum_len` (continuing the first point's), and `weight` = 1 − speed normalized over the
/// stroke, so slow ⇒ heavy. The first weight is then pinned to 0.7 and the last copies its
/// neighbour's. Two points only get weight 1; one is left alone. If all points coincide the
/// weights are NaN ([`outline`] reads them as 0.5).
pub fn metrics(pts: &mut [Pt]) {
    let n = pts.len();
    if n <= 1 {
        return;
    }
    if n == 2 {
        pts[0].weight = 1.0;
        pts[1].weight = 1.0;
        return;
    }
    let (mut prev, mut cum) = (pts[0].pos, pts[0].cum_len);
    for p in pts.iter_mut() {
        let len = norm(prev - p.pos);
        cum += len;
        (p.len, p.speed, p.cum_len) = (len, len, cum);
        prev = p.pos;
    }
    let Some((min, max)) = extremes(pts.iter().map(|p| p.speed)) else {
        return;
    };
    for p in pts.iter_mut() {
        p.weight = 1.0 - (p.speed - min) / (max - min);
    }
    pts[0].weight = 0.7;
    pts[n - 1].weight = pts[n - 2].weight;
}

/// Shape correction "h": pulls each point's y toward the first point's row by
/// `w = clamp(1 − angle/10°, 0, 1)`, `angle` being its direction from the first point folded
/// into 0–90°. The walk ends after the first point outside the window (`w` = 0) at least
/// `10·res` along the stroke, so `cum_len` must be set ([`metrics`]). Two points or fewer pass
/// through.
#[must_use]
pub fn h_snap(pts: &[Pt], res: f64) -> Vec<Pt> {
    let mut out = pts.to_vec();
    if out.len() <= 2 {
        return out;
    }
    let start = out[0].pos;
    for p in &mut out[1..] {
        let angle = atan2((p.pos.y - start.y).abs(), (p.pos.x - start.x).abs());
        let w = clamp01(1.0 - angle / SNAP_WINDOW);
        p.pos.y = start.y * w + p.pos.y * (1.0 - w);
        if p.cum_len >= res * 10.0 && w <= 0.0 {
            break;
        }
    }
    out
}

/// Scales positions about the origin of `raw` = (x, y, w, h) by `w/raw.w` and `h/raw.h`,
/// i.e. into the drawn rect after a resize. A zero raw extent gives NaN.
#[must_use]
pub fn map_rect(pts: &[Pt], raw: (f64, f64, f64, f64), w: f64, h: f64) -> Vec<Pt> {
    let (x, y, raw_w, raw_h) = raw;
    pts.iter()
        .map(|p| Pt {
            // Intentional: multiplies by the new size before dividing by the raw one, which
            // rounds differently from scaling by `w/raw_w`.
            pos: V2 {
                x: x + w * (p.pos.x - x) / raw_w,
                y: y + h * (p.pos.y - y) / raw_h,
            },
            ..*p
        })
        .collect()
}

/// `moveTo(P0)`, then a cubic to each later point with controls `P[i−1].b` and `P[i].a` (the
/// anchors when unset), everything shifted by `−origin`. Callers close outlines themselves.
#[must_use]
pub fn bezier_path(pts: &[Pt], origin: V2) -> PathData {
    // Fewer than two points make an empty path, without even the moveTo.
    if pts.len() < 2 {
        return PathData::default();
    }
    let mut segs = Vec::with_capacity(pts.len());
    segs.push(Seg::Move(pts[0].pos - origin));
    segs.extend(pts.windows(2).map(|w| {
        Seg::Cubic(
            w[0].ctrl_out() - origin,
            w[1].ctrl_in() - origin,
            w[1].pos - origin,
        )
    }));
    PathData(segs)
}

/// `|v|` as `sqrt(x² + y²)`, not `hypot`, whose rounding differs.
pub(crate) fn norm(v: V2) -> f64 {
    dot(v, v).sqrt()
}

fn dot(u: V2, v: V2) -> f64 {
    u.x * v.x + u.y * v.y
}

/// Clamps to 0–1 as `v < 0 ? 0 : fmin(v, 1)`, so NaN becomes 1.
fn clamp01(v: f64) -> f64 {
    if v < 0.0 { 0.0 } else { v.min(1.0) }
}

/// `(min, max)` by strict comparison: the first value, replaced by each later one below
/// (above) it. `None` when empty.
fn extremes(mut vals: impl Iterator<Item = f64>) -> Option<(f64, f64)> {
    let first = vals.next()?;
    Some(vals.fold((first, first), |(lo, hi), v| {
        (if v < lo { v } else { lo }, if hi < v { v } else { hi })
    }))
}

#[cfg(test)]
fn v(x: f64, y: f64) -> V2 {
    V2 { x, y }
}

/// Raw points at the given positions.
#[cfg(test)]
fn points(xy: &[(f64, f64)]) -> Vec<Pt> {
    xy.iter().map(|&(x, y)| Pt::new(v(x, y))).collect()
}

#[cfg(test)]
fn assert_close(got: &[f64], want: &[f64]) {
    assert_eq!(got.len(), want.len(), "{got:?} != {want:?}");
    for (g, w) in got.iter().zip(want) {
        assert!((g - w).abs() < 1e-6, "{got:?} != {want:?}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_weights() {
        let mut p = points(&[(0.0, 0.0), (1.0, 0.0), (4.0, 0.0)]);
        metrics(&mut p);
        // Speeds 0, 1, 3 → weights 1, 2/3, 0; then the first is pinned to 0.7 and the last
        // copies its neighbour.
        assert_close(
            &p.iter().map(|p| p.weight).collect::<Vec<_>>(),
            &[0.7, 2.0 / 3.0, 2.0 / 3.0],
        );
        assert_eq!(
            p.iter().map(|p| p.cum_len).collect::<Vec<_>>(),
            [0.0, 1.0, 4.0]
        );
        assert_eq!(
            p.iter().map(|p| p.speed).collect::<Vec<_>>(),
            [0.0, 1.0, 3.0]
        );
        assert_eq!(p.iter().map(|p| p.len).collect::<Vec<_>>(), [0.0, 1.0, 3.0]);
    }

    #[test]
    fn metrics_two_points() {
        let mut p = points(&[(0.0, 0.0), (3.0, 4.0)]);
        metrics(&mut p);
        assert_eq!(p.iter().map(|p| p.weight).collect::<Vec<_>>(), [1.0, 1.0]);
        // No lengths for two points.
        assert_eq!(p.iter().map(|p| p.cum_len).collect::<Vec<_>>(), [0.0, 0.0]);
    }

    #[test]
    fn h_snap_flattens_near_horizontal() {
        // 2.86° off the start: y is pulled toward the start row by w = 1 − 2.86°/10°, i.e.
        // keeps 28.624% of its offset.
        let mut gentle = points(&[
            (0.0, 0.0),
            (25.0, 1.25),
            (50.0, 2.5),
            (75.0, 3.75),
            (100.0, 5.0),
        ]);
        metrics(&mut gentle);
        let snapped = h_snap(&gentle, 1.0);
        assert_eq!(
            snapped.iter().map(|p| p.pos.x).collect::<Vec<_>>(),
            [0.0, 25.0, 50.0, 75.0, 100.0]
        );
        assert_close(
            &snapped.iter().map(|p| p.pos.y).collect::<Vec<_>>(),
            &[0.0, 0.357_800_5, 0.715_601, 1.073_401_5, 1.431_202],
        );
        // 16.7° is outside the 10° window: nothing moves.
        let mut steep = points(&[
            (0.0, 0.0),
            (25.0, 7.5),
            (50.0, 15.0),
            (75.0, 22.5),
            (100.0, 30.0),
        ]);
        metrics(&mut steep);
        assert_eq!(h_snap(&steep, 1.0), steep);
    }

    #[test]
    fn h_snap_stops_at_the_first_unsnapped_point_past_ten_res() {
        // res 2: the first point outside the window 30 ≥ 20 along ends the walk …
        let mut far = points(&[(0.0, 0.0), (0.0, 30.0), (100.0, 1.0)]);
        metrics(&mut far);
        assert_eq!(h_snap(&far, 2.0)[2].pos.y, 1.0);
        // … one only 15 along does not, so the 0.57° point after it is snapped.
        let mut near = points(&[(0.0, 0.0), (0.0, 15.0), (100.0, 1.0)]);
        metrics(&mut near);
        let snapped = h_snap(&near, 2.0);
        assert_eq!(snapped[1].pos.y, 15.0);
        assert!((snapped[2].pos.y - 0.057_294).abs() < 1e-6, "{snapped:?}");
    }

    #[test]
    fn map_rect_scales_about_the_raw_origin() {
        let mut p = points(&[(10.0, 20.0), (60.0, 45.0)]);
        p[1].weight = 0.25;
        let m = map_rect(&p, (10.0, 20.0, 100.0, 50.0), 200.0, 25.0);
        assert_eq!(m[0].pos, v(10.0, 20.0));
        assert_eq!(m[1].pos, v(110.0, 32.5));
        assert_eq!(m[1].weight, 0.25);
    }

    #[test]
    fn bezier_path_uses_handles_or_anchors_relative_to_origin() {
        let mut p = points(&[(10.0, 20.0), (30.0, 20.0)]);
        p[0].b = Some(v(15.0, 25.0));
        assert_eq!(
            bezier_path(&p, v(10.0, 10.0)),
            PathData(vec![
                Seg::Move(v(0.0, 10.0)),
                Seg::Cubic(v(5.0, 15.0), v(20.0, 10.0), v(20.0, 10.0)),
            ])
        );
    }

    #[test]
    fn bezier_path_needs_two_points() {
        assert_eq!(
            bezier_path(&points(&[(1.0, 2.0)]), V2::default()),
            PathData::default()
        );
    }
}
