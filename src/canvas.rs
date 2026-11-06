//! The drawing target: the source image plus transparent layers, all addressed in y-up
//! image pixels. The y flip happens here and nowhere else.

use tiny_skia::{
    BlendMode, Color, ColorU8, FillRule, IntSize, LineCap, LineJoin, Paint, Path, PathBuilder,
    PathStroker, Pixmap, PixmapPaint, Stroke, StrokeDash, Transform,
};

use crate::color::Rgb;
use crate::error::Error;
use crate::geom::{PathData, Seg, V2};
use crate::image_io::{ColorSpace, SourceImage, encode_png};

/// CoreGraphics' default miter limit (tiny-skia's own default is 4).
const MITER_LIMIT: f32 = 10.0;

/// A paint: sRGB color (converted to the canvas color space when drawn), an alpha that
/// multiplies coverage, and the blend mode.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Ink {
    pub rgb: Rgb,
    /// 0–1.
    pub alpha: f64,
    pub blend: BlendMode,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StrokeStyle {
    /// In image pixels; zero or less draws nothing.
    pub width: f64,
    pub cap: LineCap,
    pub join: LineJoin,
    /// Alternating on/off lengths starting at the path start; an invalid array (odd length,
    /// negative entries, zero total) strokes solid.
    pub dash: Option<Vec<f64>>,
}

/// A premultiplied RGBA layer the size of the canvas, drawn in y-up image pixels.
pub struct Surface {
    pixmap: Pixmap,
    /// Maps y-up image pixels to the pixmap's top-down rows.
    transform: Transform,
    /// Whether ink colors are converted from sRGB to Display P3.
    display_p3: bool,
}

impl Surface {
    /// Fills `p`; an empty or degenerate path draws nothing.
    pub fn fill(&mut self, p: &PathData, ink: Ink, rule: FillRule) {
        let Some(path) = to_skia_path(p) else {
            return;
        };
        let paint = self.paint(ink);
        self.pixmap
            .fill_path(&path, &paint, rule, self.transform, None);
    }

    /// Strokes `p`; an empty or degenerate path, or a width that is not positive, draws nothing.
    ///
    /// The outline is always computed and filled. tiny-skia's own `stroke_path` draws strokes up
    /// to 1 px wide with its hairline rasterizer instead, whose coverage differs from
    /// CoreGraphics' and which draws nothing on pixmaps 2 px tall.
    pub fn stroke(&mut self, p: &PathData, ink: Ink, s: &StrokeStyle) {
        if s.width.is_nan() || s.width <= 0.0 {
            return;
        }
        let Some(mut path) = to_skia_path(p) else {
            return;
        };
        let resolution = PathStroker::compute_resolution_scale(&self.transform);
        let dash = s
            .dash
            .as_ref()
            .and_then(|d| StrokeDash::new(d.iter().copied().map(to_f32).collect(), 0.0));
        if let Some(dash) = dash {
            let Some(dashed) = path.dash(&dash, resolution) else {
                return;
            };
            path = dashed;
        }
        let stroke = Stroke {
            width: to_f32(s.width),
            miter_limit: MITER_LIMIT,
            line_cap: s.cap,
            line_join: s.join,
            dash: None,
        };
        let Some(outline) = path.stroke(&stroke, resolution) else {
            return;
        };
        let paint = self.paint(ink);
        self.pixmap
            .fill_path(&outline, &paint, FillRule::Winding, self.transform, None);
    }

    fn paint(&self, ink: Ink) -> Paint<'static> {
        let rgb = if self.display_p3 {
            ink.rgb.srgb_to_display_p3()
        } else {
            ink.rgb
        };
        let unit = |v: f64| to_f32(if v.is_nan() { 0.0 } else { v.clamp(0.0, 1.0) });
        let color = Color::from_rgba(unit(rgb.r), unit(rgb.g), unit(rgb.b), unit(ink.alpha))
            .expect("components are clamped to 0–1");
        let mut paint = Paint {
            blend_mode: ink.blend,
            ..Paint::default()
        };
        paint.set_color(color);
        paint
    }

    /// Straight-alpha RGBA8, rows top-down.
    fn to_rgba(&self) -> Vec<u8> {
        self.pixmap
            .pixels()
            .iter()
            .flat_map(|p| {
                let c = p.demultiply();
                [c.red(), c.green(), c.blue(), c.alpha()]
            })
            .collect()
    }
}

/// The source image (kept for [`Canvas::sample`]) and the main surface everything ends up on.
pub struct Canvas {
    pub width: u32,
    pub height: u32,
    pub dpr: f64,
    src: SourceImage,
    main: Surface,
}

impl Canvas {
    /// # Panics
    /// When `src` is empty or `src.rgba` is not `width × height × 4` bytes.
    #[must_use]
    pub fn new(src: SourceImage, dpr: f64) -> Canvas {
        let size = IntSize::from_wh(src.width, src.height).expect("the image is not empty");
        let premultiplied = src
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|&[r, g, b, a]| {
                let p = ColorU8::from_rgba(r, g, b, a).premultiply();
                [p.red(), p.green(), p.blue(), p.alpha()]
            })
            .collect();
        let pixmap =
            Pixmap::from_vec(premultiplied, size).expect("the RGBA buffer matches the image size");
        let main = Surface {
            pixmap,
            transform: Transform::from_row(1.0, 0.0, 0.0, -1.0, 0.0, to_f32(f64::from(src.height))),
            display_p3: src.color_space == ColorSpace::DisplayP3,
        };
        Canvas {
            width: src.width,
            height: src.height,
            dpr,
            src,
            main,
        }
    }

    pub fn main(&mut self) -> &mut Surface {
        &mut self.main
    }

    /// A transparent surface the size of the canvas, for [`Canvas::composite`].
    ///
    /// # Panics
    /// Never: the canvas size was validated in [`Canvas::new`].
    #[must_use]
    pub fn new_layer(&self) -> Surface {
        Surface {
            pixmap: Pixmap::new(self.width, self.height).expect("the canvas is not empty"),
            transform: self.main.transform,
            display_p3: self.main.display_p3,
        }
    }

    /// Draws `layer` over the main surface (source-over, opacity 1).
    pub fn composite(&mut self, layer: &Surface) {
        self.main.pixmap.draw_pixmap(
            0,
            0,
            layer.pixmap.as_ref(),
            &PixmapPaint::default(),
            Transform::identity(),
            None,
        );
    }

    /// A y-down image point in y-up space.
    #[must_use]
    pub fn up(&self, p: [f64; 2]) -> V2 {
        V2 {
            x: p[0],
            y: f64::from(self.height) - p[1],
        }
    }

    /// A y-down `[x, y, w, h]` rect (`w`, `h` may be negative) as a y-up `(x, y, w, h)` with
    /// `(x, y)` the bottom-left corner and `w`, `h` non-negative.
    #[must_use]
    pub fn up_rect(&self, r: [f64; 4]) -> (f64, f64, f64, f64) {
        let [x, y_down, dx, dy] = r;
        let left = x.min(x + dx);
        let top = y_down.min(y_down + dy);
        let (width, height) = (dx.abs(), dy.abs());
        (left, f64::from(self.height) - top - height, width, height)
    }

    /// the reference app's `getPixel` (Blur.md §3.1): [`Canvas::source_pixel`], black outside the image.
    /// The bytes are in the source colour space; the reference app converts them to sRGB, this keeps them
    /// (spec §5.9).
    #[must_use]
    pub fn sample(&self, x: f64, y: f64) -> [u8; 3] {
        self.source_pixel(x, y).unwrap_or([0, 0, 0])
    }

    /// The original pixel at column `floor(x)`, row `H − ceil(y)` (rows top-down), or `None`
    /// outside the image.
    #[must_use]
    pub fn source_pixel(&self, x: f64, y: f64) -> Option<[u8; 3]> {
        let col = x.floor();
        let row = f64::from(self.height) - y.ceil();
        if !(0.0..f64::from(self.width)).contains(&col)
            || !(0.0..f64::from(self.height)).contains(&row)
        {
            return None;
        }
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "col and row are whole numbers inside the image"
        )]
        let i = (row as usize * self.width as usize + col as usize) * 4;
        Some([self.src.rgba[i], self.src.rgba[i + 1], self.src.rgba[i + 2]])
    }

    /// Overwrites the main surface with a `w × h` straight-alpha RGBA bitmap (rows top-down)
    /// whose bottom-left corner is at y-up `(x, y_up)`, clipped to the canvas.
    ///
    /// # Panics
    /// When `rgba` is not `w × h × 4` bytes.
    pub fn put_rgba(&mut self, x: i32, y_up: i32, w: u32, h: u32, rgba: &[u8]) {
        // Bitmap row r lands on canvas row `top + r`, column c on `x + c` (both may be negative).
        let top = i64::from(self.height) - i64::from(y_up) - i64::from(h);
        let (w, h) = (w as usize, h as usize);
        assert_eq!(rgba.len(), w * h * 4, "bitmap size mismatch");
        if w == 0 {
            return;
        }
        let (width, height) = (self.width as usize, self.height as usize);
        let pixels = self.main.pixmap.pixels_mut();
        for (r, row) in rgba.chunks_exact(w * 4).enumerate() {
            let Some(dst_row) = offset(top, r).filter(|&v| v < height) else {
                continue;
            };
            for (c, &[red, green, blue, alpha]) in row.as_chunks::<4>().0.iter().enumerate() {
                if let Some(dst_col) = offset(i64::from(x), c).filter(|&v| v < width) {
                    pixels[dst_row * width + dst_col] =
                        ColorU8::from_rgba(red, green, blue, alpha).premultiply();
                }
            }
        }
    }

    /// Encodes the main surface, keeping the source's DPI and ICC profile.
    ///
    /// # Errors
    /// [`Error::Png`] when encoding fails.
    pub fn into_png(self) -> Result<Vec<u8>, Error> {
        encode_png(
            self.width,
            self.height,
            &self.main.to_rgba(),
            self.src.dpi,
            self.src.icc.as_deref(),
        )
    }
}

/// `start + i` as an index, `None` when negative.
fn offset(start: i64, i: usize) -> Option<usize> {
    usize::try_from(start + i64::try_from(i).ok()?).ok()
}

/// `None` for an empty or degenerate path.
fn to_skia_path(p: &PathData) -> Option<Path> {
    let mut pb = PathBuilder::new();
    let pt = |v: V2| (to_f32(v.x), to_f32(v.y));
    for seg in &p.0 {
        match *seg {
            Seg::Move(a) => {
                let (x, y) = pt(a);
                pb.move_to(x, y);
            }
            Seg::Line(a) => {
                let (x, y) = pt(a);
                pb.line_to(x, y);
            }
            Seg::Cubic(c1, c2, a) => {
                let ((x1, y1), (x2, y2), (x, y)) = (pt(c1), pt(c2), pt(a));
                pb.cubic_to(x1, y1, x2, y2, x, y);
            }
            Seg::Close => pb.close(),
        }
    }
    pb.finish()
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "geometry is f64; tiny-skia takes f32"
)]
fn to_f32(v: f64) -> f32 {
    v as f32
}

#[cfg(test)]
mod tests {
    use tiny_skia::{BlendMode, FillRule, LineCap, LineJoin};

    use super::*;
    use crate::color::parse_hex;
    use crate::geom::Seg;
    use crate::image_io::decode_png;

    const RED: [u8; 4] = [255, 0, 0, 255];
    const CLEAR: [u8; 4] = [0, 0, 0, 0];

    fn image(width: u32, height: u32, pixels: &[[u8; 4]], color_space: ColorSpace) -> SourceImage {
        SourceImage {
            width,
            height,
            rgba: pixels.concat(),
            dpi: Some(144.0),
            icc: None,
            color_space,
        }
    }

    fn blank(width: u32, height: u32) -> Canvas {
        let pixels = vec![CLEAR; (width * height) as usize];
        Canvas::new(image(width, height, &pixels, ColorSpace::Srgb), 2.0)
    }

    fn ink(hex: &str, alpha: f64) -> Ink {
        Ink {
            rgb: parse_hex(hex).unwrap(),
            alpha,
            blend: BlendMode::SourceOver,
        }
    }

    fn rect(left: f64, bottom: f64, width: f64, height: f64) -> PathData {
        let (right, top) = (left + width, bottom + height);
        let v = |x, y| V2 { x, y };
        PathData(vec![
            Seg::Move(v(left, bottom)),
            Seg::Line(v(right, bottom)),
            Seg::Line(v(right, top)),
            Seg::Line(v(left, top)),
            Seg::Close,
        ])
    }

    fn pixels(c: &Canvas) -> Vec<[u8; 4]> {
        c.main.to_rgba().as_chunks::<4>().0.to_vec()
    }

    #[test]
    fn sample_uses_reference_row_convention() {
        let black = [0, 0, 0, 255];
        let c = Canvas::new(
            image(2, 2, &[RED, black, black, black], ColorSpace::Srgb),
            1.0,
        );
        assert_eq!(c.sample(0.0, 2.0), [255, 0, 0]);
        // row = H − ceil(y): y in (1, 2] is the top row, (0, 1] the bottom one.
        assert_eq!(c.sample(0.9, 1.5), [255, 0, 0]);
        assert_eq!(c.sample(0.0, 1.0), [0, 0, 0]);
    }

    #[test]
    fn sample_reads_the_original_and_is_black_outside() {
        let white = [255, 255, 255, 255];
        let mut c = Canvas::new(image(1, 1, &[white], ColorSpace::Srgb), 1.0);
        c.main().fill(
            &rect(0.0, 0.0, 1.0, 1.0),
            ink("#000000", 1.0),
            FillRule::Winding,
        );
        assert_eq!(c.sample(0.5, 1.0), [255, 255, 255]);
        for (x, y) in [
            (-0.5, 1.0),
            (1.0, 1.0),
            (0.0, 0.0),
            (0.0, 2.0),
            (f64::NAN, 1.0),
        ] {
            assert_eq!(c.sample(x, y), [0, 0, 0], "({x}, {y})");
        }
    }

    #[test]
    fn source_pixel_is_none_outside_the_image() {
        let c = Canvas::new(image(1, 1, &[RED], ColorSpace::Srgb), 1.0);
        assert_eq!(c.source_pixel(0.5, 1.0), Some([255, 0, 0]));
        for (x, y) in [
            (-0.5, 1.0),
            (1.0, 1.0),
            (0.0, 0.0),
            (0.0, 2.0),
            (f64::NAN, 1.0),
        ] {
            assert_eq!(c.source_pixel(x, y), None, "({x}, {y})");
        }
    }

    #[test]
    fn fill_is_y_up() {
        let mut c = blank(2, 2);
        c.main().fill(
            &rect(0.0, 0.0, 1.0, 1.0),
            ink("#FF0000", 1.0),
            FillRule::Winding,
        );
        assert_eq!(pixels(&c), [CLEAR, CLEAR, RED, CLEAR]);
    }

    #[test]
    fn p3_canvas_converts_ink() {
        let src = image(1, 1, &[CLEAR], ColorSpace::DisplayP3);
        let mut c = Canvas::new(src, 1.0);
        c.main().fill(
            &rect(0.0, 0.0, 1.0, 1.0),
            ink("#FF0000", 1.0),
            FillRule::Winding,
        );
        assert_eq!(pixels(&c), [[234, 51, 35, 255]]);
    }

    #[test]
    fn ink_alpha_scales_coverage() {
        let mut canvas = blank(1, 1);
        canvas.main().fill(
            &rect(0.0, 0.0, 1.0, 1.0),
            ink("#FF0000", 0.5),
            FillRule::Winding,
        );
        let [[r, g, b, a]] = pixels(&canvas)[..] else {
            unreachable!()
        };
        assert_eq!((r, g, b), (255, 0, 0));
        assert!(a.abs_diff(128) <= 1, "{a}");
    }

    #[test]
    fn degenerate_paths_and_zero_width_strokes_draw_nothing() {
        let mut c = blank(2, 2);
        let red = ink("#FF0000", 1.0);
        let style = StrokeStyle {
            width: 1.0,
            cap: LineCap::Butt,
            join: LineJoin::Miter,
            dash: None,
        };
        let zero = StrokeStyle {
            width: 0.0,
            ..style.clone()
        };
        let surface = c.main();
        surface.fill(&PathData::default(), red, FillRule::Winding);
        surface.fill(
            &PathData(vec![Seg::Move(V2::default())]),
            red,
            FillRule::EvenOdd,
        );
        surface.stroke(&PathData::default(), red, &style);
        surface.stroke(&rect(0.0, 0.0, 2.0, 2.0), red, &zero);
        assert_eq!(pixels(&c), [CLEAR; 4]);
    }

    #[test]
    fn stroke_is_y_up_and_honours_dash() {
        let v = |x, y| V2 { x, y };
        let line = PathData(vec![Seg::Move(v(0.0, 1.0)), Seg::Line(v(4.0, 1.0))]);
        let mut c = blank(4, 3);
        let style = StrokeStyle {
            width: 2.0,
            cap: LineCap::Butt,
            join: LineJoin::Miter,
            dash: Some(vec![1.0, 1.0]),
        };
        c.main().stroke(&line, ink("#FF0000", 1.0), &style);
        let dashed = [RED, CLEAR, RED, CLEAR];
        assert_eq!(pixels(&c), [[CLEAR; 4], dashed, dashed].concat());
    }

    #[test]
    fn one_pixel_stroke_paints_on_a_two_pixel_canvas() {
        // tiny-skia's hairline rasterizer, which it uses for strokes up to 1 px wide, draws
        // nothing on a pixmap 2 px tall; a geometric stroke covers exactly the bottom row.
        let v = |x, y| V2 { x, y };
        let line = PathData(vec![Seg::Move(v(0.0, 0.5)), Seg::Line(v(2.0, 0.5))]);
        let mut c = blank(2, 2);
        let style = StrokeStyle {
            width: 1.0,
            cap: LineCap::Butt,
            join: LineJoin::Miter,
            dash: None,
        };
        c.main().stroke(&line, ink("#FF0000", 1.0), &style);
        assert_eq!(pixels(&c), [CLEAR, CLEAR, RED, RED]);
    }

    #[test]
    fn layers_composite_over_main() {
        let mut canvas = blank(2, 1);
        canvas.main().fill(
            &rect(0.0, 0.0, 2.0, 1.0),
            ink("#FF0000", 1.0),
            FillRule::Winding,
        );
        let mut layer = canvas.new_layer();
        layer.fill(
            &rect(1.0, 0.0, 1.0, 1.0),
            ink("#0000FF", 0.5),
            FillRule::Winding,
        );
        assert_eq!(
            pixels(&canvas),
            [RED, RED],
            "drawing on a layer leaves main untouched"
        );
        canvas.composite(&layer);
        let [left, [r, g, b, a]] = pixels(&canvas)[..] else {
            unreachable!()
        };
        assert_eq!(left, RED);
        assert_eq!((g, a), (0, 255));
        assert!(r.abs_diff(127) <= 1 && b.abs_diff(128) <= 1, "{r} {b}");
    }

    #[test]
    fn clear_blend_punches_holes() {
        let mut c = blank(2, 1);
        let mut layer = c.new_layer();
        layer.fill(
            &rect(0.0, 0.0, 2.0, 1.0),
            ink("#000000", 1.0),
            FillRule::Winding,
        );
        let clear = Ink {
            blend: BlendMode::Clear,
            ..ink("#000000", 1.0)
        };
        layer.fill(&rect(0.0, 0.0, 1.0, 1.0), clear, FillRule::Winding);
        c.composite(&layer);
        assert_eq!(pixels(&c), [CLEAR, [0, 0, 0, 255]]);
    }

    #[test]
    fn up_flips_points_and_normalizes_rects() {
        let c = blank(10, 8);
        assert_eq!(c.up([3.0, 2.0]), V2 { x: 3.0, y: 6.0 });
        assert_eq!(c.up_rect([1.0, 2.0, 3.0, 4.0]), (1.0, 2.0, 3.0, 4.0));
        assert_eq!(c.up_rect([4.0, 6.0, -3.0, -4.0]), (1.0, 2.0, 3.0, 4.0));
        assert_eq!((c.width, c.height, c.dpr), (10, 8, 2.0));
    }

    #[test]
    fn put_rgba_places_bottom_left_corner_and_clips() {
        let mut c = blank(3, 3);
        let g = [0, 255, 0, 255];
        let b = [0, 0, 255, 128];
        // 2×2 bitmap, top row [R G], bottom row [B R], bottom-left at y-up (1, 0).
        c.put_rgba(1, 0, 2, 2, &[RED, g, b, RED].concat());
        assert_eq!(
            pixels(&c),
            [CLEAR, CLEAR, CLEAR, CLEAR, RED, g, CLEAR, b, RED]
        );
        // Clips on every side: only the bottom row's last two pixels land, in row 0.
        c.put_rgba(-1, 2, 3, 2, &[RED, RED, RED, g, b, RED].concat());
        assert_eq!(pixels(&c), [b, RED, CLEAR, CLEAR, RED, g, CLEAR, b, RED]);
        // Overwrites rather than blends.
        c.put_rgba(2, -1, 2, 2, &[CLEAR; 4].concat());
        assert_eq!(pixels(&c), [b, RED, CLEAR, CLEAR, RED, g, CLEAR, b, CLEAR]);
        c.put_rgba(-5, 1, 3, 3, &[RED; 9].concat());
        c.put_rgba(5, 5, 1, 1, &RED);
        assert_eq!(pixels(&c), [b, RED, CLEAR, CLEAR, RED, g, CLEAR, b, CLEAR]);
    }

    #[test]
    fn into_png_keeps_pixels_and_metadata() {
        let icc = b"not really a profile".to_vec();
        let half = [10, 20, 30, 128];
        let src = SourceImage {
            icc: Some(icc.clone()),
            ..image(2, 1, &[RED, half], ColorSpace::Srgb)
        };
        let png = Canvas::new(src, 2.0).into_png().unwrap();
        let back = decode_png(&png).unwrap();
        assert_eq!((back.width, back.height), (2, 1));
        assert_eq!(back.dpi, Some(144.0));
        assert_eq!(back.icc, Some(icc));
        let [red, [r, g, b, a]] = back.rgba.as_chunks::<4>().0[..] else {
            unreachable!()
        };
        assert_eq!(red, RED);
        assert_eq!(a, 128);
        // Premultiplying to 8 bits and back loses at most one step per channel.
        assert!(r.abs_diff(10) <= 1 && g.abs_diff(20) <= 1 && b.abs_diff(30) <= 1);
    }
}
