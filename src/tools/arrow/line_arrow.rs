//! The `line` and `double` arrows (Arrow.md §3.4): a shaft with an open chevron at the tip (and
//! at the tail), stroked with square caps and miter joins. The shaft is pulled in half the width
//! at both ends, so its caps end flush with the arrow's ends.

use crate::geom::{PathData, Seg, V2};
use crate::math::{PI, PI_TRUNCATED, atan2, cos, sin, tan};
use crate::tools::line::{half_angle, head_len, pixel_offset};

use super::Geometry;
use super::curve::{Arc, wrap};

/// A quarter turn as this routine has it: neither π/2 nor the reference app's truncated π halved.
const QUARTER_TURN: f64 = 1.570_796_45;

/// The path to stroke `lw` px wide; `double` adds the chevron at the tail.
pub(super) fn path(g: &Geometry, lw: f64, double: bool) -> PathData {
    let d = g.p1 - g.p0;
    let theta = atan2(d.y, d.x);
    let mut path = PathData::default();
    // `off` aligns straight shafts to pixels; `psi` tilts a bent arrow's chevrons toward the
    // arc's tangent.
    let (off, psi) = if g.mid_angle.abs() < 0.01 {
        let off = pixel_offset(lw, g.dpr);
        let half = V2 {
            x: lw * (cos(theta) * 0.5),
            y: lw * (sin(theta) * 0.5),
        };
        path.0.extend([
            Seg::Move(V2 {
                x: g.p0.x + off + half.x,
                y: g.p0.y + off + half.y,
            }),
            Seg::Line(V2 {
                x: g.p1.x + off - half.x,
                y: g.p1.y + off - half.y,
            }),
        ]);
        (off, 0.0)
    } else {
        let a = wrap(g.mid_angle, PI_TRUNCATED);
        let arc = Arc::new(g.p0, g.p1, g.mid_angle, g.mid_dist);
        // Half the width as an arc angle, so the shaft's caps end flush too.
        let turn = if a > 0.0 {
            2.0 * PI_TRUNCATED
        } else {
            -2.0 * PI_TRUNCATED
        };
        let delta = lw * turn * 0.5 / (arc.radius * (2.0 * PI_TRUNCATED));
        add_arc(&mut path, &arc, arc.start - delta, arc.end + delta, a > 0.0);
        let mut v = theta - arc.end - QUARTER_TURN;
        if a > 0.0 {
            v += PI_TRUNCATED;
        }
        let v = wrap(v, PI_TRUNCATED);
        (0.0, v * ((PI_TRUNCATED - v.abs()) * -0.15 / PI_TRUNCATED + 1.0))
    };
    let start = V2 {
        x: g.p0.x + off,
        y: g.p0.y + off,
    };
    let end = V2 {
        x: start.x + d.x,
        y: d.y + start.y,
    };
    let (alpha, wing) = (half_angle(g.len), head_len(g.len, lw, g.dpr));
    if double {
        let back = atan2(start.y - (d.y + start.y), start.x - (start.x + d.x));
        chevron(&mut path, start, back + psi, alpha, wing);
    }
    chevron(
        &mut path,
        end,
        atan2(end.y - start.y, end.x - start.x) - psi,
        alpha,
        wing,
    );
    path
}

/// An open chevron pointing along `beta` with its point on `tip`: two wings `wing` px long, at
/// `alpha` either side of the way back.
fn chevron(path: &mut PathData, tip: V2, beta: f64, alpha: f64, wing: f64) {
    let t1 = PI_TRUNCATED - alpha + beta;
    path.0.extend([
        Seg::Move(tip + V2::dir(t1) * wing),
        Seg::Line(tip),
        Seg::Line(tip + V2::dir(alpha + alpha + t1) * wing),
    ]);
}

/// CoreGraphics' `addArc(center:radius:startAngle:endAngle:clockwise:)` on an empty path, in
/// y-up space (clockwise: the angle falls), as cubics of at most a quarter turn each.
fn add_arc(path: &mut PathData, arc: &Arc, from: f64, to: f64, clockwise: bool) {
    let (c, r) = (arc.center, arc.radius);
    let sweep = (if clockwise { from - to } else { to - from }).rem_euclid(2.0 * PI);
    let n = (sweep / (0.5 * PI)).ceil().clamp(1.0, 4.0);
    let step = if clockwise { -sweep / n } else { sweep / n };
    // Controls `k·r` along the tangents: the usual cubic fit of a circular arc.
    let k = 4.0 / 3.0 * tan(step / 4.0);
    let at = |a: f64| c + V2::dir(a) * r;
    let tangent = |a: f64| V2::dir(a + 0.5 * PI) * (k * r);
    path.0.push(Seg::Move(at(from)));
    for i in (1..=4).map(f64::from).take_while(|&i| i <= n) {
        let (a0, a1) = (from + step * (i - 1.0), from + step * i);
        path.0.push(Seg::Cubic(
            at(a0) + tangent(a0),
            at(a1) - tangent(a1),
            at(a1),
        ));
    }
}

#[cfg(test)]
mod tests {
    use crate::spec::ArrowStyle;
    use crate::tools::arrow::{coverage, resolved};

    #[test]
    fn shaft_ends_flush_with_square_caps() {
        // 80 px at size 2, dpr 2: 6 px wide, so rows 47–52 from x 20 to 100 (y down). The
        // chevron's wings are 20 px (a quarter of the length) at 50°: their midpoints are
        // (93.57, 42.34) and (93.57, 57.66); its miter point reaches 3.9 px past the tip.
        let c = coverage(
            120,
            100,
            2.0,
            &resolved(ArrowStyle::Line, [20.0, 50.0], [100.0, 50.0]),
        );
        let column = |x: usize| c.iter().map(|row| row[x]).collect::<Vec<_>>();
        let mut want = vec![0; 100];
        want[47..53].fill(255);
        assert_eq!(column(21), want);
        assert_eq!(column(60), want);
        assert_eq!(column(19), [0; 100], "behind the tail");
        assert_eq!((c[42][93], c[57][93]), (255, 255), "wings");
        assert_eq!((c[50][106], c[42][106]), (0, 0), "past the tip");
    }

    #[test]
    fn double_adds_a_chevron_at_the_tail() {
        // The tail's chevron mirrors the tip's: wings through (26.43, 42.34) and (26.43, 57.66),
        // its miter point 3.9 px behind x 20.
        let single = coverage(
            120,
            100,
            2.0,
            &resolved(ArrowStyle::Line, [20.0, 50.0], [100.0, 50.0]),
        );
        let double = coverage(
            120,
            100,
            2.0,
            &resolved(ArrowStyle::Double, [20.0, 50.0], [100.0, 50.0]),
        );
        for (x, y) in [(26, 42), (26, 57), (19, 49), (19, 50)] {
            assert_eq!((single[y][x], double[y][x]), (0, 255), "({x}, {y})");
        }
        // The tip's chevron is the same.
        assert_eq!((double[42][93], double[57][93]), (255, 255));
    }

    #[test]
    fn dpr_one_aligns_odd_widths_to_pixels() {
        // Size 4 over 50 px at dpr 1 is 5 px wide (rounded): the half-pixel offset puts it on
        // rows 7–11 exactly instead of half-covering rows 7 and 12.
        let mut a = resolved(ArrowStyle::Line, [10.0, 10.0], [60.0, 10.0]);
        a.size = 4;
        let c = coverage(80, 20, 1.0, &a);
        let column: Vec<u8> = c.iter().map(|row| row[30]).collect();
        let mut want = vec![0; 20];
        want[7..12].fill(255);
        assert_eq!(column, want);
    }

    #[test]
    fn bent_line_passes_through_via() {
        for style in [ArrowStyle::Line, ArrowStyle::Double] {
            let mut a = resolved(style, [20.0, 100.0], [220.0, 100.0]);
            a.via = Some([120.0, 40.0]);
            let c = coverage(240, 200, 2.0, &a);
            assert_eq!(c[40][120], 255, "{style:?} at via");
            assert_eq!(c[100][120], 0, "{style:?} on the chord");
        }
    }
}
