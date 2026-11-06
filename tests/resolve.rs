//! Resolving a spec: coordinate spaces, defaults, ids, seeds and counter numbers, errors that
//! locate the bad value, off-image and degenerate annotations, and the resolved-spec round trip.

use std::fmt::Debug;

use markshot::color::Rgb;
use markshot::error::Error;
use markshot::resolve::{Resolution, Resolved, resolve};
use markshot::rng::seed_from_id;
use markshot::spec::{ArrowStyle, BlurMode, Cap, CounterStyle, Fill, Spec, SpotShape};
use serde_json::json;

/// `#FF0C01`, the default annotation color.
const RED: Rgb = Rgb {
    r: 1.0,
    g: 12.0 / 255.0,
    b: 1.0 / 255.0,
};

/// `#FFF68D`, the default highlighter color.
const YELLOW: Rgb = Rgb {
    r: 1.0,
    g: 246.0 / 255.0,
    b: 141.0 / 255.0,
};

/// One annotation of every type with only the required fields, inside a 100×100 image.
const EVERY_TYPE: &str = r#"{"annotations": [
    {"type": "arrow", "from": [10, 10], "to": [60, 40]},
    {"type": "rect", "rect": [10, 50, 30, 20]},
    {"type": "oval", "rect": [50, 50, 30, 20]},
    {"type": "blur", "rect": [10, 75, 20, 10]},
    {"type": "highlight", "from": [40, 80], "to": [90, 80]},
    {"type": "spotlight", "rect": [60, 5, 30, 30]},
    {"type": "counter", "center": [80, 60]},
    {"type": "line", "from": [5, 95], "to": [95, 95]}
]}"#;

fn parse(json: &str) -> Spec {
    Spec::from_json(json).expect("spec parses")
}

/// Resolves `json` for a `width` × `height` image of DPR 1, without a `--dpr` override.
fn resolve_json(json: &str, width: u32, height: u32) -> Resolution {
    resolve(&parse(json), width, height, 1.0, None).expect("spec resolves")
}

/// The `(path, message)` of a spec error.
fn spec_error<T: Debug>(result: Result<T, Error>) -> (String, String) {
    match result {
        Err(Error::Spec { path, message }) => (path, message),
        other => panic!("expected a spec error, got {other:?}"),
    }
}

fn ids(r: &Resolution) -> Vec<&str> {
    r.items.iter().map(Resolved::id).collect()
}

fn warned_ids(r: &Resolution) -> Vec<&str> {
    r.warnings.iter().map(|w| w.id.as_str()).collect()
}

#[test]
fn scaled_coords_map_per_axis() {
    let r = resolve_json(
        r#"{"coords": {"space": "scaled", "width": 1000, "height": 500},
            "annotations": [{"type": "line", "from": [100, 50], "to": [400, 300]}]}"#,
        2000,
        1000,
    );
    let Resolved::Line(line) = &r.items[0] else {
        panic!("{:?}", r.items)
    };
    assert_eq!((line.from, line.to), ([200.0, 100.0], [800.0, 600.0]));

    // 0.4 % off the image's aspect ratio: each axis keeps its own factor, and no warning.
    let r = resolve_json(
        r#"{"coords": {"space": "scaled", "width": 1000, "height": 502},
            "annotations": [{"type": "rect", "rect": [100, 251, 50, 50.2]}]}"#,
        2000,
        1000,
    );
    let Resolved::Rect(rect) = &r.items[0] else {
        panic!("{:?}", r.items)
    };
    assert_eq!(rect.rect, [200.0, 500.0, 100.0, 100.0]);
    assert!(r.warnings.is_empty(), "{:?}", r.warnings);
}

#[test]
fn normalized_coords() {
    let r = resolve_json(
        r#"{"coords": {"space": "normalized"}, "annotations": [
            {"type": "counter", "center": [500, 500]},
            {"type": "blur", "rect": [100, 200, 300, 400]},
            {"type": "arrow", "from": [0, 0], "to": [500, 100], "via": [100, 400]},
            {"type": "highlight", "points": [[100, 100], [200, 300]]},
            {"type": "counter", "at": [250, 250], "style": "circle"}
        ]}"#,
        2000,
        1000,
    );
    let [
        Resolved::Counter(counter),
        Resolved::Blur(blur),
        Resolved::Arrow(arrow),
        Resolved::Highlight(highlight),
        Resolved::Counter(placed),
    ] = &r.items[..]
    else {
        panic!("{:?}", r.items)
    };
    assert_eq!(counter.center, [1000.0, 500.0]);
    assert_eq!(blur.rect, [200.0, 200.0, 600.0, 400.0]);
    assert_eq!(arrow.via, Some([200.0, 400.0]));
    assert_eq!(highlight.points, [[200.0, 100.0], [400.0, 300.0]]);
    assert_eq!(placed.center, [500.0, 250.0]);
}

#[test]
fn scaled_aspect_mismatch_warns() {
    let r = resolve_json(
        r#"{"coords": {"space": "scaled", "width": 1000, "height": 600},
            "annotations": [{"type": "line", "from": [300, 300], "to": [600, 450]}]}"#,
        2000,
        1000,
    );
    let Resolved::Line(line) = &r.items[0] else {
        panic!("{:?}", r.items)
    };
    assert_eq!((line.from, line.to), ([600.0, 500.0], [1200.0, 750.0]));
    assert_eq!(warned_ids(&r), ["coords"]);
}

#[test]
fn pixel_coords_round_half_away_from_zero() {
    let r = resolve_json(
        r#"{"annotations": [{"type": "line", "from": [10.4, 10.5], "to": [-0.4, -2.5]}]}"#,
        100,
        100,
    );
    let Resolved::Line(line) = &r.items[0] else {
        panic!("{:?}", r.items)
    };
    assert_eq!((line.from, line.to), ([10.0, 11.0], [0.0, -3.0]));
    assert!(line.to[0].is_sign_positive(), "-0.4 rounds to 0, not -0");
}

#[test]
fn negative_rect_size_is_normalized() {
    let r = resolve_json(
        r#"{"annotations": [{"type": "rect", "rect": [50, 60, -20, -30]}]}"#,
        100,
        100,
    );
    let Resolved::Rect(rect) = &r.items[0] else {
        panic!("{:?}", r.items)
    };
    assert_eq!(rect.rect, [30.0, 30.0, 20.0, 30.0]);
}

#[test]
fn default_ids_and_seeds() {
    let r = resolve_json(
        r#"{"annotations": [
            {"type": "arrow", "from": [0, 0], "to": [50, 50]},
            {"type": "arrow", "from": [0, 0], "to": [50, 50], "seed": 7},
            {"type": "line", "from": [0, 0], "to": [50, 50]}
        ]}"#,
        100,
        100,
    );
    assert_eq!(ids(&r), ["arrow-1", "arrow-2", "line-1"]);
    let [Resolved::Arrow(first), Resolved::Arrow(second), _] = &r.items[..] else {
        panic!("{:?}", r.items)
    };
    assert_eq!(first.seed, seed_from_id("arrow-1"));
    assert_eq!(second.seed, 7);
}

#[test]
fn generated_ids_use_the_type_name() {
    let r = resolve_json(EVERY_TYPE, 100, 100);
    assert_eq!(
        ids(&r),
        [
            "arrow-1",
            "rect-1",
            "oval-1",
            "blur-1",
            "highlight-1",
            "spotlight-1",
            "counter-1",
            "line-1"
        ]
    );
}

#[test]
fn generated_ids_skip_ids_already_taken() {
    let r = resolve_json(
        r#"{"annotations": [
            {"type": "line", "from": [0, 0], "to": [50, 50]},
            {"type": "line", "from": [0, 0], "to": [50, 50], "id": "line-1"},
            {"type": "line", "from": [0, 0], "to": [50, 50]}
        ]}"#,
        100,
        100,
    );
    assert_eq!(ids(&r), ["line-2", "line-1", "line-3"]);
}

#[test]
fn counter_auto_numbering() {
    let numbers = |numbers: [Option<u32>; 4]| -> Vec<u32> {
        let counters: Vec<String> = numbers
            .iter()
            .map(|n| match n {
                Some(n) => format!(r#"{{"type": "counter", "center": [50, 50], "number": {n}}}"#),
                None => r#"{"type": "counter", "center": [50, 50]}"#.to_owned(),
            })
            .collect();
        let json = format!(r#"{{"annotations": [{}]}}"#, counters.join(", "));
        resolve_json(&json, 100, 100)
            .items
            .iter()
            .map(|item| match item {
                Resolved::Counter(counter) => counter.number,
                other => panic!("{other:?}"),
            })
            .collect()
    };
    assert_eq!(numbers([None, None, Some(7), None]), [1, 2, 7, 8]);
    // One more than the largest so far, not than the previous one.
    assert_eq!(numbers([Some(5), None, Some(2), None]), [5, 6, 2, 7]);
}

#[test]
fn defaults() {
    let r = resolve_json(EVERY_TYPE, 100, 100);
    assert_eq!(r.spotlight_dim, 0.214);
    let [
        Resolved::Arrow(arrow),
        Resolved::Rect(rect),
        Resolved::Oval(oval),
        Resolved::Blur(blur),
        Resolved::Highlight(highlight),
        Resolved::Spotlight(spotlight),
        Resolved::Counter(counter),
        Resolved::Line(line),
    ] = &r.items[..]
    else {
        panic!("{:?}", r.items)
    };
    assert_eq!(
        (
            arrow.color,
            arrow.size,
            arrow.style,
            arrow.variant,
            arrow.via
        ),
        (RED, 2, ArrowStyle::Shaped, 0, None)
    );
    assert_eq!((rect.color, rect.size, rect.fill), (RED, 2, Fill::Sketch));
    assert_eq!((oval.color, oval.size, oval.fill), (RED, 2, Fill::Sketch));
    assert_eq!((blur.size, blur.mode), (2, BlurMode::Pixelate));
    assert_eq!(
        (highlight.color, highlight.size, highlight.cap),
        (YELLOW, 2, 3)
    );
    assert_eq!(
        (spotlight.color, spotlight.size, spotlight.shape),
        (RED, 2, SpotShape::Rect)
    );
    assert_eq!(
        (counter.color, counter.size, counter.style, counter.angle),
        (RED, 2, CounterStyle::Bubble, 135.0)
    );
    assert_eq!(
        (
            line.color,
            line.size,
            line.dash,
            line.start_cap,
            line.end_cap
        ),
        (RED, 2, false, Cap::None, Cap::None)
    );
    assert_eq!(
        [arrow.seed, rect.seed, oval.seed, blur.seed],
        ["arrow-1", "rect-1", "oval-1", "blur-1"].map(seed_from_id)
    );
}

#[test]
fn fill_alpha_defaults_follow_the_fill() {
    let r = resolve_json(
        r#"{"annotations": [
            {"type": "rect", "rect": [0, 0, 10, 10], "fill": "semi"},
            {"type": "rect", "rect": [0, 0, 10, 10], "fill": "solid"},
            {"type": "oval", "rect": [0, 0, 10, 10], "fill": "outline"},
            {"type": "oval", "rect": [0, 0, 10, 10], "fill": "solid", "fill_alpha": 0.7}
        ]}"#,
        100,
        100,
    );
    let alphas: Vec<f64> = r
        .items
        .iter()
        .map(|item| match item {
            Resolved::Rect(shape) | Resolved::Oval(shape) => shape.fill_alpha,
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(alphas, [0.4, 1.0, 0.3, 0.7]);
}

#[test]
fn counter_at_places_the_center_like_a_reference_click() {
    let center = |extra: &str, dpr: f64| {
        let json = format!(
            r#"{{"dpr": {dpr}, "annotations": [{{"type": "counter", "at": [100, 100]{extra}}}]}}"#
        );
        let r = resolve(&parse(&json), 300, 300, 1.0, None).expect("spec resolves");
        let Resolved::Counter(counter) = &r.items[0] else {
            panic!("{:?}", r.items)
        };
        counter.center
    };
    // Bubble, size 2 (36 pt, R = 18·dpr), 135°: the center sits 1.65·R up-right of `at`;
    // 1.65 · 18 · cos 45° = 21.0011.
    assert_eq!(center("", 1.0), [121.0, 79.0]);
    assert_eq!(center("", 2.0), [142.0, 58.0]);
    assert_eq!(center(r#", "style": "pin""#, 1.0), [121.0, 79.0]);
    assert_eq!(center(r#", "style": "circle""#, 1.0), [100.0, 100.0]);
    // 0° points the tail right, so the center is 1.65 · 18 = 29.7 left of `at`.
    assert_eq!(center(r#", "angle": 0"#, 1.0), [70.0, 100.0]);
    // Size 0 is 22 pt: 1.65 · 11 · cos 45° = 12.8339.
    assert_eq!(center(r#", "size": 0"#, 1.0), [113.0, 87.0]);
}

#[test]
fn dpr_precedence() {
    let with_dpr = parse(r#"{"dpr": 2, "annotations": []}"#);
    let without_dpr = parse(r#"{"annotations": []}"#);
    let dpr = |spec: &Spec, image_dpr: f64, dpr_override: Option<f64>| {
        resolve(spec, 10, 10, image_dpr, dpr_override)
            .expect("spec resolves")
            .dpr
    };
    assert_eq!(dpr(&with_dpr, 1.0, Some(3.0)), 3.0);
    assert_eq!(dpr(&with_dpr, 1.0, None), 2.0);
    assert_eq!(dpr(&without_dpr, 1.0, None), 1.0);
    assert_eq!(dpr(&without_dpr, 2.0, None), 2.0);
}

#[test]
fn dpr_out_of_range_rejected() {
    let spec = |dpr: f64| parse(&format!(r#"{{"dpr": {dpr}, "annotations": []}}"#));
    let without_dpr = parse(r#"{"annotations": []}"#);
    let (path, message) = spec_error(resolve(&without_dpr, 10, 10, 1.0, Some(0.1)));
    assert_eq!(path, "--dpr");
    assert_eq!(message, "out of range: dpr must be between 0.5 and 4");
    let (path, _) = spec_error(resolve(&spec(10.0), 10, 10, 1.0, None));
    assert_eq!(path, "dpr");
    for dpr in [0.5, 4.0] {
        assert_eq!(resolve(&spec(dpr), 10, 10, 1.0, None).unwrap().dpr, dpr);
        assert_eq!(
            resolve(&without_dpr, 10, 10, 1.0, Some(dpr)).unwrap().dpr,
            dpr
        );
    }
}

#[test]
fn far_coordinates_rejected() {
    let line = |to: &str| {
        format!(r#"{{"annotations": [{{"type": "line", "from": [50, 50], "to": {to}}}]}}"#)
    };
    let (path, message) = spec_error(resolve(&parse(&line("[1e9, 50]")), 100, 100, 1.0, None));
    assert_eq!(path, "annotations[0].to");
    assert_eq!(
        message,
        "out of range: coordinates must be within 16× the image size"
    );
    // 16× the image size is the limit, and that far is only partly outside.
    let r = resolve_json(&line("[1600, 50]"), 100, 100);
    assert_eq!(ids(&r), ["line-1"]);
    assert_eq!(warned_ids(&r), ["line-1"]);
    assert_eq!(
        spec_error(resolve(&parse(&line("[-1601, 50]")), 100, 100, 1.0, None)).0,
        "annotations[0].to"
    );
    // A rect's far corner is bounded too.
    assert_eq!(
        spec_error(resolve(
            &parse(r#"{"annotations": [{"type": "blur", "rect": [10, 10, 1e9, 10]}]}"#),
            100,
            100,
            1.0,
            None
        ))
        .0,
        "annotations[0].rect"
    );
}

#[test]
fn coordinates_are_bounded_after_mapping() {
    let cases = [
        // Normalized: [1e6, 0] is 1e5 pixels across a 100-pixel image.
        r#"{"coords": {"space": "normalized"},
            "annotations": [{"type": "highlight", "points": [[0, 0], [1e6, 0]]}]}"#,
        // Scaled 1×1 → 100×100: [17, 0] is 1700 pixels.
        r#"{"coords": {"space": "scaled", "width": 1, "height": 1},
            "annotations": [{"type": "highlight", "points": [[0, 0], [17, 0]]}]}"#,
    ];
    for json in cases {
        let (path, _) = spec_error(resolve(&parse(json), 100, 100, 1.0, None));
        assert_eq!(path, "annotations[0].points[1]", "{json}");
    }
    // Scaled 1×1 → 100×100: [16, 0] is 1600 pixels, the limit.
    let r = resolve_json(
        r#"{"coords": {"space": "scaled", "width": 1, "height": 1},
            "annotations": [{"type": "highlight", "points": [[0, 0], [16, 0]]}]}"#,
        100,
        100,
    );
    assert_eq!(ids(&r), ["highlight-1"]);
}

#[test]
fn errors_carry_location() {
    let cases = [
        (
            r#"[{"type": "line", "from": [0, 0], "to": [9, 9], "size": 8}]"#,
            "annotations[0].size",
        ),
        (
            r#"[{"type": "counter", "at": [5, 5], "center": [5, 5]}]"#,
            "annotations[0]",
        ),
        (r#"[{"type": "counter"}]"#, "annotations[0]"),
        (
            r#"[{"type": "line", "id": "a", "from": [0, 0], "to": [9, 9]},
                {"type": "line", "id": "a", "from": [0, 0], "to": [9, 9]}]"#,
            "annotations[1].id",
        ),
        (
            r#"[{"type": "rect", "rect": [0, 0, 9, 9], "fill": "semi", "fill_alpha": 0.05}]"#,
            "annotations[0].fill_alpha",
        ),
        (
            r#"[{"type": "line", "from": [0, 0], "to": [9, 9]},
                {"type": "arrow", "from": [0, 0], "to": [9, 9], "color": "red"}]"#,
            "annotations[1].color",
        ),
        (
            r#"[{"type": "arrow", "from": [0, 0], "to": [9, 9], "variant": 4}]"#,
            "annotations[0].variant",
        ),
        (
            r#"[{"type": "highlight", "points": [[0, 0]], "cap": 4}]"#,
            "annotations[0].cap",
        ),
        (r#"[{"type": "line", "from": [0, 0]}]"#, "annotations[0].to"),
        (r#"[{"type": "spotlight"}]"#, "annotations[0].rect"),
        (
            r#"[{"type": "highlight", "points": []}]"#,
            "annotations[0].points",
        ),
        (
            r#"[{"type": "highlight", "from": [0, 0], "to": [9, 9], "points": [[0, 0]]}]"#,
            "annotations[0]",
        ),
    ];
    for (annotations, want) in cases {
        let spec = parse(&format!(r#"{{"annotations": {annotations}}}"#));
        let (path, _) = spec_error(resolve(&spec, 100, 100, 1.0, None));
        assert_eq!(path, want, "{annotations}");
    }
}

#[test]
fn top_level_errors_carry_location() {
    let cases = [
        (r#"{"dpr": 0, "annotations": []}"#, None, "dpr"),
        (r#"{"annotations": []}"#, Some(-1.0), "--dpr"),
        (
            r#"{"spotlight_dim": 1.5, "annotations": []}"#,
            None,
            "spotlight_dim",
        ),
        (
            r#"{"coords": {"space": "scaled", "width": 0, "height": 500}, "annotations": []}"#,
            None,
            "coords.width",
        ),
        (
            r#"{"coords": {"space": "scaled", "width": 500, "height": -1}, "annotations": []}"#,
            None,
            "coords.height",
        ),
    ];
    for (json, dpr_override, want) in cases {
        let (path, _) = spec_error(resolve(&parse(json), 100, 100, 1.0, dpr_override));
        assert_eq!(path, want, "{json}");
    }
}

#[test]
fn unknown_field_rejected() {
    let (path, message) = spec_error(Spec::from_json(
        r##"{"annotations": [{"type": "line", "from": [0, 0], "to": [10, 10], "colour": "#000000"}]}"##,
    ));
    assert_eq!(path, "annotations[0]");
    assert!(message.contains("colour"), "{message}");

    let (_, message) = spec_error(Spec::from_json(r#"{"dpi": 144, "annotations": []}"#));
    assert!(message.contains("dpi"), "{message}");

    let (_, message) = spec_error(Spec::from_json(
        r#"{"coords": {"space": "scaled", "width": 10, "height": 10, "unit": "px"}, "annotations": []}"#,
    ));
    assert!(message.contains("unit"), "{message}");
}

#[test]
fn parse_errors_carry_location() {
    let cases = [
        ("{", "spec"),
        (r#"{"dpr": "2", "annotations": []}"#, "spec"),
        (
            r#"{"annotations": [{"type": "line"}, {"type": "line", "size": "big"}]}"#,
            "annotations[1]",
        ),
        (r#"{"annotations": [{"type": "circle"}]}"#, "annotations[0]"),
    ];
    for (json, want) in cases {
        let (path, _) = spec_error(Spec::from_json(json));
        assert_eq!(path, want, "{json}");
    }
}

#[test]
fn fully_outside_is_skipped_with_warning() {
    let r = resolve_json(
        r#"{"annotations": [
            {"type": "rect", "rect": [200, 200, 10, 10]},
            {"type": "line", "from": [-50, -50], "to": [-10, -1]},
            {"type": "counter", "center": [120, 50]},
            {"type": "line", "from": [10, 10], "to": [90, 90]}
        ]}"#,
        100,
        100,
    );
    assert_eq!(ids(&r), ["line-2"]);
    assert_eq!(warned_ids(&r), ["rect-1", "line-1", "counter-1"]);
}

#[test]
fn partly_outside_is_kept_with_warning() {
    let r = resolve_json(
        r#"{"annotations": [
            {"type": "blur", "rect": [80, 80, 50, 50]},
            {"type": "counter", "center": [110, 50]},
            {"type": "rect", "rect": [0, 0, 100, 100]},
            {"type": "arrow", "from": [10, 10], "to": [50, 10], "via": [30, -40]}
        ]}"#,
        100,
        100,
    );
    assert_eq!(ids(&r), ["blur-1", "counter-1", "rect-1", "arrow-1"]);
    // A counter's extent is its circle (R = 18 at size 2, dpr 1), which reaches into the
    // image; an arrow's includes the `via` it curves through.
    assert_eq!(warned_ids(&r), ["blur-1", "counter-1", "arrow-1"]);
    let Resolved::Blur(blur) = &r.items[0] else {
        panic!("{:?}", r.items)
    };
    assert_eq!(blur.rect, [80.0, 80.0, 50.0, 50.0]);
}

#[test]
fn degenerate_skipped() {
    let r = resolve_json(
        r#"{"annotations": [
            {"type": "line", "from": [10, 10], "to": [10, 10]},
            {"type": "rect", "rect": [10, 10, 0, 20]},
            {"type": "oval", "rect": [10, 10, 20, 0.4]},
            {"type": "blur", "rect": [10, 10, 5, 0]},
            {"type": "spotlight", "rect": [10, 10, 0, 0]},
            {"type": "arrow", "from": [30, 30], "to": [30.2, 29.8]},
            {"type": "highlight", "points": [[50, 50]]}
        ]}"#,
        100,
        100,
    );
    assert_eq!(ids(&r), ["highlight-1"]);
    assert_eq!(
        warned_ids(&r),
        [
            "line-1",
            "rect-1",
            "oval-1",
            "blur-1",
            "spotlight-1",
            "arrow-1"
        ]
    );
}

#[test]
fn resolve_is_idempotent() {
    let spec = parse(
        r#"{"coords": {"space": "scaled", "width": 500, "height": 250}, "annotations": [
            {"type": "arrow", "from": [10, 10], "to": [100, 80], "via": [60, 20], "style": "sketch", "variant": 3},
            {"type": "rect", "rect": [120, 20, -40, 30], "fill": "semi"},
            {"type": "oval", "rect": [10, 100, 50, 40], "fill": "solid", "seed": 99},
            {"type": "blur", "rect": [200, 100, 60, 30], "mode": "erase"},
            {"type": "highlight", "from": [10, 200], "to": [150, 200], "cap": 1},
            {"type": "spotlight", "rect": [300, 50, 80, 60], "shape": "oval", "size": 0},
            {"type": "counter", "at": [400, 150]},
            {"type": "counter", "center": [450, 150], "style": "pin", "number": 5},
            {"type": "line", "from": [10, 240], "to": [490, 240], "size": 7, "dash": true, "end_cap": "arrow"}
        ]}"#,
    );
    let first = resolve(&spec, 1000, 500, 2.0, None)
        .expect("spec resolves")
        .to_spec();
    let second = resolve(&first, 1000, 500, 1.0, None)
        .expect("resolved spec resolves")
        .to_spec();
    assert_eq!(second, first);
    // The CLI prints the resolved spec as JSON; reading that back gives the same spec.
    let text = serde_json::to_string(&first).expect("spec serializes");
    assert_eq!(Spec::from_json(&text).expect("output parses"), first);
}

#[test]
fn to_spec_spells_out_every_field() {
    let spec = serde_json::to_value(resolve_json(EVERY_TYPE, 100, 100).to_spec())
        .expect("spec serializes");
    assert_eq!(spec["coords"], json!({"space": "pixels"}));
    assert_eq!(spec["dpr"], json!(1.0));
    assert_eq!(spec["spotlight_dim"], json!(0.214));
    let keys: Vec<Vec<&str>> = spec["annotations"]
        .as_array()
        .expect("annotations array")
        .iter()
        .map(|annotation| {
            let mut keys: Vec<&str> = annotation
                .as_object()
                .expect("annotation object")
                .keys()
                .map(String::as_str)
                .collect();
            keys.sort_unstable();
            keys
        })
        .collect();
    assert_eq!(
        keys,
        [
            vec![
                "color", "from", "id", "seed", "size", "style", "to", "type", "variant"
            ],
            vec![
                "color",
                "fill",
                "fill_alpha",
                "id",
                "rect",
                "seed",
                "size",
                "type"
            ],
            vec![
                "color",
                "fill",
                "fill_alpha",
                "id",
                "rect",
                "seed",
                "size",
                "type"
            ],
            vec!["id", "mode", "rect", "seed", "size", "type"],
            vec!["cap", "color", "id", "points", "size", "type"],
            vec!["color", "id", "rect", "shape", "size", "type"],
            vec![
                "angle", "center", "color", "id", "number", "size", "style", "type"
            ],
            vec![
                "color",
                "dash",
                "end_cap",
                "from",
                "id",
                "size",
                "start_cap",
                "to",
                "type"
            ],
        ]
    );
    // A highlight given as `from`/`to` comes back as `points`.
    assert_eq!(
        spec["annotations"][4]["points"],
        json!([[40.0, 80.0], [90.0, 80.0]])
    );
}
