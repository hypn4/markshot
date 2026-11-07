//! The rect and oval tools: a rounded rect or a squircle, stroked and optionally filled, or
//! hand-drawn: a stroke of varying width around the shape whose outline is filled in the
//! color.
//!
//! `fill` picks the style: `sketch` hand-drawn, `outline` the border alone, `semi` and `solid`
//! the border over the color at `fill_alpha`, by default 0.4 and 1.

mod handdrawn;
mod rounded_rect;
mod squircle;

use tiny_skia::{BlendMode, FillRule, LineCap, LineJoin};

use crate::canvas::{Canvas, Ink, StrokeStyle};
use crate::geom::{PathData, Seg, V2};
use crate::image_io::luma;
use crate::resolve::ResolvedShape;
use crate::rng::rnd100;
use crate::spec::{Fill, Rect};
use crate::tools::Pixels;

pub use handdrawn::hd_corner;
pub use rounded_rect::rounded_rect;
pub use squircle::squircle;

/// Stroke widths in points, by size.
pub const WIDTHS: [f64; 10] = [1.0, 2.0, 3.0, 4.0, 6.0, 8.0, 11.0, 15.0, 21.0, 30.0];

/// The rect's jitter strength while hand-drawn: in the hand-drawn outline and in the plain
/// rect drawn instead on small shapes.
const JITTER: f64 = 1.3;

/// Hand-drawn needs a shorter side over this many px; smaller shapes are drawn plain.
const MIN_HANDDRAWN_SIDE: f64 = 40.0;

/// The opacity of the `semi` and `solid` fills when the spec gives no `fill_alpha`.
const SEMI_FILL_ALPHA: f64 = 0.4;
const SOLID_FILL_ALPHA: f64 = 1.0;

/// Which outline a [`ResolvedShape`] gets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeKind {
    Rect,
    Oval,
}

/// Draws `s` on the main surface. A `sketch` over 40 px on its shorter side is hand-drawn;
/// anything else is the plain outline, filled at `fill_alpha` (by default 0.4 for `semi` and 1
/// for `solid`), then stroked with round joins.
///
/// # Panics
/// When `s.size` is 10 or more; resolved sizes are 0 to 7.
pub fn draw(c: &mut Canvas, s: &ResolvedShape, kind: ShapeKind) {
    let g = Geometry::new(c, s);
    let ink = Ink {
        rgb: s.color,
        alpha: 1.0,
        blend: BlendMode::SourceOver,
    };
    if g.handdrawn {
        let outline = match kind {
            ShapeKind::Rect => handdrawn::rect(g.rect, g.wpt, g.stroke, g.dpr, &g.rnd),
            ShapeKind::Oval => handdrawn::oval(g.rect, g.stroke, g.dpr, s.spotlight, &g.rnd),
        };
        c.main().fill(&outline, ink, FillRule::Winding);
        return;
    }
    let path = g.plain_path(kind, s.fill);
    let fill_alpha = match s.fill {
        Fill::Sketch | Fill::Outline => None,
        Fill::Semi => Some(s.fill_alpha.unwrap_or(SEMI_FILL_ALPHA)),
        Fill::Solid => Some(s.fill_alpha.unwrap_or(SOLID_FILL_ALPHA)),
    };
    if let Some(alpha) = fill_alpha {
        let background = Ink { alpha, ..ink };
        c.main().fill(&path, background, FillRule::Winding);
    }
    let style = StrokeStyle {
        width: g.stroke,
        cap: LineCap::Butt,
        join: LineJoin::Round,
        dash: None,
    };
    c.main().stroke(&path, ink, &style);
}

/// The spotlight hole of `s` in y-up px: the centre line [`draw`] strokes, closed, running
/// counter-clockwise like a spotlight's outline so that overlapping holes merge. A hand-drawn
/// oval's spiral, which then runs past its start, is closed by a straight line between its
/// ends.
///
/// # Panics
/// When `s.size` is 10 or more; resolved sizes are 0 to 7.
#[must_use]
pub fn hole(c: &Canvas, s: &ResolvedShape, kind: ShapeKind) -> PathData {
    let g = Geometry::new(c, s);
    if !g.handdrawn {
        return g.plain_path(kind, s.fill);
    }
    let line = match kind {
        ShapeKind::Rect => handdrawn::rect_line(g.rect, g.wpt, g.dpr, &g.rnd),
        ShapeKind::Oval => handdrawn::oval_line(g.rect, g.stroke, g.dpr, s.spotlight, &g.rnd),
    };
    let mut pos: Vec<V2> = line.iter().map(|p| p.pos).collect();
    if signed_area(&pos) < 0.0 {
        pos.reverse();
    }
    let mut segs: Vec<Seg> = pos.iter().map(|&p| Seg::Line(p)).collect();
    if let Some(Seg::Line(first)) = segs.first().copied() {
        segs[0] = Seg::Move(first);
        segs.push(Seg::Close);
    }
    PathData(segs)
}

/// What [`draw`] and [`hole`] share about `s`: its y-up rect, widths, randomness and whether
/// it is hand-drawn.
struct Geometry {
    rect: (f64, f64, f64, f64),
    dpr: f64,
    /// The size's width in points.
    wpt: f64,
    /// The stroke width in px.
    stroke: f64,
    rnd: [f64; 100],
    /// A `sketch` over 40 px on its shorter side.
    handdrawn: bool,
}

impl Geometry {
    /// The geometry of `s` on `c`.
    ///
    /// # Panics
    /// When `s.size` is 10 or more; resolved sizes are 0 to 7.
    fn new(c: &Canvas, s: &ResolvedShape) -> Self {
        let rect = c.up_rect(s.rect);
        let (_, _, w, h) = rect;
        let wpt = WIDTHS[usize::from(s.size)];
        Self {
            rect,
            dpr: c.dpr,
            wpt,
            stroke: stroke_px(w, h, wpt, c.dpr),
            rnd: rnd100(s.seed),
            handdrawn: s.fill == Fill::Sketch && w.min(h) > MIN_HANDDRAWN_SIDE,
        }
    }

    /// The outline of a shape that is not hand-drawn. Intentional: a small `sketch` rect keeps
    /// the hand-drawn jitter.
    fn plain_path(&self, kind: ShapeKind, fill: Fill) -> PathData {
        let jitter = if fill == Fill::Sketch { JITTER } else { 0.0 };
        plain_path(kind, self.rect, jitter, 2.0 * self.wpt, self.dpr, &self.rnd)
    }
}

/// How many vertical paddings [`fit_oval`] tries at most on a target taller than that in px.
const FIT_STEPS: u32 = 24;

/// The shares by which [`fit_oval`] moves an oval up (positive) or down off its target's centre,
/// as a share of the average padding above and below; centred first.
const FIT_SHIFTS: [f64; 7] = [0.0, 0.25, -0.25, 0.5, -0.5, 0.75, -0.75];

/// What [`fit_oval`] charges for each stretch of its line that runs onto ink.
const INK_RUN: u32 = 40;

/// The `rect` (y-down px) for `oval` (its size, seed, fill and `spotlight`) at `dpr` that
/// circles `target` (y-down px) on `image`. Each vertical padding in whole px from half the
/// line width to the target's height plus six line widths (at most 24 of them, evenly spaced,
/// on a huge target), split evenly above and below or moved up or down by `FIT_SHIFTS`, gets
/// the least horizontal padding (whole px, by bisection) whose actual centre line clears the
/// target by its reach: half its width, a quarter more for a hand-drawn line's swell and
/// 1.5 px for its anti-aliased edge. Of those, the oval that costs least wins: `ink_cost`
/// for the ink its line comes near, the `obstacles` it runs through and the points outside
/// `image.area`, plus `enclosed_ink` for anything else it would circle. Ties go to the smaller
/// rect, then to the smaller vertical padding. Returns the rect and its cost (0: the line keeps
/// clear of all ink, obstacles and edges). A target without area is returned as it is.
#[must_use]
pub fn fit_oval(
    target: Rect,
    oval: &ResolvedShape,
    dpr: f64,
    image: &Pixels<'_>,
    obstacles: &[[f64; 4]],
) -> (Rect, u32) {
    let [tx, ty, tw, th] = target;
    if tw <= 0.0 || th <= 0.0 {
        return (target, 0);
    }
    let (fill, spotlight) = (oval.fill, oval.spotlight);
    let rnd = rnd100(oval.seed);
    let wpt = WIDTHS[usize::from(oval.size)];
    let height = f64::from(image.height);
    // The oval's centre line (y-down px) on `rect` and the clearance it needs.
    let line = |rect: Rect| -> (Vec<V2>, f64) {
        let [x, y, w, h] = rect;
        let up = (x, height - y - h, w, h);
        let stroke = stroke_px(w, h, wpt, dpr);
        let handdrawn = fill == Fill::Sketch && w.min(h) > MIN_HANDDRAWN_SIDE;
        let points: Vec<V2> = if handdrawn {
            handdrawn::oval_line(up, stroke, dpr, spotlight, &rnd)
                .iter()
                .map(|p| p.pos)
                .collect()
        } else {
            sample_path(&squircle(up.0, up.1, up.2, up.3))
        };
        let swell = if handdrawn { 0.625 } else { 0.5 };
        let down = points
            .into_iter()
            .map(|p| V2 {
                x: p.x,
                y: height - p.y,
            })
            .collect();
        (down, stroke * swell + 1.5)
    };
    let clears = |points: &[V2], need: f64| {
        points.iter().all(|p| {
            let dx = (tx - p.x).max(p.x - (tx + tw)).max(0.0);
            let dy = (ty - p.y).max(p.y - (ty + th)).max(0.0);
            dx * dx + dy * dy >= need * need
        })
    };
    // `pv` px above and below on average, shifted up (`shift` > 0) or down by a share of it.
    let padded = |pv: f64, shift: f64, ph: f64| {
        let top = (pv * (1.0 + shift)).round();
        [tx - ph, ty - top, tw + 2.0 * ph, th + 2.0 * pv]
    };
    let line_px = wpt * dpr;
    let (least_pad, most_pad) = ((0.5 * line_px).round(), (th + 6.0 * line_px).round());
    let widest = (8.0 * th + tw + 8.0 * line_px).ceil();
    // Every whole px of vertical padding, or FIT_STEPS evenly spaced ones on a huge target.
    let step = ((most_pad - least_pad) / f64::from(FIT_STEPS - 1)).max(1.0);
    // Ink anywhere the widest, tallest candidate's line could reach.
    let margin = widest + most_pad * 2.0 + 4.0 * line_px + 8.0;
    let ink = InkMap::new(
        [tx - margin, ty - margin, tx + tw + margin, ty + th + margin],
        target,
        image,
    );
    let mut best: Option<(u32, f64, Rect)> = None;
    let mut pv = least_pad;
    while pv <= most_pad {
        for shift in FIT_SHIFTS {
            let (outer, need) = line(padded(pv, shift, widest));
            if !clears(&outer, need) {
                continue;
            }
            let (mut short, mut enough) = (0.0_f64, widest);
            while enough - short > 1.0 {
                let mid = short.midpoint(enough).floor();
                let (points, need) = line(padded(pv, shift, mid));
                if clears(&points, need) {
                    enough = mid;
                } else {
                    short = mid;
                }
            }
            let rect = padded(pv, shift, enough);
            let (points, reach) = line(rect);
            let cost = ink_cost(&points, reach, image, ink.as_ref(), obstacles)
                + enclosed_ink(rect, dpr, ink.as_ref());
            let better = match best {
                None => true,
                Some((c, area, _)) => cost < c || (cost == c && rect[2] * rect[3] < area),
            };
            if better {
                best = Some((cost, rect[2] * rect[3], rect));
            }
        }
        pv = (pv + step).round();
    }
    best.map_or_else(
        || (padded(most_pad, 0.0, widest), u32::MAX),
        |(cost, _, rect)| (rect, cost),
    )
}

/// Distances to the nearest ink pixel over part of an image, for [`fit_oval`]. Ink is a pixel
/// outside the target whose luma differs from the region's median by over 0.15; distances are
/// chamfer distances (1 across, 1.4 diagonally) in px, capped at [`InkMap::FAR`].
struct InkMap {
    left: f64,
    top: f64,
    width: usize,
    height: usize,
    dist: Vec<f64>,
}

impl InkMap {
    /// Farther than any line reach; the cap of every distance.
    const FAR: f64 = 1.0e6;

    /// The map of `region` (`[x0, y0, x1, y1]`, y-down px, clipped to the image), ignoring the
    /// pixels inside `target`; `None` without pixels or area.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the region is clipped to the image, so its corners are small non-negative pixels"
    )]
    fn new(region: [f64; 4], target: Rect, image: &Pixels<'_>) -> Option<Self> {
        let rgba = image.rgba?;
        let (w, h) = (f64::from(image.width), f64::from(image.height));
        let left = region[0].clamp(0.0, w).floor();
        let top = region[1].clamp(0.0, h).floor();
        let right = region[2].clamp(0.0, w).ceil();
        let bottom = region[3].clamp(0.0, h).ceil();
        if right <= left || bottom <= top {
            return None;
        }
        let (width, height) = ((right - left) as usize, (bottom - top) as usize);
        let [tx, ty, tw, th] = target;
        let mut lumas = Vec::with_capacity(width * height);
        let mut in_target = Vec::with_capacity(width * height);
        let mut y = top;
        while y < bottom {
            let mut x = left;
            while x < right {
                lumas.push(luma(rgba, image.width, [x, y]));
                in_target.push((tx..tx + tw).contains(&x) && (ty..ty + th).contains(&y));
                x += 1.0;
            }
            y += 1.0;
        }
        let mut sorted = lumas.clone();
        sorted.sort_by(f64::total_cmp);
        let median = sorted[sorted.len() / 2];
        let mut dist: Vec<f64> = lumas
            .iter()
            .zip(&in_target)
            .map(|(&l, &inside)| {
                if !inside && (l - median).abs() > 0.15 {
                    0.0
                } else {
                    Self::FAR
                }
            })
            .collect();
        // Two chamfer passes: down and right, then up and left.
        let at = |row: usize, col: usize| row * width + col;
        for row in 0..height {
            for col in 0..width {
                let mut d = dist[at(row, col)];
                if col > 0 {
                    d = d.min(dist[at(row, col - 1)] + 1.0);
                }
                if row > 0 {
                    d = d.min(dist[at(row - 1, col)] + 1.0);
                    if col > 0 {
                        d = d.min(dist[at(row - 1, col - 1)] + 1.4);
                    }
                    if col + 1 < width {
                        d = d.min(dist[at(row - 1, col + 1)] + 1.4);
                    }
                }
                dist[at(row, col)] = d;
            }
        }
        for row in (0..height).rev() {
            for col in (0..width).rev() {
                let mut d = dist[at(row, col)];
                if col + 1 < width {
                    d = d.min(dist[at(row, col + 1)] + 1.0);
                }
                if row + 1 < height {
                    d = d.min(dist[at(row + 1, col)] + 1.0);
                    if col + 1 < width {
                        d = d.min(dist[at(row + 1, col + 1)] + 1.4);
                    }
                    if col > 0 {
                        d = d.min(dist[at(row + 1, col - 1)] + 1.4);
                    }
                }
                dist[at(row, col)] = d;
            }
        }
        Some(Self {
            left,
            top,
            width,
            height,
            dist,
        })
    }

    /// The distance from `p` (y-down px) to the nearest ink; [`InkMap::FAR`] off the map.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "checked to lie on the map first"
    )]
    fn distance(&self, p: V2) -> f64 {
        let (col, row) = ((p.x - self.left).floor(), (p.y - self.top).floor());
        if col < 0.0 || row < 0.0 || col as usize >= self.width || row as usize >= self.height {
            return Self::FAR;
        }
        self.dist[row as usize * self.width + col as usize]
    }
}

/// What it costs to draw a line through `points` (y-down px, in order) that reaches `reach` px
/// to either side of them; see [`fit_oval`]. A point outside `image.area` costs 10, one inside
/// any of `obstacles` (`[x0, y0, x1, y1]`, y-down px) 5, one whose line would come within 3 px
/// of ink 1, and every stretch of line that runs that close to ink [`INK_RUN`] more, so that a
/// line cutting across a row of text costs about as much as one running along it. Points are
/// checked every pixel along the line.
fn ink_cost(
    points: &[V2],
    reach: f64,
    image: &Pixels<'_>,
    ink: Option<&InkMap>,
    obstacles: &[[f64; 4]],
) -> u32 {
    let [x0, y0, x1, y1] = image.area;
    // Keep 3 px of air between ink and the line's reach.
    let near = reach + 3.0;
    let mut cost = 0;
    let mut on_ink = false;
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let steps = (b - a).len().ceil().max(1.0);
        let mut i = 0.0;
        while i < steps {
            let p = a + (b - a) * (i / steps);
            i += 1.0;
            if !(x0..x1).contains(&p.x) || !(y0..y1).contains(&p.y) {
                cost += 10;
            }
            if obstacles.iter().any(|&[ox0, oy0, ox1, oy1]| {
                (ox0..=ox1).contains(&p.x) && (oy0..=oy1).contains(&p.y)
            }) {
                cost += 5;
            }
            let inked = ink.is_some_and(|map| map.distance(p) < near);
            if inked {
                cost += 1;
                if !on_ink {
                    cost += INK_RUN;
                }
            }
            on_ink = inked;
        }
    }
    cost
}

/// The ink an oval on `rect` would circle besides its target: samples every `2 * dpr` px
/// inside the ellipse inscribed in `rect` (y-down px) that lie on ink.
fn enclosed_ink(rect: Rect, dpr: f64, ink: Option<&InkMap>) -> u32 {
    let Some(map) = ink else {
        return 0;
    };
    let [x, y, w, h] = rect;
    let (cx, cy, rx, ry) = (x + w / 2.0, y + h / 2.0, w / 2.0, h / 2.0);
    let step = 2.0 * dpr;
    let mut count = 0;
    let mut sy = y + step / 2.0;
    while sy < y + h {
        let mut sx = x + step / 2.0;
        while sx < x + w {
            let (dx, dy) = ((sx - cx) / rx, (sy - cy) / ry);
            if dx * dx + dy * dy <= 1.0 && map.distance(V2 { x: sx, y: sy }) == 0.0 {
                count += 1;
            }
            sx += step;
        }
        sy += step;
    }
    count
}

/// Points along `path`'s lines and cubics, each cubic sampled at 16 even steps of its
/// parameter.
fn sample_path(path: &PathData) -> Vec<V2> {
    let mut points = Vec::new();
    let mut last = V2::default();
    for seg in &path.0 {
        match *seg {
            Seg::Move(p) | Seg::Line(p) => {
                points.push(p);
                last = p;
            }
            Seg::Cubic(a, b, end) => {
                for i in 1..=16_u8 {
                    let t = f64::from(i) / 16.0;
                    let u = 1.0 - t;
                    points.push(
                        last * (u * u * u)
                            + a * (3.0 * u * u * t)
                            + b * (3.0 * u * t * t)
                            + end * (t * t * t),
                    );
                }
                last = end;
            }
            Seg::Close => {}
        }
    }
    points
}

/// Twice the signed area of the polygon `pts` (y-up): positive when it runs
/// counter-clockwise.
fn signed_area(pts: &[V2]) -> f64 {
    pts.iter()
        .zip(pts.iter().cycle().skip(1))
        .map(|(a, b)| a.x * b.y - b.x * a.y)
        .sum()
}

/// The outline of `kind` on `rect` when not hand-drawn: the rounded rect with corners of `cp`
/// points, or the squircle.
fn plain_path(
    kind: ShapeKind,
    (x, y, w, h): (f64, f64, f64, f64),
    jitter: f64,
    cp: f64,
    dpr: f64,
    rnd: &[f64; 100],
) -> PathData {
    match kind {
        ShapeKind::Rect => rounded_rect(x, y, w, h, jitter, cp, dpr, rnd),
        ShapeKind::Oval => squircle(x, y, w, h),
    }
}

/// The stroke width in px of a `w × h` shape whose size is `wpt` points wide: `wpt`, at most a
/// tenth of the shorter side, times dpr.
#[must_use]
pub fn stroke_px(w: f64, h: f64, wpt: f64, dpr: f64) -> f64 {
    // Intentional: a tenth of the shorter side in px caps a width in points.
    (w.abs().min(h.abs()) / 10.0).min(wpt) * dpr
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::parse_hex;
    use crate::image_io::{ColorSpace, SourceImage, decode_png};

    const RED: [u8; 4] = [255, 12, 1, 255];
    const WHITE: [u8; 4] = [255; 4];

    /// A size-2 `#FF0C01` shape with seed 7 and no `fill_alpha`.
    fn shape(fill: Fill, rect: [f64; 4]) -> ResolvedShape {
        ResolvedShape {
            id: "rect-1".to_owned(),
            color: parse_hex("#FF0C01").unwrap(),
            size: 2,
            seed: 7,
            rect,
            fill,
            fill_alpha: None,
            spotlight: false,
            around: None,
            pick_seed: false,
            pick_size: false,
        }
    }

    /// A white `width` × `height` canvas at dpr 2.
    fn white(width: u32, height: u32) -> Canvas {
        let src = SourceImage {
            width,
            height,
            rgba: vec![255; (width * height * 4) as usize],
            dpi: None,
            icc: None,
            color_space: ColorSpace::Srgb,
        };
        Canvas::new(src, 2.0)
    }

    /// `s` drawn as `kind` on a white `width` × `height` canvas at dpr 2, as PNG.
    fn png(s: &ResolvedShape, kind: ShapeKind, width: u32, height: u32) -> Vec<u8> {
        let mut c = white(width, height);
        draw(&mut c, s, kind);
        c.into_png().unwrap()
    }

    /// The RGBA pixel at column `x`, row `y` (top-down) of `png`, `width` pixels wide.
    fn pixel(png: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
        let rgba = decode_png(png).unwrap().rgba;
        let i = ((y * width + x) * 4) as usize;
        rgba[i..i + 4].try_into().unwrap()
    }

    /// `target`'s plain outline (the rounded rect with jitter `jitter` and 6-point corners, or
    /// the squircle) stroked `width` px in its color with round joins, as a shape that is not
    /// hand-drawn is drawn; on a white canvas as `png` makes it.
    fn plain(
        target: &ResolvedShape,
        kind: ShapeKind,
        jitter: f64,
        width: f64,
        size: (u32, u32),
    ) -> Vec<u8> {
        let mut canvas = white(size.0, size.1);
        let (x, y, w, h) = canvas.up_rect(target.rect);
        let path = match kind {
            ShapeKind::Rect => rounded_rect(x, y, w, h, jitter, 6.0, 2.0, &rnd100(target.seed)),
            ShapeKind::Oval => squircle(x, y, w, h),
        };
        let ink = Ink {
            rgb: target.color,
            alpha: 1.0,
            blend: BlendMode::SourceOver,
        };
        let style = StrokeStyle {
            width,
            cap: LineCap::Butt,
            join: LineJoin::Round,
            dash: None,
        };
        canvas.main().stroke(&path, ink, &style);
        canvas.into_png().unwrap()
    }

    #[test]
    fn stroke_px_caps_small_shapes() {
        assert_eq!(stroke_px(200., 100., 3.0, 2.0), 6.0);
        assert_eq!(stroke_px(20., 100., 3.0, 2.0), 4.0);
        assert_eq!(stroke_px(100., 20., 1.0, 2.0), 2.0);
    }

    #[test]
    fn sketch_falls_back_when_small() {
        // A shorter side of 40 px is not over 40: the sketch is the plain outline, 6 px wide
        // (min(40/10, 3 pt) * 2), the rect still with the hand-drawn jitter.
        let s = shape(Fill::Sketch, [30.0, 20.0, 40.0, 200.0]);
        for kind in [ShapeKind::Rect, ShapeKind::Oval] {
            assert!(
                png(&s, kind, 100, 240) == plain(&s, kind, 1.3, 6.0, (100, 240)),
                "{kind:?}"
            );
        }
    }

    #[test]
    fn sketch_over_40_px_is_hand_drawn() {
        let s = shape(Fill::Sketch, [30.0, 20.0, 41.0, 200.0]);
        for kind in [ShapeKind::Rect, ShapeKind::Oval] {
            assert!(
                png(&s, kind, 100, 240) != plain(&s, kind, 1.3, 6.0, (100, 240)),
                "{kind:?}"
            );
        }
    }

    #[test]
    fn outline_is_stroked_centred_on_the_edges() {
        // 160 × 60 at size 2: a 6 px stroke, so the rect's left edge at x = 20 covers columns
        // 17–22 (mid-height, below the 12 px corners); the squircle's is round there, so only
        // its inner columns are certain. Nothing inside.
        let s = shape(Fill::Outline, [20.0, 20.0, 160.0, 60.0]);
        for (kind, red, white) in [
            (ShapeKind::Rect, 17..=22, [16, 23, 100]),
            (ShapeKind::Oval, 18..=21, [15, 24, 100]),
        ] {
            let png = png(&s, kind, 200, 100);
            for x in red {
                assert_eq!(pixel(&png, 200, x, 50), RED, "{kind:?} column {x}");
            }
            for x in white {
                assert_eq!(pixel(&png, 200, x, 50), WHITE, "{kind:?} column {x}");
            }
        }
    }

    #[test]
    fn semi_and_solid_fill_inside_the_border() {
        // #FF0C01 at 0.4 over white is (255, 157.8, 153.4), so (255, 158, 153).
        for (fill, want) in [
            (Fill::Outline, WHITE),
            (Fill::Semi, [255, 158, 153, 255]),
            (Fill::Solid, RED),
        ] {
            let s = shape(fill, [20.0, 20.0, 160.0, 60.0]);
            for kind in [ShapeKind::Rect, ShapeKind::Oval] {
                let png = png(&s, kind, 200, 100);
                let got = pixel(&png, 200, 100, 50);
                assert!(
                    got.iter().zip(want).all(|(&g, w)| g.abs_diff(w) <= 1),
                    "{fill:?} {kind:?}: {got:?}"
                );
                assert_eq!(pixel(&png, 200, 20, 50), RED, "{fill:?} {kind:?} border");
            }
        }
    }

    #[test]
    fn fill_alpha_sets_the_background_opacity() {
        // Solid at 0.5: (255, 133.5, 128).
        let mut s = shape(Fill::Solid, [20.0, 20.0, 160.0, 60.0]);
        s.fill_alpha = Some(0.5);
        let got = pixel(&png(&s, ShapeKind::Rect, 200, 100), 200, 100, 50);
        assert!(
            got.iter()
                .zip([255, 134, 128, 255])
                .all(|(&g, w)| g.abs_diff(w) <= 1),
            "{got:?}"
        );
    }

    /// Whether row `y` of `png` (`width` wide) has a pixel of the color between columns
    /// `x0` and `x1`.
    fn red_in_row(png: &[u8], width: u32, y: u32, x0: u32, x1: u32) -> bool {
        (x0..=x1).any(|x| pixel(png, width, x, y) == RED)
    }

    /// Whether column `x` of `png` (`width` wide) has a pixel of the color between rows `y0`
    /// and `y1`.
    fn red_in_column(png: &[u8], width: u32, x: u32, y0: u32, y1: u32) -> bool {
        (y0..=y1).any(|y| pixel(png, width, x, y) == RED)
    }

    #[test]
    fn sketch_rect_is_a_hollow_stroke_along_the_edges() {
        // 300 × 200: a 6 px stroke 4.4–7.6 px wide, shifted at most 1 px, edges bowed under
        // 1 px; corners of 37 px, whose curve passes 37/8 px inside the sharp corner.
        let png = png(
            &shape(Fill::Sketch, [50.0, 50.0, 300.0, 200.0]),
            ShapeKind::Rect,
            400,
            300,
        );
        assert!(red_in_row(&png, 400, 150, 47, 53), "left edge");
        assert!(red_in_row(&png, 400, 150, 347, 353), "right edge");
        assert!(red_in_column(&png, 400, 200, 47, 53), "top edge");
        assert!(red_in_column(&png, 400, 200, 247, 253), "bottom edge");
        for (x, y) in [
            (200, 150),
            (40, 150),
            (60, 150),
            (200, 40),
            (200, 60),
            (49, 49),
        ] {
            assert_eq!(pixel(&png, 400, x, y), WHITE, "({x}, {y})");
        }
    }

    #[test]
    fn sketch_oval_is_a_ring_through_the_mid_edges() {
        // 300 × 200, stroke 6 px under a fifth of 200: a spiral whose distance drifts ±7.8 px
        // around the rect's mid-edges.
        let png = png(
            &shape(Fill::Sketch, [50.0, 50.0, 300.0, 200.0]),
            ShapeKind::Oval,
            400,
            300,
        );
        assert!(red_in_row(&png, 400, 150, 40, 60), "left");
        assert!(red_in_row(&png, 400, 150, 340, 360), "right");
        assert!(red_in_column(&png, 400, 200, 40, 60), "top");
        assert!(red_in_column(&png, 400, 200, 240, 260), "bottom");
        for (x, y) in [(200, 150), (60, 60), (340, 60), (60, 240), (340, 240)] {
            assert_eq!(pixel(&png, 400, x, y), WHITE, "({x}, {y})");
        }
    }

    #[test]
    fn sketch_is_seed_deterministic() {
        for kind in [ShapeKind::Rect, ShapeKind::Oval] {
            let mut s = shape(Fill::Sketch, [50.0, 50.0, 300.0, 200.0]);
            let first = png(&s, kind, 400, 300);
            assert!(png(&s, kind, 400, 300) == first, "{kind:?}");
            s.seed = 8;
            assert!(png(&s, kind, 400, 300) != first, "{kind:?}");
        }
    }
}
