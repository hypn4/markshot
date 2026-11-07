//! The highlighter: a filled marker stroke through the points, built by the shared stroke
//! engine and blended by the brightness of the screenshot under it.

use tiny_skia::{BlendMode, FillRule};

use crate::canvas::{Canvas, Ink};
use crate::color::Rgb;
use crate::geom::{Seg, V2};
use crate::resolve::ResolvedHighlight;
use crate::stroke::{
    Pt, assign_widths, bezier_path, ema_weight, flatten, h_snap, handles, metrics, norm_weight,
    outline, resample, smooth,
};

/// Stroke widths in points, by size.
pub const WIDTHS_PT: [f64; 8] = [14.0, 20.0, 28.0, 38.0, 54.0, 76.0, 108.0, 154.0];

/// The highlighter's smoothing and width variability.
const SMOOTHING: f64 = 0.9;
const VARIABILITY: f64 = 0.05;

/// Draws `h` on the main surface: the outline of the stroke through the points, filled with
/// the blend that the original image under it calls for.
///
/// # Panics
/// When `h.size` is 8 or more; resolved sizes are 0 to 7.
pub fn draw(c: &mut Canvas, h: &ResolvedHighlight) {
    let points: Vec<V2> = h.points.iter().map(|&p| c.up(p)).collect();
    let shape = rebuild(&points, h.size, h.cap, c.dpr);
    let (blend, alpha, saturate) = blend_for(luminance_under(c, &shape));
    let rgb = if saturate {
        h.color.saturate()
    } else {
        h.color
    };
    let ink = Ink { rgb, alpha, blend };
    let mut path = bezier_path(&shape, V2::default());
    path.0.push(Seg::Close);
    c.main().fill(&path, ink, FillRule::Winding);
}

/// The blend for a background of luminance `lum`: `(mode, alpha, saturate)`, where `saturate`
/// means the color is drawn fully saturated. Up to 0.33 the color is screened at 33%, up to
/// 0.66 laid over at 25%, and above that multiplied as it is. `None` (fewer than four samples)
/// multiplies too.
#[must_use]
pub fn blend_for(lum: Option<f64>) -> (BlendMode, f64, bool) {
    match lum {
        Some(l) if l <= 0.33 => (BlendMode::Screen, 0.33, true),
        Some(l) if l <= 0.66 => (BlendMode::SourceOver, 0.25, true),
        _ => (BlendMode::Multiply, 1.0, false),
    }
}

/// The outline polygon of the stroke through `points` (y-up px), at resolution `1/dpr`, with
/// shape correction "h".
///
/// The stroke arithmetic turns some inputs into NaN, which would blank the stroke:
/// - points all on one spot (`metrics` divides by the range of speeds): repeated points are
///   dropped, so these draw the single-point dot;
/// - a point whose neighbours coincide, i.e. an exact reversal (`handles` divides by their
///   distance): its handles are dropped, making it a sharp corner;
/// - a raw rect with no width or height (`map_rect` divides by it), as horizontal and vertical
///   strokes at whole pixels have: `map_rect` is left out, being the identity here anyway
///   since the drawn rect is the raw one.
fn rebuild(points: &[V2], size: u8, cap: u8, dpr: f64) -> Vec<Pt> {
    let res = 1.0 / dpr;
    let mut p: Vec<Pt> = points.iter().copied().map(Pt::new).collect();
    p.dedup_by_key(|q| q.pos);
    // A single point becomes a stroke 1 px to the right.
    if p.len() == 1 {
        let end = Pt::new(p[0].pos + V2 { x: 1.0, y: 0.0 });
        p.push(end);
    }
    metrics(&mut p);
    p = h_snap(&p, res);
    p = smooth(&p, SMOOTHING, res);
    p = smooth(&p, SMOOTHING * 0.8, res);
    p = smooth(&p, SMOOTHING * 0.6, res);
    p = resample(&p, 10.0 / res);
    handles(&mut p);
    let finite = |h: &V2| h.x.is_finite() && h.y.is_finite();
    for q in &mut p {
        q.a = q.a.filter(finite);
        q.b = q.b.filter(finite);
    }
    ema_weight(&mut p, 0.65);
    ema_weight(&mut p, 0.65);
    p = flatten(&p, 2.0 * dpr);
    ema_weight(&mut p, 0.65);
    ema_weight(&mut p, 0.65);
    norm_weight(&mut p, 0.0, 1.0, 0.3);
    p = smooth(&p, SMOOTHING * 0.6, res);
    p = smooth(&p, SMOOTHING * 0.2, res);
    let w = dpr * WIDTHS_PT[usize::from(size)];
    outline(&assign_widths(&p, w, VARIABILITY), cap, w)
}

/// The interquartile mean of the original image's luminance under the outline points, or
/// `None` for fewer than four samples. Points off the image are skipped. A stroke whose
/// bounding box has no width or height, as every horizontal one at whole pixels has, is
/// sampled like any other.
fn luminance_under(c: &Canvas, shape: &[Pt]) -> Option<f64> {
    let mut lum: Vec<f64> = shape
        .iter()
        .filter_map(|q| c.source_pixel(q.pos.x, q.pos.y))
        .map(|px| {
            let [r, g, b] = px.map(|v| f64::from(v) / 255.0);
            Rgb { r, g, b }.luminance()
        })
        .collect();
    if lum.len() < 4 {
        return None;
    }
    lum.sort_by(f64::total_cmp);
    let mid = &lum[lum.len() / 4..lum.len() * 3 / 4];
    #[expect(clippy::cast_precision_loss, reason = "a few hundred samples")]
    let n = mid.len() as f64;
    Some(mid.iter().sum::<f64>() / n)
}

#[cfg(test)]
mod tests {
    use std::ops::Range;

    use super::*;
    use crate::color::parse_hex;
    use crate::geom::PathData;
    use crate::image_io::{ColorSpace, SourceImage, decode_png};
    use crate::spec::Point;

    const WHITE: [u8; 3] = [255, 255, 255];
    /// `#FFF68D` multiplied onto white.
    const ON_WHITE: [u8; 3] = [255, 246, 141];

    /// A default highlight (`#FFF68D`, size 2, cap 3) through `points`.
    fn highlight(points: &[Point]) -> ResolvedHighlight {
        ResolvedHighlight {
            id: "highlight-1".to_owned(),
            color: parse_hex("#FFF68D").unwrap(),
            size: 2,
            points: points.to_vec(),
            cap: 3,
        }
    }

    /// A `width` × `height` canvas whose row `y` (top-down) is the grey `level(y)`.
    fn canvas(width: u32, height: u32, dpr: f64, level: impl Fn(u32) -> u8) -> Canvas {
        let rgba = (0..height)
            .flat_map(|y| {
                let v = level(y);
                (0..width).flat_map(move |_| [v, v, v, 255])
            })
            .collect();
        let src = SourceImage {
            width,
            height,
            rgba,
            dpi: None,
            icc: None,
            color_space: ColorSpace::Srgb,
        };
        Canvas::new(src, dpr)
    }

    /// A white canvas at dpr 2.
    fn white(width: u32, height: u32) -> Canvas {
        canvas(width, height, 2.0, |_| 255)
    }

    /// The canvas as RGB pixels, by row (top-down).
    fn pixels(c: Canvas) -> Vec<Vec<[u8; 3]>> {
        let width = c.width as usize;
        let png = decode_png(&c.into_png().unwrap()).unwrap();
        png.rgba
            .as_chunks::<4>()
            .0
            .chunks(width)
            .map(|row| row.iter().map(|&[r, g, b, _]| [r, g, b]).collect())
            .collect()
    }

    /// The pixels after drawing `h` on `c`.
    fn render(mut c: Canvas, h: &ResolvedHighlight) -> Vec<Vec<[u8; 3]>> {
        draw(&mut c, h);
        pixels(c)
    }

    fn column(rows: &[Vec<[u8; 3]>], x: usize) -> Vec<[u8; 3]> {
        rows.iter().map(|row| row[x]).collect()
    }

    /// `len` white pixels, `ON_WHITE` over `painted`.
    fn band(len: usize, painted: Range<usize>) -> Vec<[u8; 3]> {
        let mut v = vec![WHITE; len];
        v[painted].fill(ON_WHITE);
        v
    }

    #[test]
    fn blend_rules() {
        assert_eq!(blend_for(Some(0.9)), (BlendMode::Multiply, 1.0, false));
        assert_eq!(blend_for(Some(0.5)), (BlendMode::SourceOver, 0.25, true));
        assert_eq!(blend_for(Some(0.2)), (BlendMode::Screen, 0.33, true));
        // Fewer than four samples: multiplied, as on a light background.
        assert_eq!(blend_for(None), (BlendMode::Multiply, 1.0, false));
        // The dark band ends at 0.33 inclusive, the light one starts above 0.66.
        assert_eq!(blend_for(Some(0.33)), (BlendMode::Screen, 0.33, true));
        assert_eq!(blend_for(Some(0.66)), (BlendMode::SourceOver, 0.25, true));
        assert_eq!(blend_for(Some(0.661)), (BlendMode::Multiply, 1.0, false));
    }

    #[test]
    fn width_at_default_size() {
        // Size 2 is 28 pt: 56 px at dpr 2, rows 22–77 around y = 50. A two-point stroke is
        // equally wide everywhere, with straight sides between the caps.
        let rows = render(white(200, 100), &highlight(&[[40.0, 50.0], [160.0, 50.0]]));
        assert_eq!(column(&rows, 100), band(100, 22..78));
    }

    #[test]
    fn width_follows_size_and_dpr() {
        // Size 0 is 14 pt: 14 px at dpr 1, rows 43–56.
        let h = ResolvedHighlight {
            size: 0,
            ..highlight(&[[40.0, 50.0], [160.0, 50.0]])
        };
        let rows = render(canvas(200, 100, 1.0, |_| 255), &h);
        assert_eq!(column(&rows, 100), band(100, 43..57));
    }

    #[test]
    fn single_point_draws_dot() {
        // A single point becomes a 1 px stroke: a dot as tall as the stroke is wide, its caps
        // reaching about 20 px to each side.
        let rows = render(white(200, 200), &highlight(&[[100.0, 100.0]]));
        assert_eq!(rows[100][100], ON_WHITE);
        for (x, y) in [(100, 60), (100, 140), (55, 100), (146, 100)] {
            assert_eq!(rows[y][x], WHITE, "({x}, {y})");
        }
    }

    #[test]
    fn repeated_points_are_dropped() {
        // All points on one spot draw the single-point dot (their speeds would all be 0, which
        // makes NaN weights); a repeat elsewhere changes nothing.
        let on_white = |points: &[Point]| render(white(200, 200), &highlight(points));
        assert_eq!(on_white(&[[100.0, 100.0]; 3]), on_white(&[[100.0, 100.0]]));
        assert_eq!(
            on_white(&[[40.0, 50.0], [40.0, 50.0], [160.0, 50.0], [160.0, 50.0]]),
            on_white(&[[40.0, 50.0], [160.0, 50.0]])
        );
    }

    #[test]
    fn exact_reversal_is_drawn() {
        // Back and forth over one line: the turning points' neighbours coincide, so they have
        // no tangent to put Bézier handles on (NaN would blank the whole stroke).
        let rows = render(
            white(200, 100),
            &highlight(&[[40.0, 50.0], [160.0, 50.0], [40.0, 50.0], [160.0, 50.0]]),
        );
        for y in [30, 50, 70] {
            assert_eq!(rows[y][100], ON_WHITE, "(100, {y})");
        }
        assert_eq!([rows[10][100], rows[90][100]], [WHITE; 2]);
    }

    #[test]
    fn self_crossing_stroke_fills_its_crossing() {
        // The outline winds twice around the crossing at (100, 100): filled non-zero, it is
        // covered once, not left as a hole.
        let z = highlight(&[[40.0, 40.0], [160.0, 160.0], [160.0, 40.0], [40.0, 160.0]]);
        assert_eq!(render(white(200, 200), &z)[100][100], ON_WHITE);
    }

    #[test]
    fn vertical_stroke_is_drawn() {
        // Its bounding box has no width, which `map_rect` would divide by.
        let rows = render(white(100, 200), &highlight(&[[50.0, 40.0], [50.0, 160.0]]));
        assert_eq!(rows[100], band(100, 22..78));
    }

    #[test]
    fn blend_follows_the_background() {
        // The default color on white, #808080 and #1E1E1E: multiplied, laid over at 25% and
        // screened at 33% (the last two saturated).
        let mut c = canvas(200, 300, 2.0, |y| [255, 128, 30][y as usize / 100]);
        for y in [50.0, 150.0, 250.0] {
            draw(&mut c, &highlight(&[[40.0, y], [160.0, y]]));
        }
        let rows = pixels(c);
        assert_eq!(
            [rows[50][100], rows[150][100], rows[250][100]],
            [[255, 246, 141], [160, 155, 96], [104, 98, 30]]
        );
    }

    #[test]
    fn samples_the_original_image() {
        // Black drawn over the left half of the stroke does not count: the white image keeps
        // the stroke multiplied, so black stays black and white turns the color.
        let mut c = white(200, 100);
        let v = |x, y| V2 { x, y };
        let left_half = PathData(vec![
            Seg::Move(v(0.0, 0.0)),
            Seg::Line(v(100.0, 0.0)),
            Seg::Line(v(100.0, 100.0)),
            Seg::Line(v(0.0, 100.0)),
            Seg::Close,
        ]);
        let black = Ink {
            rgb: parse_hex("#000000").unwrap(),
            alpha: 1.0,
            blend: BlendMode::SourceOver,
        };
        c.main().fill(&left_half, black, FillRule::Winding);
        let rows = render(c, &highlight(&[[40.0, 50.0], [160.0, 50.0]]));
        assert_eq!([rows[50][60], rows[50][140]], [[0, 0, 0], ON_WHITE]);
    }

    #[test]
    fn ignores_samples_off_the_image() {
        // Along the top edge of a white image half the outline is off the image. Counted as
        // black, those points would make the background look mid-grey and the stroke 25% over.
        let rows = render(white(200, 100), &highlight(&[[40.0, 0.0], [160.0, 0.0]]));
        assert_eq!(rows[10][100], ON_WHITE);
    }

    #[test]
    fn luminance_is_the_interquartile_mean() {
        // A black row over a white one. Of eight samples three are black, so the middle half
        // of the sorted luminances is one black and three white: 0.75 (the mean is 0.625).
        let c = canvas(1, 2, 1.0, |y| [0, 255][y as usize]);
        let at = |y| Pt::new(V2 { x: 0.5, y });
        let (top, bottom) = (at(1.5), at(0.5));
        let lum = luminance_under(&c, &[[top; 3].as_slice(), &[bottom; 5]].concat());
        assert!(lum.is_some_and(|l| (l - 0.75).abs() < 1e-12), "{lum:?}");
        // Three samples on the image are too few.
        let off = at(2.5);
        assert_eq!(luminance_under(&c, &[top, bottom, bottom, off, off]), None);
    }
}
