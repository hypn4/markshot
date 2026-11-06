//! The `sketch` arrow (Arrow.md §3.5–3.7): a hand-drawn stroke and a two-wing head, built by the
//! shared stroke engine and randomised by the seed.

use crate::geom::{PathData, Seg, V2};
use crate::math::{PI, atan2, cos, sin};
use crate::rng::rnd100;
use crate::stroke::{Pt, bezier_path, flatten, handles, norm, outline, resample};

use super::curve::Arc;
use super::profiles::WIDTHS;
use super::{Geometry, taper};

/// The width curves a sketch picks from (Arrow.md §1.2 `EASE_PICK`): thin in the middle, thick
/// to thin, thick–thin–thick.
const EASE_PICK: [u8; 3] = [1, 3, 6];

/// Cosine and sine of 36° and of 0.4 rad, the head's fixed handle turns, as the reference app stores them.
const COS_36: f64 = 0.809_016_994_374_947_5;
const SIN_36: f64 = 0.587_785_252_292_473_1;
const COS_0_4: f64 = 0.921_060_994_002_885_1;
const SIN_0_4: f64 = 0.389_418_342_308_650_5;

/// The arrow's filled paths: straight, the stroke and the head in one path (the reference app adds them up
/// without a union); bent, the two of them.
pub(super) fn paths(g: &Geometry, seed: u64) -> Vec<PathData> {
    let rnd = rnd100(seed);
    // Arrow.md §3.5: a little wider than the line arrow's points, at most a fifteenth of the
    // length; the head is 20·√w long, scaled like the shaped arrow's.
    let w = ((WIDTHS[g.size] * 0.7 + 1.3) * g.dpr).min(g.len / 15.0);
    let sketch = Sketch {
        g,
        w,
        h: g.k_head * (w.sqrt() * 20.0),
        closed: rnd[62] < 0.25,
        ease: EASE_PICK[pick(rnd[18])],
        rnd,
    };
    if g.mid_angle != 0.0 {
        let arc = Arc::new(g.p0, g.p1, g.mid_angle, g.mid_dist);
        if arc.radius != 0.0 && g.mid_angle.abs() >= 1e-6 {
            return sketch.curved(&arc);
        }
    }
    vec![sketch.straight()]
}

/// [`EASE_PICK`]'s index for draw `r`: `Int(2.99·r)`.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "draws are in [0, 1], so this is 0, 1 or 2"
)]
fn pick(r: f64) -> usize {
    (r * 2.99) as usize
}

/// One sketch arrow's draws and sizes.
struct Sketch<'a> {
    g: &'a Geometry,
    rnd: [f64; 100],
    /// The stroke's nominal width.
    w: f64,
    /// The head's length.
    h: f64,
    /// Whether the head is a triangle rather than two wings.
    closed: bool,
    ease: u8,
}

impl Sketch<'_> {
    /// Arrow.md §3.5: one slightly bowed cubic (or a wobbly line) from the tail to short of the
    /// tip, and the head pointing back along the arrow.
    fn straight(&self) -> PathData {
        let (g, rnd, w) = (self.g, &self.rnd, self.w);
        let d = g.p1 - g.p0;
        let theta = atan2(d.y, d.x);
        // The shaft stops short of the tip: further for a closed head, sometimes 2w further.
        let mut shaft = norm(d) - w * 0.8;
        if self.closed {
            shaft = shaft + w * 0.5 + self.h * -0.89;
        }
        if rnd[10] < 0.25 {
            shaft -= w + w;
        }
        let end = V2 {
            x: g.p0.x + cos(theta) * shaft,
            y: g.p0.y + sin(theta) * shaft,
        };
        let pts = if rnd[11] < 0.25 {
            self.wobble(end)
        } else {
            // Handles a third of the way in, tilted up to 5.4° at the tail and 1.8° at the end.
            let mut start = Pt::new(g.p0);
            start.b = Some(tilted(g.p0, end, 0.33, turn((rnd[11] * 0.06 - 0.03) * PI)));
            let mut stop = Pt::new(end);
            stop.a = Some(tilted(end, g.p0, 0.33, turn((rnd[12] * 0.02 - 0.01) * PI)));
            vec![start, stop]
        };
        let mut path = self.stroke(flatten(&pts, 2.0 * g.dpr));
        let back = atan2(g.p0.y - g.p1.y, g.p0.x - g.p1.x);
        let aim = V2 {
            x: g.p1.x + self.h * cos(back),
            y: g.p1.y + self.h * sin(back),
        };
        path.0.extend(self.head(g.p1, aim).0);
        path
    }

    /// Arrow.md §3.5's wobbly shaft to `end`: nine points along it, jittered across by up to
    /// 0.375% of its length (30% of that next to the ends, none at them), smoothed by
    /// [`handles`].
    fn wobble(&self, end: V2) -> Vec<Pt> {
        let (p0, rnd) = (self.g.p0, &self.rnd);
        let d = end - p0;
        let (len, theta) = (norm(d), atan2(d.y, d.x));
        let amp = len * 0.03 * 0.125;
        let mut pts: Vec<Pt> = (0..=8u8)
            .map(|k| {
                let scale = match k {
                    0 | 8 => 0.0,
                    1 | 7 => 0.3,
                    _ => 1.0,
                };
                let jitter = |r: f64| ((amp + amp) * r - amp) * scale;
                let (i, along) = (usize::from(k), len * (f64::from(k) / 8.0));
                Pt::new(V2 {
                    x: p0.x + cos(theta) * along + jitter(rnd[(87 + 2 * i) % 100]),
                    y: p0.y + sin(theta) * along + jitter(rnd[(88 + 2 * i) % 100]),
                })
            })
            .collect();
        handles(&mut pts);
        pts
    }

    /// Arrow.md §3.7: the stroke along the arc to short of the tip, and the head on the tip,
    /// aimed at the arc about `h` behind it.
    fn curved(&self, arc: &Arc) -> Vec<PathData> {
        let (g, w, h) = (self.g, self.w, self.h);
        // +1: clockwise from the tail (angles fall).
        let sgn = if g.mid_angle > 0.0 { 1.0 } else { -1.0 };
        let (mut start, mut end) = (arc.start, arc.end);
        if sgn > 0.0 && start < end {
            start += 2.0 * PI;
        }
        if sgn < 0.0 && end < start {
            end += 2.0 * PI;
        }
        let mut gap = w * 0.7;
        if self.rnd[10] < 0.25 {
            gap = w * 0.7 + w + w;
        }
        if self.closed {
            gap += h * 0.9 + w * -0.5;
        }
        let mut pts = arc.points(start, end + sgn * (gap / arc.radius), sgn);
        // Straight handles a third of the way into the first and last segments.
        let (first, second) = (pts[0].pos, pts[1].pos);
        pts[0].b = Some(V2 {
            x: first.x + (second.x - first.x) / 3.0,
            y: first.y + (second.y - first.y) / 3.0,
        });
        let (prev, last) = (pts[49].pos, pts[50].pos);
        pts[50].a = Some(V2 {
            x: last.x - (last.x - prev.x) / 3.0,
            y: last.y - (last.y - prev.y) / 3.0,
        });
        let body = self.stroke(flatten(&pts, 2.0 * g.dpr));
        // The head looks back along the chord of the arc's last `h` (the reference app aims it through a
        // point the arrow's length away).
        let phi =
            end + sgn * -PI * 0.5 + h * ((sgn + sgn) * PI) / (arc.radius * (2.0 * PI)) * 0.5 + -PI;
        let chord = norm(g.p0 - g.p1);
        let far = V2 {
            x: g.p1.x + chord * cos(phi),
            y: g.p1.y + chord * sin(phi),
        };
        let back = atan2(far.y - g.p1.y, far.x - g.p1.x);
        let aim = V2 {
            x: g.p1.x + h * cos(back),
            y: g.p1.y + h * sin(back),
        };
        vec![body, self.head(g.p1, aim)]
    }

    /// Arrow.md §3.6: the head, its point on `tip` and its wings back toward `aim`: a curved
    /// stroke from one wing's end through `tip` to the other's, and closed, on along the base.
    fn head(&self, tip: V2, aim: V2) -> PathData {
        let rnd = &self.rnd;
        let (w1, w2) = wings(rnd, self.closed, tip, aim, self.w);
        let mut pts = [w1, tip, w2].map(Pt::new).to_vec();
        pts[0].b = Some(tilted(w1, tip, 0.5, turn((rnd[19] * 0.1 - 0.08) * PI)));
        pts[1].a = Some(tilted(tip, w1, 0.1, (COS_36, SIN_36)));
        pts[1].b = Some(tilted(tip, w2, 0.1, (COS_36, -SIN_36)));
        pts[2].a = Some(tilted(w2, tip, 0.5, turn((rnd[20] * 0.1 - 0.02) * PI)));
        if self.closed {
            // The base, back to the first wing or, a quarter of the time, two thirds of the way.
            let end = if rnd[88] < 0.25 {
                toward(w2, w1, 0.66)
            } else {
                w1
            };
            pts[2].b = Some(tilted(w2, end, 0.3, (COS_0_4, SIN_0_4)));
            let mut base = Pt::new(end);
            base.a = Some(tilted(end, w2, 0.3, (COS_0_4, -SIN_0_4)));
            pts.push(base);
        }
        let dpr = self.g.dpr;
        self.stroke(resample(&flatten(&pts, 2.0 * dpr), dpr))
    }

    /// The closed outline of the stroke through `pts`, 0.8w to 1.2w wide along the picked
    /// curve, with round caps.
    fn stroke(&self, mut pts: Vec<Pt>) -> PathData {
        taper(&mut pts, self.w * 0.8, self.w * 1.2, self.ease);
        let mut path = bezier_path(&outline(&pts, 0, self.w), V2::default());
        path.0.push(Seg::Close);
        path
    }
}

/// Arrow.md §3.6: the ends of the head's wings, from its point `tip` back toward `aim`: as far
/// as `aim` (times 0.8–1.2 each when open), turned either way off `aim`'s direction.
fn wings(rnd: &[f64; 100], closed: bool, tip: V2, aim: V2, w: f64) -> (V2, V2) {
    let span = aim - tip;
    let (back, len) = (atan2(span.y, span.x), norm(span));
    let extra = (w.sqrt() - 2.0).max(0.0);
    let mut a1 = (extra + 9.0 + rnd[13] + rnd[13]) * PI / 180.0;
    let mut a2 = (extra + 9.0 + rnd[14] + rnd[14]) * PI / 180.0;
    let (mut f1, mut f2) = (rnd[15] * 0.4 + 0.8, rnd[16] * 0.4 + 0.8);
    if closed {
        a1 *= 0.66;
        a2 = a1;
        (f1, f2) = (1.0, 1.0);
    }
    // the reference app quirk: a1 and a2 are radians already, yet the wings turn π times that: they spread
    // (9 + e + 2r)·π degrees each way, about 28–35° (closed 19–23°), not 9–11°.
    let w1 = wing(tip, span, turn(a1 * PI), len * f1);
    // the reference app turns the second wing off `aim` rebuilt from its angle and length.
    let span = V2 {
        x: tip.x + len * cos(back) - tip.x,
        y: tip.y + len * sin(back) - tip.y,
    };
    let w2 = wing(tip, span, turn(a2 * -PI), len * f2);
    (w1, w2)
}

/// The end of a wing `len` long from `tip`, along `span` turned by the angle whose cosine and
/// sine are `(c, s)`.
fn wing(tip: V2, span: V2, (c, s): (f64, f64), len: f64) -> V2 {
    let dir = atan2(
        span.y * c + tip.y + span.x * s - tip.y,
        tip.x + span.x * c - span.y * s - tip.x,
    );
    V2 {
        x: tip.x + len * cos(dir),
        y: tip.y + len * sin(dir),
    }
}

/// `from + k·(to − from)` as the reference app computes it: by angle and length.
fn toward(from: V2, to: V2, k: f64) -> V2 {
    let d = to - from;
    let (a, r) = (atan2(d.y, d.x), norm(d) * k);
    V2 {
        x: from.x + cos(a) * r,
        y: from.y + sin(a) * r,
    }
}

/// A Bézier handle of `from`: [`toward`] `to` by `k`, then turned about `from` by the angle whose
/// cosine and sine are `(c, s)`.
fn tilted(from: V2, to: V2, k: f64, (c, s): (f64, f64)) -> V2 {
    let v = toward(from, to, k) - from;
    V2 {
        x: from.x + c * v.x - s * v.y,
        y: c * v.y + from.y + s * v.x,
    }
}

/// The cosine and sine of `a`.
fn turn(a: f64) -> (f64, f64) {
    (cos(a), sin(a))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::ArrowStyle;
    use crate::tools::arrow::{coverage, resolved};

    fn assert_near(got: V2, want: V2) {
        assert!(
            (got.x - want.x).abs() < 1e-9 && (got.y - want.y).abs() < 1e-9,
            "{got:?} != {want:?}"
        );
    }

    #[test]
    fn wing_spread_keeps_references_extra_pi() {
        // With every draw 0.5 and w ≤ 4, the reference app's half-angle is (9 + 0 + 2·0.5)° = 10°, in
        // radians, then multiplied by π once more: 10π²/180 = 0.548311 rad (31.4°), not 10°.
        // Open wings are (0.8 + 0.4·0.5) = 1× as long as the head.
        let rnd = [0.5; 100];
        let (t, b) = (V2 { x: 0.0, y: 0.0 }, V2 { x: -100.0, y: 0.0 });
        let (w1, w2) = wings(&rnd, false, t, b, 4.0);
        assert_near(
            w1,
            V2 {
                x: -85.340_593_899_870_23,
                y: -52.124_687_364_217_65,
            },
        );
        assert_near(
            w2,
            V2 {
                x: -85.340_593_899_870_23,
                y: 52.124_687_364_217_65,
            },
        );
        // Closed, both wings turn 0.66 of the first one's angle and are exactly as long.
        let (w1, w2) = wings(&rnd, true, t, b, 4.0);
        assert_near(
            w1,
            V2 {
                x: -93.523_094_927_101_3,
                y: -35.403_823_455_333_15,
            },
        );
        assert_near(
            w2,
            V2 {
                x: -93.523_094_927_101_3,
                y: 35.403_823_455_333_15,
            },
        );
    }

    #[test]
    fn sketch_runs_from_tail_to_tip() {
        // Seed 5 draws an open head without wobble or gap. w is 6.8 px; the stroke starts
        // with a round cap 0.6·w = 4.1 px behind the tail and the head's point is at the tip.
        let mut a = resolved(ArrowStyle::Sketch, [20.0, 50.0], [280.0, 50.0]);
        a.seed = 5;
        let c = coverage(300, 100, 2.0, &a);
        assert_eq!((c[49][21], c[50][21]), (255, 255), "at the tail");
        assert_eq!((c[49][12], c[50][12]), (0, 0), "behind the tail");
        assert_eq!((c[49][150], c[50][150]), (255, 255), "the shaft");
        assert_eq!((c[49][278], c[50][278]), (255, 255), "at the tip");
        assert_eq!((c[49][290], c[50][290]), (0, 0), "past the tip");
    }

    #[test]
    fn closed_head_is_hollow() {
        // Seed 1 closes the head (a triangle 52 px long, wings ±20–24°) and its shaft stops at
        // the triangle's base; seed 5 leaves it open and its shaft runs to the tip. 39 px behind
        // the tip, on the axis, is inside the triangle.
        let mut a = resolved(ArrowStyle::Sketch, [20.0, 50.0], [280.0, 50.0]);
        for (seed, want) in [(1, 0), (5, 255)] {
            a.seed = seed;
            let c = coverage(300, 100, 2.0, &a);
            assert_eq!((c[49][241], c[50][241]), (want, want), "seed {seed}");
        }
    }

    #[test]
    fn bent_sketch_passes_through_via() {
        let mut a = resolved(ArrowStyle::Sketch, [20.0, 100.0], [220.0, 100.0]);
        a.via = Some([120.0, 40.0]);
        let c = coverage(240, 200, 2.0, &a);
        assert_eq!(c[40][120], 255, "at via");
        assert_eq!(c[100][120], 0, "on the chord");
    }
}
