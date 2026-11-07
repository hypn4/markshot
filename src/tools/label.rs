//! The text tool: a short label in the embedded Inter Medium, one line per `\n`. The `label`
//! style puts it on a rounded plate in the color, white or black by the plate's luminance; the
//! `plain` style draws it in the color over a contrasting halo.

use tiny_skia::{BlendMode, FillRule, LineCap, LineJoin};

use crate::canvas::{Canvas, Ink, StrokeStyle};
use crate::color::Rgb;
use crate::geom::{PathData, V2};
use crate::resolve::ResolvedText;
use crate::spec::TextStyle;
use crate::text::{Weight, line_box, text_path};

/// Text sizes in points per em, by size.
pub const POINTS: [f64; 8] = [10.0, 12.0, 14.0, 16.0, 20.0, 24.0, 32.0, 40.0];

/// Plates with a luminance above this get black text, others white, as counters do.
const LIGHT_PLATE: f64 = 0.6;

const WHITE: Rgb = Rgb {
    r: 1.0,
    g: 1.0,
    b: 1.0,
};

const BLACK: Rgb = Rgb {
    r: 0.0,
    g: 0.0,
    b: 0.0,
};

/// Where the lines of a text go, in px.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    /// Pixels per em.
    pub px: f64,
    /// Space between the box's left and right edges and the lines.
    pub pad_x: f64,
    /// Space between the box's top and bottom edges and the lines.
    pub pad_y: f64,
    /// The pitch of the lines: Inter's line height at `px`.
    pub line_height: f64,
    /// The box's width and height.
    pub width: f64,
    pub height: f64,
}

/// The layout of `text` at `size` (0 to 7) and `dpr`. A label pads its lines by half an em
/// across and a quarter em down; plain text is padded by its halo, 0.15 em.
///
/// # Panics
/// When `size` is 8 or more; resolved sizes are 0 to 7.
#[must_use]
pub fn layout(text: &str, size: u8, style: TextStyle, dpr: f64) -> Layout {
    let px = POINTS[usize::from(size)] * dpr;
    let (pad_x, pad_y) = match style {
        TextStyle::Label => (0.5 * px, 0.25 * px),
        TextStyle::Plain => (0.15 * px, 0.15 * px),
    };
    let mut lines = 0.0;
    let mut widest: f64 = 0.0;
    let mut line_height = 0.0;
    for line in text.split('\n') {
        let (advance, height) = line_box(line, Weight::Medium, px);
        widest = widest.max(advance);
        line_height = height;
        lines += 1.0;
    }
    Layout {
        px,
        pad_x,
        pad_y,
        line_height,
        width: widest + 2.0 * pad_x,
        height: lines * line_height + 2.0 * pad_y,
    }
}

/// Draws `t` on the main surface: the plate, a rect with quarter-circle corners a quarter of an
/// em in radius,
/// then the lines; or, plain, each line's outline stroked a fifth of an em wide in the halo
/// color with round joins under the line in the text's color.
///
/// # Panics
/// When `t.size` is 8 or more; resolved sizes are 0 to 7.
pub fn draw(c: &mut Canvas, t: &ResolvedText) {
    let l = layout(&t.text, t.size, t.style, c.dpr);
    let (left, bottom, width, height) = c.up_rect([t.at[0], t.at[1], l.width, l.height]);
    let ink = |rgb| Ink {
        rgb,
        alpha: 1.0,
        blend: BlendMode::SourceOver,
    };
    let contrast = if t.color.luminance() > LIGHT_PLATE {
        BLACK
    } else {
        WHITE
    };
    let (text_ink, halo) = match t.style {
        TextStyle::Label => {
            let mut plate = PathData::default();
            plate.add_rounded_rect(left, bottom, width, height, l.px / 4.0);
            c.main().fill(&plate, ink(t.color), FillRule::Winding);
            (contrast, None)
        }
        TextStyle::Plain => (t.color, Some(contrast)),
    };
    let top = bottom + height - l.pad_y;
    let lines = t.text.split('\n').enumerate();
    for (i, line) in lines {
        let below = f64::from(u32::try_from(i + 1).unwrap_or(u32::MAX)) * l.line_height;
        let origin = V2 {
            x: left + l.pad_x,
            y: top - below,
        };
        let path = text_path(line, Weight::Medium, l.px, origin);
        if let Some(halo) = halo {
            let style = StrokeStyle {
                width: l.px / 5.0,
                cap: LineCap::Round,
                join: LineJoin::Round,
                dash: None,
            };
            c.main().stroke(&path, ink(halo), &style);
        }
        c.main().fill(&path, ink(text_ink), FillRule::Winding);
    }
}
