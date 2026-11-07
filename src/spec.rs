//! The annotation spec that agents write: a JSON document listing what to draw on one
//! screenshot. Field docs double as descriptions in the JSON schema (`markshot schema`).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::Error;

/// A point `[x, y]`.
pub type Point = [f64; 2];

/// A rectangle `[x, y, width, height]`.
pub type Rect = [f64; 4];

/// The annotations to draw on one screenshot.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    /// The coordinate space of every point and rectangle.
    #[serde(default)]
    pub coords: Coords,
    /// Device pixel ratio, 0.5 to 4: sizes are in points, drawn at `dpr` pixels per point.
    /// Default: from the PNG's DPI (144 → 2), else 1. The `--dpr` option overrides it.
    pub dpr: Option<f64>,
    /// Opacity, 0 to 1, of the black curtain that spotlights cut holes in. Default 0.214.
    pub spotlight_dim: Option<f64>,
    /// Drawn blurs first, then the spotlight curtain, then everything else in order.
    pub annotations: Vec<Annotation>,
}

/// The coordinate space of the spec. The origin is the top-left corner; y grows downward.
/// Every coordinate must lie within 16× the longer image side, in pixels, of the origin.
// `Pixels` and `Normalized` are empty struct variants, not unit ones, so that serde rejects
// unknown keys next to their `space` as it does for `Scaled`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "space", rename_all = "snake_case", deny_unknown_fields)]
pub enum Coords {
    /// Pixels of the input image.
    Pixels {},
    /// Pixels of the image scaled to `width` × `height`, e.g. the size you viewed it at.
    /// Each axis is mapped separately.
    Scaled {
        /// Width of the scaled image.
        width: f64,
        /// Height of the scaled image.
        height: f64,
    },
    /// 0 to 1000 across the image on each axis.
    Normalized {},
}

impl Default for Coords {
    fn default() -> Self {
        Self::Pixels {}
    }
}

/// One annotation; `type` selects the tool.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Annotation {
    /// An arrow pointing from `from` to `to`.
    Arrow(ArrowSpec),
    /// A rectangle.
    Rect(ShapeSpec),
    /// the reference app's oval inside `rect`: a squircle-like rounded shape, fuller than an ellipse.
    Oval(ShapeSpec),
    /// Hides what is under `rect`.
    Blur(BlurSpec),
    /// A highlighter stroke from `from` to `to`, or through `points`.
    Highlight(HighlightSpec),
    /// Dims the image except inside `rect`.
    Spotlight(SpotlightSpec),
    /// A numbered badge, placed by `at` (the point its tail marks, like a click in the reference app) or
    /// by `center`.
    Counter(CounterSpec),
    /// A straight line.
    Line(LineSpec),
}

impl Annotation {
    /// The `type` tag; generated ids start with it.
    pub(crate) fn kind(&self) -> &'static str {
        match self {
            Self::Arrow(_) => "arrow",
            Self::Rect(_) => "rect",
            Self::Oval(_) => "oval",
            Self::Blur(_) => "blur",
            Self::Highlight(_) => "highlight",
            Self::Spotlight(_) => "spotlight",
            Self::Counter(_) => "counter",
            Self::Line(_) => "line",
        }
    }

    /// The id given in the spec, if any.
    pub(crate) fn id(&self) -> Option<&str> {
        match self {
            Self::Arrow(a) => a.id.as_deref(),
            Self::Rect(s) | Self::Oval(s) => s.id.as_deref(),
            Self::Blur(b) => b.id.as_deref(),
            Self::Highlight(h) => h.id.as_deref(),
            Self::Spotlight(s) => s.id.as_deref(),
            Self::Counter(c) => c.id.as_deref(),
            Self::Line(l) => l.id.as_deref(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ArrowSpec {
    /// Unique name. Default `arrow-<n>`.
    pub id: Option<String>,
    /// `#RRGGBB`. Default `#FF0C01`.
    pub color: Option<String>,
    /// Thickness step, 0 to 7. Default 2.
    pub size: Option<u8>,
    /// Seed of the hand-drawn randomness of `sketch`, 0 to 2^53 − 1. Default: derived from `id`.
    pub seed: Option<u64>,
    /// Tail `[x, y]`.
    #[schemars(required)]
    pub from: Option<Point>,
    /// Tip `[x, y]`.
    #[schemars(required)]
    pub to: Option<Point>,
    /// Default `shaped`.
    pub style: Option<ArrowStyle>,
    /// Outline of the `shaped` style, 0 to 3. Default 0.
    pub variant: Option<u8>,
    /// A point `[x, y]` the arrow curves through. Default: straight.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub via: Option<Point>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ArrowStyle {
    /// A filled arrow with a tapered shaft.
    #[default]
    Shaped,
    /// A hand-drawn arrow.
    Sketch,
    /// A line with an arrowhead.
    Line,
    /// A line with arrowheads at both ends.
    Double,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ShapeSpec {
    /// Unique name. Default `<type>-<n>`.
    pub id: Option<String>,
    /// `#RRGGBB`. Default `#FF0C01`.
    pub color: Option<String>,
    /// Border thickness step, 0 to 7. Default 2.
    pub size: Option<u8>,
    /// Seed of the hand-drawn randomness of `sketch`, 0 to 2^53 − 1. Default: derived from `id`.
    pub seed: Option<u64>,
    /// `[x, y, width, height]`.
    #[schemars(required)]
    pub rect: Option<Rect>,
    /// Default `sketch`.
    pub fill: Option<Fill>,
    /// Opacity of the `semi` and `solid` fill, 0.1 to 1. Default 0.4 for `semi`, 1 for `solid`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill_alpha: Option<f64>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Fill {
    /// A hand-drawn border.
    #[default]
    Sketch,
    /// A border.
    Outline,
    /// A border filled with the color at `fill_alpha`.
    Semi,
    /// Filled with the color.
    Solid,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BlurSpec {
    /// Unique name. Default `blur-<n>`.
    pub id: Option<String>,
    /// Pixelation block size step, 0 to 7. Default 2.
    pub size: Option<u8>,
    /// Seed of the pixelation blocks, 0 to 2^53 − 1. Default: derived from `id`.
    pub seed: Option<u64>,
    /// `[x, y, width, height]`.
    #[schemars(required)]
    pub rect: Option<Rect>,
    /// Default `pixelate`.
    pub mode: Option<BlurMode>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BlurMode {
    /// Mosaic blocks.
    #[default]
    Pixelate,
    /// Paints over the area with the colors along its border.
    Erase,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HighlightSpec {
    /// Unique name. Default `highlight-<n>`.
    pub id: Option<String>,
    /// `#RRGGBB`. Default `#FFF68D`.
    pub color: Option<String>,
    /// Stroke width step, 0 to 7. Default 2.
    pub size: Option<u8>,
    /// Start `[x, y]`, with `to`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<Point>,
    /// End `[x, y]`, with `from`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<Point>,
    /// `[[x, y], ...]`, instead of `from` and `to`. A single point draws a dot.
    pub points: Option<Vec<Point>>,
    /// End shape: 0 round, 1 flat round, 2 notched, 3 double lobe. Default 3.
    pub cap: Option<u8>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SpotlightSpec {
    /// Unique name. Default `spotlight-<n>`.
    pub id: Option<String>,
    /// Border `#RRGGBB`. Default `#FF0C01`.
    pub color: Option<String>,
    /// Border thickness step, 0 to 7; 0 draws no border. Default 2.
    pub size: Option<u8>,
    /// The area left undimmed, `[x, y, width, height]`.
    #[schemars(required)]
    pub rect: Option<Rect>,
    /// Default `rect`.
    pub shape: Option<SpotShape>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SpotShape {
    /// A rounded rectangle.
    #[default]
    Rect,
    /// the reference app's oval: a squircle-like rounded shape, fuller than an ellipse.
    Oval,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CounterSpec {
    /// Unique name. Default `counter-<n>`.
    pub id: Option<String>,
    /// `#RRGGBB`. Default `#FF0C01`.
    pub color: Option<String>,
    /// Diameter step, 0 to 7. Default 2.
    pub size: Option<u8>,
    /// The point `[x, y]` the tail marks; the badge sits 1.65 radii away, opposite the tail.
    /// A `circle` has no tail and is centered on `at`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at: Option<Point>,
    /// The badge center `[x, y]`, instead of `at`.
    pub center: Option<Point>,
    /// Tail direction in degrees: 0 right, 90 down. Default 135 (down-left).
    pub angle: Option<f64>,
    /// Default `bubble`.
    pub style: Option<CounterStyle>,
    /// Default: one more than the largest number of the counters before it.
    pub number: Option<u32>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CounterStyle {
    /// No tail.
    Circle,
    /// A rounded tail.
    #[default]
    Bubble,
    /// A pointed tail.
    Pin,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LineSpec {
    /// Unique name. Default `line-<n>`.
    pub id: Option<String>,
    /// `#RRGGBB`. Default `#FF0C01`.
    pub color: Option<String>,
    /// Thickness step, 0 to 7. Default 2.
    pub size: Option<u8>,
    /// Start `[x, y]`.
    #[schemars(required)]
    pub from: Option<Point>,
    /// End `[x, y]`.
    #[schemars(required)]
    pub to: Option<Point>,
    /// Dashed. Default false.
    pub dash: Option<bool>,
    /// Default `none`.
    pub start_cap: Option<Cap>,
    /// Default `none`.
    pub end_cap: Option<Cap>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cap {
    /// A plain end.
    #[default]
    None,
    /// An arrowhead.
    Arrow,
    /// A round dot.
    Bullet,
}

impl Spec {
    /// Parses a spec from JSON. Each annotation is parsed on its own so that an error names it.
    ///
    /// # Errors
    /// [`Error::Spec`] at `annotations[i]` when annotation `i` does not parse (unknown field,
    /// wrong type, unknown `type`), or at `spec` when the text is not JSON or the top level
    /// does not parse.
    pub fn from_json(text: &str) -> Result<Self, Error> {
        let mut root: Value = serde_json::from_str(text).map_err(|e| invalid("spec", &e))?;
        let items = match root.get_mut("annotations") {
            Some(Value::Array(items)) => std::mem::take(items),
            _ => Vec::new(),
        };
        let mut spec: Self = serde_json::from_value(root).map_err(|e| invalid("spec", &e))?;
        spec.annotations = items
            .into_iter()
            .enumerate()
            .map(|(i, item)| {
                serde_json::from_value(item).map_err(|e| invalid(&format!("annotations[{i}]"), &e))
            })
            .collect::<Result<_, _>>()?;
        Ok(spec)
    }
}

fn invalid(path: &str, e: &serde_json::Error) -> Error {
    Error::Spec {
        path: path.to_owned(),
        message: e.to_string(),
    }
}
