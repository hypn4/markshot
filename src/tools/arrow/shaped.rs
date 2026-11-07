//! The `shaped` arrow: one of four SVG outlines, its tail pinned to the tail and its head to the
//! tip; bent, a tapered band along the arc under the outline's head.

use crate::geom::{PathData, Seg, V2};
use crate::math::{PI, atan2, cos, sin};
use crate::stroke::{bezier_path, norm, outline};

use super::curve::{Arc, wrap};
use super::profiles::{ARR_PROFILES, OUTLINES, SIZEK};
use super::{Geometry, taper};

/// Outline points with x up to this belong to the tail, the rest to the head.
const SPLIT: f64 = 50.0;

/// The arrow's filled paths: one when straight; the band and the head when bent.
pub(super) fn paths(g: &Geometry, variant: usize) -> Vec<PathData> {
    if g.mid_angle == 0.0 {
        vec![straight(g, variant)]
    } else {
        Vec::from(curved(g, variant))
    }
}

/// The outline with x ≤ 50 placed at the tail (scale `kTail`), the rest at the tip (scale
/// `kHead`), both turned along the arrow; only the shaft between them stretches.
fn straight(g: &Geometry, variant: usize) -> PathData {
    let d = g.p1 - g.p0;
    let theta = atan2(d.y, d.x);
    let sizek = SIZEK[g.size];
    OUTLINES[variant].map(|q| {
        if q.x <= SPLIT {
            place(g.p0, q, theta, sizek, g.k_tail, g.dpr)
        } else {
            place(
                g.p1,
                V2 {
                    x: q.x - 100.0,
                    ..q
                },
                theta,
                sizek,
                g.k_head,
                g.dpr,
            )
        }
    })
}

/// A band along the arc, tapering linearly from the tail's width to the neck's and ending `h`
/// before the tip, and the outline's head (cut flat across the neck) set on the tangent at the
/// band's end, so its point lands near, not on, the tip. Returns both, to union.
fn curved(g: &Geometry, variant: usize) -> [PathData; 2] {
    let (w1, w2, h) = ARR_PROFILES[variant];
    let arc = Arc::new(g.p0, g.p1, g.mid_angle, g.mid_dist);
    let (center, radius) = (arc.center, arc.radius);
    // +1: the band runs clockwise (angles fall) from the tail.
    let sgn = if wrap(g.mid_angle, PI) > 0.0 {
        1.0
    } else {
        -1.0
    };
    // px per half SVG unit.
    let unit = SIZEK[g.size] * g.dpr * 0.5;
    let head_len = g.k_head * h * unit;
    // Turning `sgn·2π` per circumference: the arc angle `head_len` spans.
    let turn = (sgn + sgn) * PI * head_len / (radius * (2.0 * PI));
    let head_base = arc.end + turn;
    // The direction of travel at the head base.
    let phi = arc.end - sgn * PI * 0.5 + turn;
    // The band starts a tenth of the tail's width into the arc.
    let start =
        arc.start + w1 * (g.k_tail * ((sgn + sgn) * PI)) * unit * -0.1 / (radius * (2.0 * PI));
    let mut band = arc.points(start, head_base, sgn);
    let (w_start, w_end) = (g.k_tail * w1 * unit, g.k_head * w2 * unit);
    taper(&mut band, w_start, w_end, 0);
    let mut body = bezier_path(&outline(&band, 5, w_start.midpoint(w_end)), V2::default());
    body.0.push(Seg::Close);
    let tip = g.p0
        + V2 {
            x: head_len * cos(phi) + center.x + radius * cos(head_base) - g.p0.x,
            y: head_len * sin(phi) + center.y + radius * sin(head_base) - g.p0.y,
        };
    [body, head(g, variant, tip, phi)]
}

/// The outline's head, its point on `tip` and turned to `phi`: only the commands whose points
/// all belong to the head (and the first move), then closed.
fn head(g: &Geometry, variant: usize, tip: V2, phi: f64) -> PathData {
    let sizek = SIZEK[g.size];
    // The whole head scales by kHead, and a tail point would be pinned to the arrow's tail.
    let at = |q: V2| {
        if q.x <= SPLIT {
            place(g.p0, q, phi, sizek, g.k_head, g.dpr)
        } else {
            place(
                tip,
                V2 {
                    x: q.x - 100.0,
                    ..q
                },
                phi,
                sizek,
                g.k_head,
                g.dpr,
            )
        }
    };
    let on_head = |q: V2| q.x > SPLIT;
    let mut segs: Vec<Seg> = OUTLINES[variant]
        .0
        .iter()
        .filter_map(|seg| match *seg {
            Seg::Move(p) => Some(Seg::Move(at(p))),
            Seg::Line(p) if on_head(p) => Some(Seg::Line(at(p))),
            Seg::Cubic(c1, c2, p) if on_head(c1) && on_head(c2) && on_head(p) => {
                Some(Seg::Cubic(at(c1), at(c2), at(p)))
            }
            _ => None,
        })
        .collect();
    segs.push(Seg::Close);
    PathData(segs)
}

/// Outline point `q` (SVG units from `anchor`) scaled by `sizek·k·dpr` and turned by `theta`,
/// through polar coordinates rather than a rotation, which rounds differently.
fn place(anchor: V2, q: V2, theta: f64, sizek: f64, k: f64, dpr: f64) -> V2 {
    let r = sizek * (norm(q) * k) * dpr;
    let a = theta + atan2(q.y, q.x);
    V2 {
        x: anchor.x + cos(a) * r,
        y: anchor.y + sin(a) * r,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::{Seg, V2};
    use crate::spec::ArrowStyle;
    use crate::tools::arrow::{coverage, resolved};

    fn v(x: f64, y: f64) -> V2 {
        V2 { x, y }
    }

    /// The end point of segment `i`.
    fn end(p: &PathData, i: usize) -> V2 {
        match p.0[i] {
            Seg::Move(e) | Seg::Line(e) | Seg::Cubic(_, _, e) => e,
            Seg::Close => panic!("segment {i} is a close"),
        }
    }

    fn assert_near(got: V2, want: V2) {
        assert_within(got, want, 1e-9);
    }

    fn assert_within(got: V2, want: V2, tol: f64) {
        assert!(
            (got.x - want.x).abs() < tol && (got.y - want.y).abs() < tol,
            "{got:?} != {want:?}"
        );
    }

    #[test]
    fn straight_outline_is_pinned_to_tail_and_tip() {
        // 400 px at size 2, dpr 2: 1 SVG unit is 0.7·2 = 1.4 px at both ends. Segment 0 (the
        // head's edge, x 53.77), 1 (next to the tip, x 99.32) and 9 (the neck, x 50.79) hang off
        // the tip; segment 10 (x 1.58) off the tail.
        let g = Geometry::new(v(100.0, 100.0), v(500.0, 100.0), 2, None, 2.0);
        let p = straight(&g, 0);
        assert_near(end(&p, 0), v(435.271_402_5, 69.079_310_9));
        assert_near(end(&p, 1), v(499.042_483_86, 98.561_656_142));
        assert_near(end(&p, 9), v(431.103_635_4, 112.512_143_966));
        assert_near(end(&p, 10), v(102.210_937_484, 109.902_833_514));
        // Pointing up, the same point is turned a quarter turn about the tip.
        let up = Geometry::new(v(100.0, 100.0), v(100.0, 500.0), 2, None, 2.0);
        assert_near(end(&straight(&up, 0), 0), v(130.920_689_1, 435.271_402_5));
    }

    #[test]
    fn head_and_tail_scale_separately() {
        // 720 px: the head grows 1.25× (1.75 px a unit), the tail shrinks 0.8× (1.12).
        let long = Geometry::new(v(0.0, 0.0), v(720.0, 0.0), 2, None, 2.0);
        let p = straight(&long, 0);
        assert_near(end(&p, 0), v(639.089_253_125, -38.650_861_375));
        assert_near(end(&p, 10), v(1.768_749_987_2, 7.922_266_811_2));
        // 100 px is half of size 2's 200 px threshold: both ends at half scale, 0.7 px a unit.
        let short = Geometry::new(v(0.0, 0.0), v(100.0, 0.0), 2, None, 2.0);
        let p = straight(&short, 0);
        assert_near(end(&p, 0), v(67.635_701_25, -15.460_344_55));
        assert_near(end(&p, 10), v(1.105_468_742, 4.951_416_757));
    }

    #[test]
    fn bent_arrow_follows_the_circle_through_via() {
        // The circle through (20, 100), (120, 40) and (220, 100) (y down) has its centre at
        // (120, 153.3) and radius 113.3. The band is about 22 px wide at `via`.
        let mut a = resolved(ArrowStyle::Shaped, [20.0, 100.0], [220.0, 100.0]);
        for (via, on_arc, chord, far_side) in [
            ([120.0, 40.0], (120, 40), (120, 70), (6, 153)),
            ([120.0, 160.0], (120, 160), (120, 130), (6, 47)),
        ] {
            a.via = Some(via);
            let c = coverage(240, 200, 2.0, &a);
            let at = |(x, y): (usize, usize)| c[y][x];
            assert_eq!(at(on_arc), 255, "via {via:?}");
            assert_eq!(at(chord), 0, "inside the bend, via {via:?}");
            // The rest of the circle stays empty: the band runs the short way round.
            assert_eq!(at(far_side), 0, "far side, via {via:?}");
        }
    }

    #[test]
    fn bent_head_sits_on_the_tangent_past_the_head_base() {
        // Bent through (120, 160) (y up), the circle has centre (120, 46.67) and radius 113.33;
        // the tip is at angle 0.48996. The head is 68.6 px long (98 half units at 0.7 px), so
        // its base is 68.6 px of arc before the tip, at 1.09525, and its point T lies 68.6 px
        // on along the tangent there (−0.47554): (232.875, 116.018), past the tip.
        let g = Geometry::new(
            v(20.0, 100.0),
            v(220.0, 100.0),
            2,
            Some(v(120.0, 160.0)),
            2.0,
        );
        let paths = paths(&g, 0);
        let [_, head] = &paths[..] else {
            panic!("{} paths", paths.len());
        };
        // Only the outline's head is drawn (16 of its 20 segments), cut straight across the
        // neck from (50.79, 8.94) to (50.79, −8.94), then closed.
        assert_eq!(head.0.len(), 17);
        assert_within(end(head, 0), v(161.172_243_86, 118.162_666_71), 1e-6);
        assert_within(end(head, 9), v(177.351_465_37, 158.684_439_40), 1e-6);
        assert_within(end(head, 10), v(165.894_772_31, 136.436_761_53), 1e-6);
        assert_eq!(head.0[16], Seg::Close);
    }
}
