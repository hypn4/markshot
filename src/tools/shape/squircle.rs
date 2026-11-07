//! The oval: a squircle fuller than an ellipse.

use crate::geom::{PathData, Seg, V2};

/// The squircle in the unit square: where it starts, then each cubic's two handles and end.
const START: (f64, f64) = (0.22, 0.06);
const CUBICS: [[(f64, f64); 3]; 8] = [
    [(0.38, -0.02), (0.62, -0.02), (0.78, 0.06)],
    [(0.8467, 0.0934), (0.9066, 0.1533), (0.94, 0.22)],
    [(1.02, 0.38), (1.02, 0.62), (0.94, 0.78)],
    [(0.9066, 0.8467), (0.8467, 0.9066), (0.78, 0.94)],
    [(0.62, 1.02), (0.38, 1.02), (0.22, 0.94)],
    [(0.1533, 0.9066), (0.0934, 0.8467), (0.06, 0.78)],
    [(-0.02, 0.62), (-0.02, 0.38), (0.06, 0.22)],
    [(0.0934, 0.1533), (0.1533, 0.0934), (0.22, 0.06)],
];

/// The squircle in the rect `(x, y, w, h)` (y-up px, corner `(x, y)`, sides positive) as an
/// open path of eight cubics. It touches the rect's mid-edges. Along a long side the
/// coefficients are pulled toward the ends by `0.75 * short/long + 0.25`, so the ends stay round
/// while the long sides stretch.
#[must_use]
pub fn squircle(x: f64, y: f64, w: f64, h: f64) -> PathData {
    let aspect = (w / h).abs();
    let (fx, fy) = ((h / w).abs() * 0.75 + 0.25, aspect * 0.75 + 0.25);
    let at = |(u, v): (f64, f64)| V2 {
        x: x + w * if aspect > 1.0 { pull(u, fx) } else { u },
        y: y + h * if aspect < 1.0 { pull(v, fy) } else { v },
    };
    let mut segs = vec![Seg::Move(at(START))];
    segs.extend(
        CUBICS
            .iter()
            .map(|&[c1, c2, end]| Seg::Cubic(at(c1), at(c2), at(end))),
    );
    PathData(segs)
}

/// Coefficient `c` pulled toward its nearer side by `f`.
fn pull(c: f64, f: f64) -> f64 {
    if c < 0.5 { f * c } else { 1.0 - (1.0 - c) * f }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_near(got: V2, want: (f64, f64)) {
        assert!(
            (got.x - want.0).abs() < 1e-9 && (got.y - want.1).abs() < 1e-9,
            "{got:?} != {want:?}"
        );
    }

    /// The path's points at `t = k/1000` along each cubic.
    fn samples(path: &PathData) -> Vec<V2> {
        let mut from = V2::default();
        let mut out = Vec::new();
        for seg in &path.0 {
            match *seg {
                Seg::Move(p) => from = p,
                Seg::Cubic(c1, c2, to) => {
                    for k in 0..=1000 {
                        let t = f64::from(k) / 1000.0;
                        let mt = 1.0 - t;
                        out.push(
                            from * (mt * mt * mt)
                                + c1 * (3.0 * mt * mt * t)
                                + c2 * (3.0 * mt * t * t)
                                + to * (t * t * t),
                        );
                    }
                    from = to;
                }
                Seg::Line(_) | Seg::Close => panic!("{seg:?}"),
            }
        }
        out
    }

    fn start(path: &PathData) -> V2 {
        match path.0[0] {
            Seg::Move(p) => p,
            ref other => panic!("{other:?}"),
        }
    }

    fn end(path: &PathData, i: usize) -> V2 {
        match path.0[i] {
            Seg::Cubic(_, _, p) => p,
            ref other => panic!("{other:?}"),
        }
    }

    #[test]
    fn squircle_touches_mid_edges() {
        let points = samples(&squircle(0.0, 0.0, 100.0, 50.0));
        for want in [(50.0, 0.0), (100.0, 25.0), (50.0, 50.0), (0.0, 25.0)] {
            assert!(
                points
                    .iter()
                    .any(|p| (p.x - want.0).abs() < 1e-9 && (p.y - want.1).abs() < 1e-9),
                "{want:?} is not on the path"
            );
        }
        // And stays inside the rect.
        for p in &points {
            assert!(
                (-1e-9..=100.0 + 1e-9).contains(&p.x) && (-1e-9..=50.0 + 1e-9).contains(&p.y),
                "{p:?}"
            );
        }
    }

    #[test]
    fn long_sides_stretch_and_the_ends_stay_round() {
        // Wide: x coefficients pulled toward the sides by 0.75 * (100/200) + 0.25 = 0.625:
        // 0.22 → 0.1375, 0.78 → 1 − 0.22 * 0.625 = 0.8625, 0.94 → 0.9625.
        let wide = squircle(0.0, 0.0, 200.0, 100.0);
        assert_near(start(&wide), (27.5, 6.0));
        assert_near(end(&wide, 1), (172.5, 6.0));
        assert_near(end(&wide, 2), (192.5, 22.0));
        // Tall: the y coefficients instead.
        let tall = squircle(0.0, 0.0, 100.0, 200.0);
        assert_near(start(&tall), (22.0, 7.5));
        assert_near(end(&tall, 2), (94.0, 27.5));
        // Square: the unit coefficients as they are.
        let square = squircle(10.0, 20.0, 100.0, 100.0);
        assert_near(start(&square), (32.0, 26.0));
        assert_near(end(&square, 2), (104.0, 42.0));
    }

    #[test]
    fn squircle_is_an_open_loop_of_eight_cubics() {
        let path = squircle(10.0, 20.0, 300.0, 100.0);
        assert_eq!(path.0.len(), 9);
        assert_eq!(end(&path, 8), start(&path));
    }
}
