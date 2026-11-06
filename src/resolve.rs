//! Turns a [`Spec`] into annotations ready to draw: coordinates mapped to original-image
//! pixels and rounded, defaults, ids, seeds and counter numbers filled in, values validated,
//! and annotations that cannot show (zero size, off the image) dropped with a warning.
//!
//! Resolved coordinates are original-image pixels (origin top-left, y down) rounded to whole
//! numbers.

use std::collections::{HashMap, HashSet};

use serde::Serialize;

use crate::color::{Rgb, parse_hex};
use crate::error::Error;
use crate::math::{cos, sin};
use crate::rng::seed_from_id;
use crate::spec::{
    Annotation, ArrowSpec, ArrowStyle, BlurMode, BlurSpec, Cap, Coords, CounterSpec, CounterStyle,
    Fill, HighlightSpec, LineSpec, Point, Rect, ShapeSpec, Spec, SpotShape, SpotlightSpec,
};

const DEFAULT_COLOR: &str = "#FF0C01";
const DEFAULT_HIGHLIGHT_COLOR: &str = "#FFF68D";
const DEFAULT_SIZE: u8 = 2;
const MAX_SIZE: u8 = 7;
/// The four `shaped` arrow outlines are 0 to 3.
const MAX_ARROW_VARIANT: u8 = 3;
/// Highlighter caps 0 to 3 are those the reference app's toolbar cycles through; 3 is its default.
const MAX_HIGHLIGHT_CAP: u8 = 3;
const DEFAULT_HIGHLIGHT_CAP: u8 = 3;
const DEFAULT_SPOTLIGHT_DIM: f64 = 0.214;
/// the reference app's default tail direction, −3π/4 with y up, in screen degrees.
const DEFAULT_COUNTER_ANGLE: f64 = 135.0;
/// The `fill_alpha` of fills that ignore it: the reference app's `backgroundAlpha` default.
const INERT_FILL_ALPHA: f64 = 0.3;
const SEMI_FILL_ALPHA: f64 = 0.4;
const SOLID_FILL_ALPHA: f64 = 1.0;
/// Scaled coordinates whose aspect ratio is off the image's by more than this get a warning.
const ASPECT_TOLERANCE: f64 = 0.01;
/// The dpr `--dpr` or the spec may give; outside it, sizes and stroke sampling run away.
const DPR_RANGE: std::ops::RangeInclusive<f64> = 0.5..=4.0;
/// Pixel coordinates may be at most this many times the longer image side from the origin.
const COORD_LIMIT_SCALE: f64 = 16.0;
/// Counter diameters in points by size (Counter.md §6 `DIAM`).
const COUNTER_DIAMETERS_PT: [f64; 10] = [
    22.0, 28.0, 36.0, 48.0, 64.0, 84.0, 112.0, 150.0, 200.0, 266.0,
];
/// A counter placed by `at` has its center this many radii from it (Counter.md §6).
const COUNTER_TAIL_OFFSET: f64 = 1.65;

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedArrow {
    pub id: String,
    pub color: Rgb,
    pub size: u8,
    pub seed: u64,
    pub from: Point,
    pub to: Point,
    pub style: ArrowStyle,
    pub variant: u8,
    /// `None` draws a straight arrow.
    pub via: Option<Point>,
}

/// A rect or an oval.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedShape {
    pub id: String,
    pub color: Rgb,
    pub size: u8,
    pub seed: u64,
    /// Width and height are positive.
    pub rect: Rect,
    pub fill: Fill,
    /// Only `semi` and `solid` fills use it.
    pub fill_alpha: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedBlur {
    pub id: String,
    pub size: u8,
    pub seed: u64,
    /// Width and height are positive.
    pub rect: Rect,
    pub mode: BlurMode,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedHighlight {
    pub id: String,
    pub color: Rgb,
    pub size: u8,
    /// At least one point.
    pub points: Vec<Point>,
    pub cap: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedSpotlight {
    pub id: String,
    pub color: Rgb,
    pub size: u8,
    /// Width and height are positive.
    pub rect: Rect,
    pub shape: SpotShape,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedCounter {
    pub id: String,
    pub color: Rgb,
    pub size: u8,
    pub center: Point,
    /// Tail direction in screen degrees: 0 right, 90 down.
    pub angle: f64,
    pub style: CounterStyle,
    pub number: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedLine {
    pub id: String,
    pub color: Rgb,
    pub size: u8,
    pub from: Point,
    pub to: Point,
    pub dash: bool,
    pub start_cap: Cap,
    pub end_cap: Cap,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Resolved {
    Arrow(ResolvedArrow),
    Rect(ResolvedShape),
    Oval(ResolvedShape),
    Blur(ResolvedBlur),
    Highlight(ResolvedHighlight),
    Spotlight(ResolvedSpotlight),
    Counter(ResolvedCounter),
    Line(ResolvedLine),
}

impl Resolved {
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Self::Arrow(a) => &a.id,
            Self::Rect(s) | Self::Oval(s) => &s.id,
            Self::Blur(b) => &b.id,
            Self::Highlight(h) => &h.id,
            Self::Spotlight(s) => &s.id,
            Self::Counter(c) => &c.id,
            Self::Line(l) => &l.id,
        }
    }

    /// The bounds `[x0, y0, x1, y1]` of the geometry (a counter's circle), or why it has none.
    #[expect(
        clippy::float_cmp,
        reason = "coordinates are whole pixels, so equal means the same pixel"
    )]
    fn extent(&self, dpr: f64) -> Result<[f64; 4], &'static str> {
        match self {
            Self::Arrow(ResolvedArrow { from, to, .. })
            | Self::Line(ResolvedLine { from, to, .. })
                if from == to =>
            {
                Err("zero length: `from` and `to` round to the same pixel")
            }
            Self::Arrow(a) => Ok(bounds([&a.from, &a.to].into_iter().chain(&a.via))),
            Self::Line(l) => Ok(bounds([&l.from, &l.to])),
            Self::Rect(ResolvedShape { rect, .. })
            | Self::Oval(ResolvedShape { rect, .. })
            | Self::Blur(ResolvedBlur { rect, .. })
            | Self::Spotlight(ResolvedSpotlight { rect, .. }) => match *rect {
                [_, _, w, h] if w == 0.0 || h == 0.0 => {
                    Err("zero width or height after rounding to pixels")
                }
                [x, y, w, h] => Ok([x, y, x + w, y + h]),
            },
            Self::Highlight(h) => Ok(bounds(&h.points)),
            Self::Counter(c) => {
                let r = counter_radius(c.size, dpr);
                let [x, y] = c.center;
                Ok([x - r, y - r, x + r, y + r])
            }
        }
    }

    /// The annotation with every field given.
    fn to_annotation(&self) -> Annotation {
        match self {
            Self::Arrow(a) => Annotation::Arrow(ArrowSpec {
                id: Some(a.id.clone()),
                color: Some(a.color.to_hex()),
                size: Some(a.size),
                seed: Some(a.seed),
                from: Some(a.from),
                to: Some(a.to),
                style: Some(a.style),
                variant: Some(a.variant),
                via: a.via,
            }),
            Self::Rect(s) => Annotation::Rect(shape_spec(s)),
            Self::Oval(s) => Annotation::Oval(shape_spec(s)),
            Self::Blur(b) => Annotation::Blur(BlurSpec {
                id: Some(b.id.clone()),
                size: Some(b.size),
                seed: Some(b.seed),
                rect: Some(b.rect),
                mode: Some(b.mode),
            }),
            Self::Highlight(h) => Annotation::Highlight(HighlightSpec {
                id: Some(h.id.clone()),
                color: Some(h.color.to_hex()),
                size: Some(h.size),
                from: None,
                to: None,
                points: Some(h.points.clone()),
                cap: Some(h.cap),
            }),
            Self::Spotlight(s) => Annotation::Spotlight(SpotlightSpec {
                id: Some(s.id.clone()),
                color: Some(s.color.to_hex()),
                size: Some(s.size),
                rect: Some(s.rect),
                shape: Some(s.shape),
            }),
            Self::Counter(c) => Annotation::Counter(CounterSpec {
                id: Some(c.id.clone()),
                color: Some(c.color.to_hex()),
                size: Some(c.size),
                at: None,
                center: Some(c.center),
                angle: Some(c.angle),
                style: Some(c.style),
                number: Some(c.number),
            }),
            Self::Line(l) => Annotation::Line(LineSpec {
                id: Some(l.id.clone()),
                color: Some(l.color.to_hex()),
                size: Some(l.size),
                from: Some(l.from),
                to: Some(l.to),
                dash: Some(l.dash),
                start_cap: Some(l.start_cap),
                end_cap: Some(l.end_cap),
            }),
        }
    }
}

fn shape_spec(s: &ResolvedShape) -> ShapeSpec {
    ShapeSpec {
        id: Some(s.id.clone()),
        color: Some(s.color.to_hex()),
        size: Some(s.size),
        seed: Some(s.seed),
        rect: Some(s.rect),
        fill: Some(s.fill),
        fill_alpha: Some(s.fill_alpha),
    }
}

/// Something the agent should know that did not stop the render.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Warning {
    /// The annotation's id, or `coords` for the coordinate space.
    pub id: String,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Resolution {
    pub dpr: f64,
    pub spotlight_dim: f64,
    /// The annotations to draw, in spec order; skipped ones are left out.
    pub items: Vec<Resolved>,
    pub warnings: Vec<Warning>,
}

impl Resolution {
    /// The resolved spec: pixel coordinates, the dpr used, and every field spelled out, so
    /// that feeding it back draws the same image. Only the alternatives not taken stay unset:
    /// counters give `center`, highlights give `points`, and straight arrows have no `via`.
    #[must_use]
    pub fn to_spec(&self) -> Spec {
        Spec {
            coords: Coords::Pixels,
            dpr: Some(self.dpr),
            spotlight_dim: Some(self.spotlight_dim),
            annotations: self.items.iter().map(Resolved::to_annotation).collect(),
        }
    }
}

/// Resolves `spec` for a `width` × `height` image whose DPI gives `image_dpr`. The dpr used
/// is `dpr_override` (`--dpr`), else the spec's `dpr`, else `image_dpr`; the first two must be
/// 0.5 to 4. Each coordinate must map to at most 16× the longer image side in pixels.
///
/// # Errors
/// [`Error::Spec`] naming the offending value, e.g. `annotations[2].size`, `dpr` or `--dpr`.
pub fn resolve(
    spec: &Spec,
    width: u32,
    height: u32,
    image_dpr: f64,
    dpr_override: Option<f64>,
) -> Result<Resolution, Error> {
    let spec_dpr = spec.dpr.map(|dpr| dpr_in_range("dpr", dpr)).transpose()?;
    let dpr_override = dpr_override
        .map(|dpr| dpr_in_range("--dpr", dpr))
        .transpose()?;
    let dpr = dpr_override.or(spec_dpr).unwrap_or(image_dpr);
    let spotlight_dim = spec.spotlight_dim.unwrap_or(DEFAULT_SPOTLIGHT_DIM);
    if !(0.0..=1.0).contains(&spotlight_dim) {
        return Err(Error::Spec {
            path: "spotlight_dim".to_owned(),
            message: format!("must be 0 to 1, got {spotlight_dim}"),
        });
    }
    let mut warnings = Vec::new();
    let coords = CoordMap::new(spec.coords, width, height, &mut warnings)?;
    let ids = assign_ids(&spec.annotations)?;
    let mut max_number = 0;
    let mut items = Vec::with_capacity(spec.annotations.len());
    for (index, (annotation, id)) in spec.annotations.iter().zip(ids).enumerate() {
        let resolver = Resolver {
            index,
            coords: &coords,
            dpr,
        };
        let item = resolver.annotation(annotation, id, &mut max_number)?;
        let warning = |message| Warning {
            id: item.id().to_owned(),
            message,
        };
        match visibility(&item, dpr, width, height) {
            Visibility::Full => items.push(item),
            Visibility::Clipped => {
                warnings.push(warning(format!(
                    "partly outside the {width}×{height} image; drawn clipped"
                )));
                items.push(item);
            }
            Visibility::Hidden(reason) => warnings.push(warning(format!("{reason}; skipped"))),
        }
    }
    Ok(Resolution {
        dpr,
        spotlight_dim,
        items,
        warnings,
    })
}

/// `value` if it is a dpr within [`DPR_RANGE`].
fn dpr_in_range(path: &str, value: f64) -> Result<f64, Error> {
    if DPR_RANGE.contains(&value) {
        Ok(value)
    } else {
        Err(Error::Spec {
            path: path.to_owned(),
            message: "out of range: dpr must be between 0.5 and 4".to_owned(),
        })
    }
}

fn positive(path: &str, value: f64) -> Result<f64, Error> {
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        Err(Error::Spec {
            path: path.to_owned(),
            message: format!("must be a positive number, got {value}"),
        })
    }
}

/// Maps spec coordinates to original-image pixels: `v · numerator / denominator` per axis.
struct CoordMap {
    x: (f64, f64),
    y: (f64, f64),
    /// The largest magnitude a pixel coordinate may have: [`COORD_LIMIT_SCALE`] × the longer
    /// image side. It keeps far-off points from making stroke flattening run unbounded.
    limit: f64,
}

impl CoordMap {
    fn new(
        coords: Coords,
        width: u32,
        height: u32,
        warnings: &mut Vec<Warning>,
    ) -> Result<Self, Error> {
        let (image_w, image_h) = (f64::from(width), f64::from(height));
        let (x, y) = match coords {
            Coords::Pixels => ((1.0, 1.0), (1.0, 1.0)),
            Coords::Normalized => ((image_w, 1000.0), (image_h, 1000.0)),
            Coords::Scaled {
                width: scaled_w,
                height: scaled_h,
            } => {
                positive("coords.width", scaled_w)?;
                positive("coords.height", scaled_h)?;
                if ((image_w / image_h) / (scaled_w / scaled_h) - 1.0).abs() > ASPECT_TOLERANCE {
                    warnings.push(Warning {
                        id: "coords".to_owned(),
                        message: format!(
                            "the scaled size {scaled_w}×{scaled_h} does not have the aspect \
                             ratio of the {width}×{height} image; x and y are scaled separately"
                        ),
                    });
                }
                ((image_w, scaled_w), (image_h, scaled_h))
            }
        };
        Ok(Self {
            x,
            y,
            limit: COORD_LIMIT_SCALE * image_w.max(image_h),
        })
    }

    fn map(&self, [x, y]: Point) -> Point {
        [x * self.x.0 / self.x.1, y * self.y.0 / self.y.1]
    }
}

/// Each annotation's id: its own, or else `<type>-<n>`, `n` being its position among the
/// annotations of its type, moved up past ids already taken.
fn assign_ids(annotations: &[Annotation]) -> Result<Vec<String>, Error> {
    let mut first_use = HashMap::new();
    for (index, annotation) in annotations.iter().enumerate() {
        if let Some(id) = annotation.id()
            && let Some(first) = first_use.insert(id, index)
        {
            return Err(Error::Spec {
                path: format!("annotations[{index}].id"),
                message: format!("duplicate id {id:?}, also used by annotations[{first}]"),
            });
        }
    }
    let mut taken: HashSet<String> = first_use.into_keys().map(str::to_owned).collect();
    let mut counts: HashMap<&str, usize> = HashMap::new();
    let ids = annotations
        .iter()
        .map(|annotation| {
            let kind = annotation.kind();
            let count = counts.entry(kind).or_default();
            *count += 1;
            if let Some(id) = annotation.id() {
                return id.to_owned();
            }
            let mut n = *count;
            loop {
                let id = format!("{kind}-{n}");
                if taken.insert(id.clone()) {
                    return id;
                }
                n += 1;
            }
        })
        .collect();
    Ok(ids)
}

/// Resolves the fields of annotation `index`; errors name `annotations[index].<field>`.
struct Resolver<'a> {
    index: usize,
    coords: &'a CoordMap,
    dpr: f64,
}

impl Resolver<'_> {
    fn annotation(
        &self,
        annotation: &Annotation,
        id: String,
        max_number: &mut u32,
    ) -> Result<Resolved, Error> {
        Ok(match annotation {
            Annotation::Arrow(a) => Resolved::Arrow(self.arrow(a, id)?),
            Annotation::Rect(s) => Resolved::Rect(self.shape(s, id)?),
            Annotation::Oval(s) => Resolved::Oval(self.shape(s, id)?),
            Annotation::Blur(b) => Resolved::Blur(self.blur(b, id)?),
            Annotation::Highlight(h) => Resolved::Highlight(self.highlight(h, id)?),
            Annotation::Spotlight(s) => Resolved::Spotlight(self.spotlight(s, id)?),
            Annotation::Counter(c) => {
                let number = c.number.unwrap_or(max_number.saturating_add(1));
                *max_number = (*max_number).max(number);
                Resolved::Counter(self.counter(c, id, number)?)
            }
            Annotation::Line(l) => Resolved::Line(self.line(l, id)?),
        })
    }

    fn arrow(&self, a: &ArrowSpec, id: String) -> Result<ResolvedArrow, Error> {
        Ok(ResolvedArrow {
            color: self.color(a.color.as_deref(), DEFAULT_COLOR)?,
            size: self.size(a.size)?,
            seed: a.seed.unwrap_or_else(|| seed_from_id(&id)),
            from: self.required_point("from", a.from)?,
            to: self.required_point("to", a.to)?,
            style: a.style.unwrap_or_default(),
            variant: self.step("variant", a.variant, 0, MAX_ARROW_VARIANT)?,
            via: a.via.map(|via| self.point("via", via)).transpose()?,
            id,
        })
    }

    fn shape(&self, s: &ShapeSpec, id: String) -> Result<ResolvedShape, Error> {
        let fill = s.fill.unwrap_or_default();
        let fill_alpha = match s.fill_alpha {
            Some(alpha) if (0.1..=1.0).contains(&alpha) => alpha,
            Some(alpha) => {
                return Err(self.error("fill_alpha", format!("must be 0.1 to 1, got {alpha}")));
            }
            None => match fill {
                Fill::Semi => SEMI_FILL_ALPHA,
                Fill::Solid => SOLID_FILL_ALPHA,
                Fill::Sketch | Fill::Outline => INERT_FILL_ALPHA,
            },
        };
        Ok(ResolvedShape {
            color: self.color(s.color.as_deref(), DEFAULT_COLOR)?,
            size: self.size(s.size)?,
            seed: s.seed.unwrap_or_else(|| seed_from_id(&id)),
            rect: self.rect(s.rect)?,
            fill,
            fill_alpha,
            id,
        })
    }

    fn blur(&self, b: &BlurSpec, id: String) -> Result<ResolvedBlur, Error> {
        Ok(ResolvedBlur {
            size: self.size(b.size)?,
            seed: b.seed.unwrap_or_else(|| seed_from_id(&id)),
            rect: self.rect(b.rect)?,
            mode: b.mode.unwrap_or_default(),
            id,
        })
    }

    fn highlight(&self, h: &HighlightSpec, id: String) -> Result<ResolvedHighlight, Error> {
        let points = match (h.from, h.to, &h.points) {
            (Some(from), Some(to), None) => vec![self.point("from", from)?, self.point("to", to)?],
            (None, None, Some(points)) if points.is_empty() => {
                return Err(self.error("points", "needs at least one point"));
            }
            (None, None, Some(points)) => points
                .iter()
                .enumerate()
                .map(|(i, &point)| self.point(&format!("points[{i}]"), point))
                .collect::<Result<_, _>>()?,
            _ => return Err(self.whole_error("give `from` and `to`, or `points`")),
        };
        Ok(ResolvedHighlight {
            color: self.color(h.color.as_deref(), DEFAULT_HIGHLIGHT_COLOR)?,
            size: self.size(h.size)?,
            points,
            cap: self.step("cap", h.cap, DEFAULT_HIGHLIGHT_CAP, MAX_HIGHLIGHT_CAP)?,
            id,
        })
    }

    fn spotlight(&self, s: &SpotlightSpec, id: String) -> Result<ResolvedSpotlight, Error> {
        Ok(ResolvedSpotlight {
            color: self.color(s.color.as_deref(), DEFAULT_COLOR)?,
            size: self.size(s.size)?,
            rect: self.rect(s.rect)?,
            shape: s.shape.unwrap_or_default(),
            id,
        })
    }

    fn counter(&self, c: &CounterSpec, id: String, number: u32) -> Result<ResolvedCounter, Error> {
        let size = self.size(c.size)?;
        let style = c.style.unwrap_or_default();
        let angle = c.angle.unwrap_or(DEFAULT_COUNTER_ANGLE);
        let center = match (c.at, c.center) {
            (None, Some(center)) => self.point("center", center)?,
            (Some(at), None) => {
                // Like a click in the reference app: the badge sits opposite its tail, 1.65 radii away.
                let offset = match style {
                    CounterStyle::Circle => 0.0,
                    CounterStyle::Bubble | CounterStyle::Pin => {
                        COUNTER_TAIL_OFFSET * counter_radius(size, self.dpr)
                    }
                };
                let [x, y] = self.point("at", at)?;
                let radians = angle.to_radians();
                self.whole_pixels("at", [x - offset * cos(radians), y - offset * sin(radians)])?
            }
            (Some(_), Some(_)) => return Err(self.whole_error("give `at` or `center`, not both")),
            (None, None) => {
                return Err(self.whole_error("missing `at` (the point the tail marks) or `center`"));
            }
        };
        Ok(ResolvedCounter {
            color: self.color(c.color.as_deref(), DEFAULT_COLOR)?,
            size,
            center,
            angle,
            style,
            number,
            id,
        })
    }

    fn line(&self, l: &LineSpec, id: String) -> Result<ResolvedLine, Error> {
        Ok(ResolvedLine {
            color: self.color(l.color.as_deref(), DEFAULT_COLOR)?,
            size: self.size(l.size)?,
            from: self.required_point("from", l.from)?,
            to: self.required_point("to", l.to)?,
            dash: l.dash.unwrap_or(false),
            start_cap: l.start_cap.unwrap_or_default(),
            end_cap: l.end_cap.unwrap_or_default(),
            id,
        })
    }

    fn error(&self, field: &str, message: impl Into<String>) -> Error {
        Error::Spec {
            path: format!("annotations[{}].{field}", self.index),
            message: message.into(),
        }
    }

    fn whole_error(&self, message: &str) -> Error {
        Error::Spec {
            path: format!("annotations[{}]", self.index),
            message: message.to_owned(),
        }
    }

    fn color(&self, hex: Option<&str>, default: &str) -> Result<Rgb, Error> {
        parse_hex(hex.unwrap_or(default)).map_err(|e| match e {
            Error::Spec { message, .. } => self.error("color", message),
            other => other,
        })
    }

    fn size(&self, size: Option<u8>) -> Result<u8, Error> {
        self.step("size", size, DEFAULT_SIZE, MAX_SIZE)
    }

    /// An integer field limited to `0..=max`.
    fn step(&self, field: &str, value: Option<u8>, default: u8, max: u8) -> Result<u8, Error> {
        match value.unwrap_or(default) {
            value if value <= max => Ok(value),
            value => Err(self.error(field, format!("must be 0 to {max}, got {value}"))),
        }
    }

    fn required_point(&self, field: &str, point: Option<Point>) -> Result<Point, Error> {
        let point = point.ok_or_else(|| self.error(field, "missing; expected [x, y]"))?;
        self.point(field, point)
    }

    fn point(&self, field: &str, point: Point) -> Result<Point, Error> {
        self.whole_pixels(field, self.coords.map(point))
    }

    /// `point` in pixels rounded half away from zero, each coordinate within the image's
    /// coordinate limit.
    fn whole_pixels(&self, field: &str, [x, y]: Point) -> Result<Point, Error> {
        let rounded = [round_pixel(x), round_pixel(y)];
        // Written so NaN fails too.
        if rounded.iter().all(|v| v.abs() <= self.coords.limit) {
            Ok(rounded)
        } else {
            Err(self.error(
                field,
                "out of range: coordinates must be within 16× the image size",
            ))
        }
    }

    /// The rect with positive or zero width and height, from its rounded corners.
    fn rect(&self, rect: Option<Rect>) -> Result<Rect, Error> {
        let [x, y, w, h] =
            rect.ok_or_else(|| self.error("rect", "missing; expected [x, y, width, height]"))?;
        let [x0, y0] = self.point("rect", [x, y])?;
        let [x1, y1] = self.point("rect", [x + w, y + h])?;
        Ok([x0.min(x1), y0.min(y1), (x1 - x0).abs(), (y1 - y0).abs()])
    }
}

/// Rounds half away from zero, giving 0 rather than −0.
fn round_pixel(v: f64) -> f64 {
    let rounded = v.round();
    if rounded == 0.0 { 0.0 } else { rounded }
}

/// A counter's radius in image pixels.
fn counter_radius(size: u8, dpr: f64) -> f64 {
    COUNTER_DIAMETERS_PT[usize::from(size)] / 2.0 * dpr
}

/// The bounds `[x0, y0, x1, y1]` of `points`.
fn bounds<'a>(points: impl IntoIterator<Item = &'a Point>) -> [f64; 4] {
    points.into_iter().fold(
        [
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ],
        |[x0, y0, x1, y1], &[x, y]| [x0.min(x), y0.min(y), x1.max(x), y1.max(y)],
    )
}

enum Visibility {
    Full,
    /// Drawn, but partly off the image.
    Clipped,
    /// Not drawn, for the given reason.
    Hidden(String),
}

/// How much of `item` shows on a `width` × `height` image. Its extent touching the image
/// edge counts as inside.
fn visibility(item: &Resolved, dpr: f64, width: u32, height: u32) -> Visibility {
    let (w, h) = (f64::from(width), f64::from(height));
    match item.extent(dpr) {
        Err(reason) => Visibility::Hidden(reason.to_owned()),
        Ok([x0, y0, x1, y1]) if x1 < 0.0 || y1 < 0.0 || x0 > w || y0 > h => {
            Visibility::Hidden(format!("entirely outside the {width}×{height} image"))
        }
        Ok([x0, y0, x1, y1]) if x0 < 0.0 || y0 < 0.0 || x1 > w || y1 > h => Visibility::Clipped,
        Ok(_) => Visibility::Full,
    }
}
