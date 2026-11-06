//! Text drawn as glyph outlines of the embedded Inter 4.1 fonts (OFL, `assets/fonts`), so it
//! renders the same on every platform without system fonts (spec §3). Lengths are image px,
//! y up. Glyphs are set by their advances alone: no kerning or other shaping.

use std::sync::LazyLock;

use ttf_parser::{Face, GlyphId, OutlineBuilder};

use crate::geom::{PathData, Seg, V2};

/// The embedded weights: the reference app labels counters in the system font's Regular or Medium.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Weight {
    Regular,
    Medium,
}

static REGULAR: LazyLock<Face<'static>> = LazyLock::new(|| {
    Face::parse(include_bytes!("../assets/fonts/Inter-Regular.ttf"), 0)
        .expect("the embedded Inter Regular parses")
});

static MEDIUM: LazyLock<Face<'static>> = LazyLock::new(|| {
    Face::parse(include_bytes!("../assets/fonts/Inter-Medium.ttf"), 0)
        .expect("the embedded Inter Medium parses")
});

fn face(w: Weight) -> &'static Face<'static> {
    match w {
        Weight::Regular => &REGULAR,
        Weight::Medium => &MEDIUM,
    }
}

/// `s` set on one line at `px` px per em: its advance width, and its line height
/// `ascender − descender + line gap`.
#[must_use]
pub fn line_box(s: &str, w: Weight, px: f64) -> (f64, f64) {
    let face = face(w);
    let scale = px / f64::from(face.units_per_em());
    let advance: u32 = glyphs(face, s).map(|g| advance(face, g)).sum();
    let height =
        f64::from(face.ascender()) - f64::from(face.descender()) + f64::from(face.line_gap());
    (f64::from(advance) * scale, height * scale)
}

/// The outlines of `s` at `px` px per em, set on one line whose box (see [`line_box`]) has its
/// bottom-left corner at `origin`: the baseline runs |descender| above it.
#[must_use]
pub fn text_path(s: &str, w: Weight, px: f64, origin: V2) -> PathData {
    let face = face(w);
    let scale = px / f64::from(face.units_per_em());
    let mut outlines = Outlines {
        path: PathData::default(),
        origin: V2 {
            x: origin.x,
            y: origin.y + f64::from(face.descender()).abs() * scale,
        },
        scale,
        last: V2::default(),
    };
    let mut pen = 0;
    for g in glyphs(face, s) {
        outlines.origin.x = origin.x + f64::from(pen) * scale;
        // A glyph without an outline, such as a space, only advances.
        let _ = face.outline_glyph(g, &mut outlines);
        pen += advance(face, g);
    }
    outlines.path
}

/// The outline of `ch` at `px` px per em with its origin, on the baseline, at `origin`;
/// characters the font lacks get its `.notdef` box.
#[must_use]
pub fn glyph_path(ch: char, w: Weight, px: f64, origin: V2) -> PathData {
    let face = face(w);
    let mut outlines = Outlines {
        path: PathData::default(),
        origin,
        scale: px / f64::from(face.units_per_em()),
        last: V2::default(),
    };
    let _ = face.outline_glyph(glyph(face, ch), &mut outlines);
    outlines.path
}

/// Where the ink of `ch` at `px` px per em starts and ends, from its origin: `(left, right)`,
/// or `(0, 0)` when it has none.
#[must_use]
pub fn ink_span(ch: char, w: Weight, px: f64) -> (f64, f64) {
    let face = face(w);
    let scale = px / f64::from(face.units_per_em());
    face.glyph_bounding_box(glyph(face, ch))
        .map_or((0.0, 0.0), |b| {
            (f64::from(b.x_min) * scale, f64::from(b.x_max) * scale)
        })
}

/// The glyphs of `s`; characters the font lacks get its `.notdef` box.
fn glyphs<'a>(face: &'a Face<'_>, s: &'a str) -> impl Iterator<Item = GlyphId> + 'a {
    s.chars().map(|ch| glyph(face, ch))
}

/// The glyph of `ch`, or `.notdef`.
fn glyph(face: &Face<'_>, ch: char) -> GlyphId {
    face.glyph_index(ch).unwrap_or(GlyphId(0))
}

/// The advance width of `g` in font units.
fn advance(face: &Face<'_>, g: GlyphId) -> u32 {
    face.glyph_hor_advance(g).map_or(0, u32::from)
}

/// Collects glyph outlines in font units (y up), scaled to px from the current glyph's origin on
/// the baseline. TrueType's quadratic curves become the equivalent cubics.
struct Outlines {
    path: PathData,
    origin: V2,
    /// px per font unit.
    scale: f64,
    /// The current point.
    last: V2,
}

impl Outlines {
    fn at(&self, x: f32, y: f32) -> V2 {
        V2 {
            x: self.origin.x + f64::from(x) * self.scale,
            y: self.origin.y + f64::from(y) * self.scale,
        }
    }

    fn push(&mut self, seg: Seg, end: V2) {
        self.path.0.push(seg);
        self.last = end;
    }
}

impl OutlineBuilder for Outlines {
    fn move_to(&mut self, x: f32, y: f32) {
        let p = self.at(x, y);
        self.push(Seg::Move(p), p);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let p = self.at(x, y);
        self.push(Seg::Line(p), p);
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (q, p) = (self.at(x1, y1), self.at(x, y));
        // The same curve as a cubic: its control points are 2/3 of the way to `q`.
        let c1 = self.last + (q - self.last) * (2.0 / 3.0);
        let c2 = p + (q - p) * (2.0 / 3.0);
        self.push(Seg::Cubic(c1, c2, p), p);
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let p = self.at(x, y);
        self.push(Seg::Cubic(self.at(x1, y1), self.at(x2, y2), p), p);
    }

    fn close(&mut self) {
        self.path.0.push(Seg::Close);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every point of `p`, control points included.
    fn points(p: &PathData) -> Vec<V2> {
        p.0.iter()
            .flat_map(|s| match *s {
                Seg::Move(a) | Seg::Line(a) => vec![a],
                Seg::Cubic(c1, c2, a) => vec![c1, c2, a],
                Seg::Close => vec![],
            })
            .collect()
    }

    /// The bounds `(x0, y0, x1, y1)` of `pts`.
    fn bounds(pts: &[V2]) -> (f64, f64, f64, f64) {
        pts.iter().fold(
            (
                f64::INFINITY,
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::NEG_INFINITY,
            ),
            |(x0, y0, x1, y1), p| (x0.min(p.x), y0.min(p.y), x1.max(p.x), y1.max(p.y)),
        )
    }

    #[test]
    fn line_box_spans_the_advances_and_the_line() {
        // Inter 4.1 has 2048 units per em. "1" advances 833 units in Regular and 850 in Medium,
        // "2" 1249 and 1262; both weights have ascender 1984, descender −494 and no line gap.
        assert_eq!(line_box("12", Weight::Regular, 2048.0), (2082.0, 2478.0));
        let (w, h) = line_box("12", Weight::Medium, 20.48);
        assert!(
            (w - 21.12).abs() < 1e-9 && (h - 24.78).abs() < 1e-9,
            "{w} × {h}"
        );
    }

    #[test]
    fn glyphs_sit_on_the_baseline_one_after_another() {
        // At 2048 px per em a font unit is a pixel. Regular "T" spans x 98–1224 and y 0–1490 from
        // its origin, which is 494 (|descender|) above the box corner.
        let t = text_path("T", Weight::Regular, 2048.0, V2 { x: 10.0, y: 20.0 });
        assert_eq!(bounds(&points(&t)), (108.0, 514.0, 1234.0, 2004.0));
        // "1" (x 96–653, advance 833) then "T", 833 further right.
        let one_t = text_path("1T", Weight::Regular, 2048.0, V2::default());
        assert_eq!(bounds(&points(&one_t)), (96.0, 494.0, 2057.0, 1984.0));
    }

    #[test]
    fn one_glyph_sits_on_its_origin() {
        // At 2048 px per em a font unit is a pixel. Medium "1" spans x 95–685 and y 0–1490 from
        // its origin on the baseline.
        let one = glyph_path('1', Weight::Medium, 2048.0, V2 { x: 10.0, y: 20.0 });
        assert_eq!(bounds(&points(&one)), (105.0, 20.0, 695.0, 1510.0));
        assert_eq!(ink_span('1', Weight::Medium, 2048.0), (95.0, 685.0));
        assert_eq!(ink_span('1', Weight::Regular, 1024.0), (48.0, 326.5));
        assert_eq!(ink_span(' ', Weight::Regular, 2048.0), (0.0, 0.0));
    }

    #[test]
    fn text_is_upright() {
        // Upright, the top fifth of "T" is its bar and the bottom fifth its stem, a few times
        // narrower; upside down it is the other way round.
        let pts = points(&text_path("T", Weight::Regular, 40.0, V2::default()));
        let (_, y0, _, y1) = bounds(&pts);
        assert!(y1 > y0, "empty outline");
        let band = 0.2 * (y1 - y0);
        let x_extent = |keep: &dyn Fn(f64) -> bool| {
            let (x0, _, x1, _) = bounds(
                &pts.iter()
                    .copied()
                    .filter(|p| keep(p.y))
                    .collect::<Vec<_>>(),
            );
            x1 - x0
        };
        let top = x_extent(&|y| y >= y1 - band);
        let bottom = x_extent(&|y| y <= y0 + band);
        assert!(top > 3.0 * bottom, "top {top} px wide, bottom {bottom} px");
    }
}
