//! The rounded rect under every rect, optionally jittered.

use crate::geom::{PathData, V2};
use crate::stroke::{Pt, bezier_path};

/// The rect `(x, y, w, h)` (y-up px, corner `(x, y)`, sides ≥ 0) with corners of
/// `R = min(min(w, h)/3, cp·dpr)` px, as an open path of eight cubics from the bottom edge's
/// left end, counter-clockwise. A `jitter` strength `s` above 0 moves each corner by up to
/// `s·h/800` sideways and `s·w/800` up or down (draws `rnd[56..64]`) and bows the edges out;
/// at 0 the edges are straight and each corner curve passes `R/8` inside the sharp corner.
#[must_use]
#[expect(
    clippy::too_many_arguments,
    reason = "the rect, the jitter, the corner size, dpr and the draws"
)]
pub fn rounded_rect(
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    jitter: f64,
    cp: f64,
    dpr: f64,
    rnd: &[f64; 100],
) -> PathData {
    bezier_path(&elements((x, y, w, h), jitter, cp, dpr, rnd), V2::default())
}

/// [`rounded_rect`]'s nine points, handles set: each segment's start, the start repeated at
/// the end (its outer handles unset).
pub(super) fn elements(
    (x, y, w, h): (f64, f64, f64, f64),
    jitter: f64,
    cp: f64,
    dpr: f64,
    rnd: &[f64; 100],
) -> [Pt; 9] {
    // A side of 0 counts as 1 px.
    let w = if w == 0.0 { 1.0 } else { w };
    let h = if h == 0.0 { 1.0 } else { h };
    // Intentional: a corner's x jitter scales with the height and its y jitter with the width.
    let corners: [V2; 4] = std::array::from_fn(|i| V2 {
        x: h * (jitter * (rnd[56 + 2 * i] - 0.5)) / 400.0,
        y: w * (jitter * (rnd[57 + 2 * i] - 0.5)) / 400.0,
    });
    let radius = (w.min(h) / 3.0).min(cp * dpr);
    let (rx, ry) = (radius / w, radius / h);
    // The edges' handles sit a quarter of the way along their straight runs.
    let (qx, qy) = (rx + (1.0 - (rx + rx)) * 0.25, ry + (1.0 - (ry + ry)) * 0.25);
    // How far the edges bow out, as fractions of the side across them.
    let bow = jitter * (w / h).min(h / w);
    let dx = bow * (dpr * 0.7 / w).min(h / dpr / 80_000.0);
    let dy = bow * (w / dpr / 80_000.0).min(dpr * 0.7 / h);
    // The corner handles move with the bow.
    let ey = ry * dx / (qy - ry).max(0.1);
    let ex = rx * dy / (qx - rx).max(0.1);
    // A point of the unit square in the rect, with its corner's jitter (none on the centre
    // lines).
    let at = |u: f64, v: f64| {
        let jit = match (u < 0.5, u > 0.5, v < 0.5, v > 0.5) {
            (true, _, true, _) => corners[0],
            (true, _, _, true) => corners[1],
            (_, true, _, true) => corners[2],
            (_, true, true, _) => corners[3],
            _ => V2::default(),
        };
        V2 {
            x: x + u * w,
            y: y + v * h,
        } + jit
    };
    let point = |pos, a, b| Pt {
        a,
        b,
        ..Pt::new(pos)
    };
    [
        point(at(rx, 0.0), None, Some(at(qx, -dy))),
        point(
            at(1.0 - rx, 0.0),
            Some(at(1.0 - qx, -dy)),
            Some(at(1.0 - ey, ex)),
        ),
        point(at(1.0, ry), Some(at(1.0 - ey, ex)), Some(at(1.0 + dx, qy))),
        point(
            at(1.0, 1.0 - ry),
            Some(at(1.0 + dx, 1.0 - qy)),
            Some(at(1.0 - ey, 1.0 - ex)),
        ),
        point(
            at(1.0 - rx, 1.0),
            Some(at(1.0 - ey, 1.0 - ex)),
            Some(at(1.0 - qx, 1.0 + dy)),
        ),
        point(at(rx, 1.0), Some(at(qx, 1.0 + dy)), Some(at(ey, 1.0 - ex))),
        point(
            at(0.0, 1.0 - ry),
            Some(at(ey, 1.0 - ex)),
            Some(at(-dx, 1.0 - qy)),
        ),
        point(at(0.0, ry), Some(at(-dx, qy)), Some(at(ey, ex))),
        point(at(rx, 0.0), Some(at(ey, ex)), None),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::Seg;
    use crate::tools::shape::WIDTHS;

    const NO_DRAWS: [f64; 100] = [0.5; 100];

    fn assert_near(got: V2, want: (f64, f64)) {
        assert!(
            (got.x - want.0).abs() < 1e-9 && (got.y - want.1).abs() < 1e-9,
            "{got:?} != {want:?}"
        );
    }

    /// The cubic `segs[i]` (after the move) as `[c1, c2, end]`.
    fn cubic(path: &PathData, i: usize) -> [V2; 3] {
        match path.0[i] {
            Seg::Cubic(c1, c2, end) => [c1, c2, end],
            ref other => panic!("segment {i} is {other:?}"),
        }
    }

    /// Where `path` starts.
    fn start_of(path: &PathData) -> V2 {
        match path.0[0] {
            Seg::Move(p) => p,
            ref other => panic!("{other:?}"),
        }
    }

    #[test]
    fn rect_corner() {
        // 200 × 100, size 2 (3 pt), dpr 2: corners of min(100/3, 2·3·2) = 12 px. Unjittered,
        // both handles of a corner curve sit on the sharp corner, so its midpoint is
        // 12/8 = 1.5 px in from both edges.
        let path = rounded_rect(0.0, 0.0, 200.0, 100.0, 0.0, 2.0 * WIDTHS[2], 2.0, &NO_DRAWS);
        assert_eq!(path.0[0], Seg::Move(V2 { x: 12.0, y: 0.0 }));
        assert_near(cubic(&path, 1)[2], (188.0, 0.0));
        let [c1, c2, end] = cubic(&path, 2);
        assert_near(c1, (200.0, 0.0));
        assert_near(c2, (200.0, 0.0));
        assert_near(end, (200.0, 12.0));
        let start = V2 { x: 188.0, y: 0.0 };
        let mid = (start + c1 * 3.0 + c2 * 3.0 + end) * 0.125;
        assert_near(mid, (198.5, 1.5));
        // A 24 px side caps the corners at a third of it: 8 px.
        let small = rounded_rect(0.0, 0.0, 30.0, 24.0, 0.0, 2.0 * WIDTHS[2], 2.0, &NO_DRAWS);
        assert_near(start_of(&small), (8.0, 0.0));
        assert_near(cubic(&small, 1)[2], (22.0, 0.0));
        assert_near(cubic(&small, 2)[2], (30.0, 8.0));
    }

    #[test]
    fn edge_handles_sit_a_quarter_along_the_straight_runs() {
        // R = 12: the bottom run is 12–188, handles at 12 + 176/4 = 56 and 144; the right run
        // is 12–88, handles at 31 and 69.
        let path = rounded_rect(0.0, 0.0, 200.0, 100.0, 0.0, 6.0, 2.0, &NO_DRAWS);
        let [c1, c2, _] = cubic(&path, 1);
        assert_near(c1, (56.0, 0.0));
        assert_near(c2, (144.0, 0.0));
        let [c1, c2, end] = cubic(&path, 3);
        assert_near(c1, (200.0, 31.0));
        assert_near(c2, (200.0, 69.0));
        assert_near(end, (200.0, 88.0));
    }

    #[test]
    fn rect_is_an_open_loop_of_eight_cubics() {
        let path = rounded_rect(10.0, 20.0, 200.0, 100.0, 1.3, 6.0, 2.0, &NO_DRAWS);
        // A move and eight cubics, the last ending on the start, without a close.
        assert_eq!(path.0.len(), 9);
        assert_eq!(cubic(&path, 8)[2], start_of(&path));
    }

    #[test]
    fn jitter_bows_the_edges_out() {
        // s' = 1.3·min(200/100, 100/200) = 0.65; dX = 0.65·min(1.4/200, 100/2/80000) and
        // dY = 0.65·min(200/2/80000, 1.4/100): both bow 0.08125 px. The corner handles then
        // move by ey = 0.12·dX/0.19 and ex = 0.06·dY/0.22.
        let path = rounded_rect(0.0, 0.0, 200.0, 100.0, 1.3, 6.0, 2.0, &NO_DRAWS);
        let [c1, c2, _] = cubic(&path, 1);
        assert_near(c1, (56.0, -0.08125));
        assert_near(c2, (144.0, -0.08125));
        let [c1, c2, _] = cubic(&path, 2);
        assert_near(c1, (199.948_684_210_526_32, 0.022_159_090_909_090_906));
        assert_near(c2, (199.948_684_210_526_32, 0.022_159_090_909_090_906));
        assert_near(cubic(&path, 3)[0], (200.081_25, 31.0));
    }

    #[test]
    fn jitter_moves_corners_by_the_other_side() {
        // Each corner's draws at an extreme: x moves by ±h·1.3·0.5/400 = ±0.1625 and y by
        // ±w·1.3·0.5/400 = ±0.325. Draws 56–57 move the bottom-left corner (y up), 58–59 the
        // top-left, 60–61 the top-right and 62–63 the bottom-right.
        let mut rnd = NO_DRAWS;
        rnd[56..64].copy_from_slice(&[1.0, 0.0, 0.0, 1.0, 1.0, 1.0, 0.0, 0.0]);
        let path = rounded_rect(0.0, 0.0, 200.0, 100.0, 1.3, 6.0, 2.0, &rnd);
        assert_near(start_of(&path), (12.1625, -0.325));
        let ends = [
            (187.8375, -0.325),
            (199.8375, 11.675),
            (200.1625, 88.325),
            (188.1625, 100.325),
            (11.8375, 100.325),
            (-0.1625, 88.325),
            (0.1625, 11.675),
            (12.1625, -0.325),
        ];
        for (i, want) in ends.into_iter().enumerate() {
            assert_near(cubic(&path, i + 1)[2], want);
        }
    }

    #[test]
    fn a_zero_side_counts_as_one_pixel() {
        let path = rounded_rect(5.0, 5.0, 0.0, 50.0, 0.0, 6.0, 2.0, &NO_DRAWS);
        let points = path.0.iter().flat_map(|seg| match *seg {
            Seg::Move(p) | Seg::Line(p) => vec![p],
            Seg::Cubic(c1, c2, p) => vec![c1, c2, p],
            Seg::Close => vec![],
        });
        for p in points {
            assert!(
                (5.0..=6.0).contains(&p.x) && (5.0..=55.0).contains(&p.y),
                "{p:?}"
            );
        }
    }
}
