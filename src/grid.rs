//! `markshot grid`: a labeled pixel grid over the screenshot, for reading the coordinates a
//! spec needs. Lines and labels are in y-down original-image pixels, like spec coordinates.

use tiny_skia::{BlendMode, FillRule};

use crate::canvas::{Canvas, Ink};
use crate::color::Rgb;
use crate::error::Error;
use crate::geom::{PathData, Seg, V2};
use crate::image_io::{SourceImage, crop_rgba, dpr_from_dpi};
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
/// area starting at column `x0` into at most 20 intervals (`max(w, h) ≤ 20 * step`) and leaves
/// room for its widest column label at `dpr`, the last one's, before the next label; 1000 when
/// none does.
#[must_use]
pub fn default_step(x0: u32, w: u32, h: u32, dpr: f64) -> u32 {
    let long = w.max(h);
    let end = x0.saturating_add(w).saturating_sub(1);
    let fits = |step: u32| {
        let (label_w, label_h) = plate_size(end / step * step, dpr);
        // Each plate starts 1 px after its line, so it ends where the next one starts.
        label_w.max(label_h) <= f64::from(step)
    };
    STEPS
        .into_iter()
        .find(|&step| long <= MAX_INTERVALS * step && fits(step))
        .unwrap_or(STEPS[STEPS.len() - 1])
}

/// `src` with a 1 px `#FF00FF` α 0.6 line on every column and row that is a positive multiple
/// of `step`, each labeled with its coordinate in Inter Medium 12 * dpr px, white on black
/// α 0.6: columns along the top edge, rows along the left edge. With `region`
/// (`[x, y, width, height]` in image pixels, clipped to the image) only that part of the image
/// is drawn, its lines and labels still in whole-image coordinates, so a small area can be read
/// at full size. `step` defaults to [`default_step`] of the drawn part. The dpr comes from the
/// PNG's DPI; the output keeps its DPI and ICC profile.
///
/// # Errors
/// [`Error::Spec`] at `--step` when `step` is 0 and at `--region` when the region has no area
/// or lies outside the image; [`Error::Png`] when encoding fails.
pub fn grid(
    src: SourceImage,
    step: Option<u32>,
    region: Option<[u32; 4]>,
) -> Result<Vec<u8>, Error> {
    let (src, x0, y0) = match region {
        Some(region) => crop(src, region)?,
        None => (src, 0, 0),
    };
    let step = match step {
        Some(0) => {
            return Err(Error::Spec {
                path: "--step".to_owned(),
                message: "must be at least 1, got 0".to_owned(),
            });
        }
        Some(step) => step,
        None => default_step(x0, src.width, src.height, dpr_from_dpi(src.dpi)),
    };
    let dpr = dpr_from_dpi(src.dpi);
    let mut c = Canvas::new(src, dpr);
    let (w, h) = (f64::from(c.width), f64::from(c.height));
    // Image coordinates of the lines, and their columns and rows in the drawn part.
    let columns: Vec<(u32, f64)> = multiples(step, x0, x0 + c.width)
        .into_iter()
        .map(|x| (x, f64::from(x - x0)))
        .collect();
    let rows: Vec<(u32, f64)> = multiples(step, y0, y0 + c.height)
        .into_iter()
        .map(|y| (y, f64::from(y - y0)))
        .collect();
    for &(_, x) in &columns {
        c.main()
            .fill(&rect(x, 0.0, 1.0, h), LINE, FillRule::Winding);
    }
    for &(_, y) in &rows {
        let y_up = h - y - 1.0;
        c.main()
            .fill(&rect(0.0, y_up, w, 1.0), LINE, FillRule::Winding);
    }
    for &(value, x) in &columns {
        label(&mut c, value, |pw, ph| (beside(x, pw, w), h - ph));
    }
    for &(value, y) in &rows {
        label(&mut c, value, |_, ph| (0.0, h - beside(y, ph, h) - ph));
    }
    c.into_png()
}

/// The part of `src` inside `[x, y, width, height]`, clipped to the image, with its top-left
/// corner in image pixels.
///
/// # Errors
/// [`Error::Spec`] at `--region` when the region has no area or lies outside the image.
fn crop(
    src: SourceImage,
    [x, y, width, height]: [u32; 4],
) -> Result<(SourceImage, u32, u32), Error> {
    let error = |message: String| Error::Spec {
        path: "--region".to_owned(),
        message,
    };
    if width == 0 || height == 0 {
        return Err(error(format!(
            "must have a positive width and height, got {width}×{height}"
        )));
    }
    let (x1, y1) = (
        x.saturating_add(width).min(src.width),
        y.saturating_add(height).min(src.height),
    );
    if x >= x1 || y >= y1 {
        return Err(error(format!(
            "outside the {}×{} image",
            src.width, src.height
        )));
    }
    let rgba = crop_rgba(&src.rgba, src.width, [x, y, x1 - x, y1 - y]);
    let cropped = SourceImage {
        width: x1 - x,
        height: y1 - y,
        rgba,
        ..src
    };
    Ok((cropped, x, y))
}

/// The positive multiples of `step` from `start` up to, not including, `end`.
fn multiples(step: u32, start: u32, end: u32) -> Vec<u32> {
    (start.div_ceil(step).max(1)..)
        .map_while(|i| i.checked_mul(step).filter(|&v| v < end))
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
    let (pw, ph) = plate_size(value, c.dpr);
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

/// The width and height in whole px of `value`'s label plate at `dpr`.
fn plate_size(value: u32, dpr: f64) -> (f64, f64) {
    let (advance, height) = line_box(&value.to_string(), Weight::Medium, LABEL_PX * dpr);
    ((advance + 2.0 * LABEL_PAD * dpr).ceil(), height.ceil())
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
        assert_eq!(default_step(0, 1000, 500, 2.0), 50);
        assert_eq!(default_step(0, 4000, 3000, 2.0), 200);
        assert_eq!(default_step(0, 1001, 10, 2.0), 100);
        assert_eq!(default_step(0, 1, 1, 1.0), 50);
        assert_eq!(
            default_step(0, 10, 20_001, 1.0),
            1000,
            "the largest step when none fits"
        );
    }

    #[test]
    fn default_step_leaves_room_for_the_labels() {
        // At dpr 2 a three-digit label's plate (50 px) fits before the next label 50 px on, a
        // four-digit one's (64 px) does not, so a 300 px region ending past x = 1000 gets lines
        // every 100 px.
        assert_eq!(plate_size(950, 2.0).0, 50.0);
        assert_eq!(plate_size(2650, 2.0).0, 64.0);
        assert_eq!(default_step(600, 300, 200, 2.0), 50);
        assert_eq!(default_step(2350, 300, 200, 2.0), 100);
    }

    #[test]
    fn multiples_stop_before_the_end_without_overflow() {
        assert_eq!(multiples(100, 0, 500), [100, 200, 300, 400]);
        assert_eq!(multiples(100, 0, 501), [100, 200, 300, 400, 500]);
        assert_eq!(multiples(7, 0, 7), [] as [u32; 0]);
        assert_eq!(multiples(u32::MAX / 2 + 1, 0, u32::MAX), [u32::MAX / 2 + 1]);
        assert_eq!(
            multiples(100, 200, 450),
            [200, 300, 400],
            "from a region's start on"
        );
        assert_eq!(multiples(100, 201, 300), [] as [u32; 0]);
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
        let err = grid(src, Some(0), None).unwrap_err();
        assert_eq!(err.to_string(), "--step: must be at least 1, got 0");
    }
}
