//! `markshot grid`: a labeled pixel grid over the screenshot, for reading the coordinates a
//! spec needs. Lines and labels are in y-down original-image pixels, like spec coordinates.

use tiny_skia::{BlendMode, FillRule};

use crate::canvas::{Canvas, Ink};
use crate::color::Rgb;
use crate::error::Error;
use crate::geom::{PathData, Seg, V2};
use crate::image_io::{SourceImage, dpr_from_dpi};
use crate::text::{Weight, line_box, text_path};

/// The steps [`default_step`] picks from.
const STEPS: [u32; 6] = [50, 100, 200, 250, 500, 1000];
/// The most intervals a default grid splits the long side into.
const MAX_INTERVALS: u32 = 20;

/// `#FF00FF` at α 0.6.
const LINE: Ink = Ink {
    rgb: Rgb {
        r: 1.0,
        g: 0.0,
        b: 1.0,
    },
    alpha: 0.6,
    blend: BlendMode::SourceOver,
};
/// Black at α 0.6, behind each label.
const PLATE: Ink = Ink {
    rgb: Rgb {
        r: 0.0,
        g: 0.0,
        b: 0.0,
    },
    alpha: 0.6,
    blend: BlendMode::SourceOver,
};
const LABEL: Ink = Ink {
    rgb: Rgb {
        r: 1.0,
        g: 1.0,
        b: 1.0,
    },
    alpha: 1.0,
    blend: BlendMode::SourceOver,
};
/// Label size in px per dpr.
const LABEL_PX: f64 = 12.0;
/// Space between a label and its plate's left and right edges, in px per dpr.
const LABEL_PAD: f64 = 1.0;

/// The smallest of 50, 100, 200, 250, 500 and 1000 that splits the longer side of a `w` × `h`
/// image into at most 20 intervals (`max(w, h) ≤ 20 * step`); 1000 when none does.
#[must_use]
pub fn default_step(w: u32, h: u32) -> u32 {
    let long = w.max(h);
    STEPS
        .into_iter()
        .find(|&step| long <= MAX_INTERVALS * step)
        .unwrap_or(STEPS[STEPS.len() - 1])
}

/// `src` with a 1 px `#FF00FF` α 0.6 line on every column and row that is a positive multiple
/// of `step` (default [`default_step`]), each labeled with its coordinate in Inter Medium
/// 12 * dpr px, white on black α 0.6: columns along the top edge, rows along the left edge. The
/// dpr comes from the PNG's DPI; the output keeps its DPI and ICC profile.
///
/// # Errors
/// [`Error::Spec`] at `--step` when `step` is 0, [`Error::Png`] when encoding fails.
pub fn grid(src: SourceImage, step: Option<u32>) -> Result<Vec<u8>, Error> {
    let step = match step {
        Some(0) => {
            return Err(Error::Spec {
                path: "--step".to_owned(),
                message: "must be at least 1, got 0".to_owned(),
            });
        }
        Some(step) => step,
        None => default_step(src.width, src.height),
    };
    let dpr = dpr_from_dpi(src.dpi);
    let mut c = Canvas::new(src, dpr);
    let (w, h) = (f64::from(c.width), f64::from(c.height));
    let columns = multiples(step, c.width);
    let rows = multiples(step, c.height);
    for &x in &columns {
        let x = f64::from(x);
        c.main()
            .fill(&rect(x, 0.0, 1.0, h), LINE, FillRule::Winding);
    }
    for &y in &rows {
        let y_up = h - f64::from(y) - 1.0;
        c.main()
            .fill(&rect(0.0, y_up, w, 1.0), LINE, FillRule::Winding);
    }
    for &x in &columns {
        label(&mut c, x, |pw, ph| (beside(f64::from(x), pw, w), h - ph));
    }
    for &y in &rows {
        label(&mut c, y, |_, ph| {
            (0.0, h - beside(f64::from(y), ph, h) - ph)
        });
    }
    c.into_png()
}

/// `step`, `2 * step`, … below `end`.
fn multiples(step: u32, end: u32) -> Vec<u32> {
    (1..)
        .map_while(|i: u32| i.checked_mul(step).filter(|&v| v < end))
        .collect()
}

/// The start of a plate `len` long next to the 1 px line at `line`: just after it, pulled back
/// to end at `end` when it would run past it (so it covers the previous label as little as
/// possible).
fn beside(line: f64, len: f64, end: f64) -> f64 {
    (line + 1.0).min(end - len)
}

/// Draws `value`'s label, centered on its plate, whose y-up bottom-left corner `place` gives
/// from the plate's width and height (whole px).
fn label(c: &mut Canvas, value: u32, place: impl FnOnce(f64, f64) -> (f64, f64)) {
    let text = value.to_string();
    let px = LABEL_PX * c.dpr;
    let (advance, height) = line_box(&text, Weight::Medium, px);
    let (pw, ph) = ((advance + 2.0 * LABEL_PAD * c.dpr).ceil(), height.ceil());
    let (left, bottom) = place(pw, ph);
    c.main()
        .fill(&rect(left, bottom, pw, ph), PLATE, FillRule::Winding);
    let origin = V2 {
        x: left + (pw - advance) / 2.0,
        y: bottom + (ph - height) / 2.0,
    };
    let glyphs = text_path(&text, Weight::Medium, px, origin);
    c.main().fill(&glyphs, LABEL, FillRule::Winding);
}

/// The y-up rect with bottom-left corner (`left`, `bottom`).
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_step_keeps_at_most_twenty_intervals() {
        assert_eq!(default_step(1000, 500), 50);
        assert_eq!(default_step(4000, 3000), 200);
        assert_eq!(default_step(1001, 10), 100);
        assert_eq!(default_step(1, 1), 50);
        assert_eq!(
            default_step(10, 20_001),
            1000,
            "the largest step when none fits"
        );
    }

    #[test]
    fn multiples_stop_before_the_end_without_overflow() {
        assert_eq!(multiples(100, 500), [100, 200, 300, 400]);
        assert_eq!(multiples(100, 501), [100, 200, 300, 400, 500]);
        assert_eq!(multiples(7, 7), [] as [u32; 0]);
        assert_eq!(multiples(u32::MAX / 2 + 1, u32::MAX), [u32::MAX / 2 + 1]);
    }

    #[test]
    fn labels_sit_after_their_line_unless_they_would_run_off() {
        assert_eq!(beside(100.0, 30.0, 200.0), 101.0);
        assert_eq!(beside(169.0, 30.0, 200.0), 170.0);
        assert_eq!(beside(185.0, 30.0, 200.0), 170.0, "ends at the edge");
        assert_eq!(beside(199.0, 30.0, 200.0), 170.0, "covers its line");
    }

    #[test]
    fn step_zero_is_an_input_error() {
        let src = SourceImage {
            width: 1,
            height: 1,
            rgba: vec![0; 4],
            dpi: None,
            icc: None,
            color_space: crate::image_io::ColorSpace::Srgb,
        };
        let err = grid(src, Some(0)).unwrap_err();
        assert_eq!(err.to_string(), "--step: must be at least 1, got 0");
    }
}
