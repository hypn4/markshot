//! Resolving a spec: coordinate spaces, defaults, ids, seeds and counter numbers, errors that
//! locate the bad value, off-image and degenerate annotations, and the resolved-spec round trip.

use std::fmt::Debug;

use markshot::color::Rgb;
use markshot::error::Error;
use markshot::resolve::{Resolution, Resolved, Warning, resolve};
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
fn seeds_are_limited_to_53_bits() {
    // A seed must survive JSON tools that read numbers as doubles: at most 2^53 − 1.
    for (kind, geometry) in [
        ("arrow", r#""from": [0, 0], "to": [50, 50]"#),
        ("rect", r#""rect": [0, 0, 50, 50]"#),
        ("oval", r#""rect": [0, 0, 50, 50]"#),
        ("blur", r#""rect": [0, 0, 50, 50]"#),
    ] {
        let json = |seed: u64| {
            format!(r#"{{"annotations": [{{"type": "{kind}", {geometry}, "seed": {seed}}}]}}"#)
        };
        assert_eq!(
            resolve_json(&json(9_007_199_254_740_991), 100, 100)
                .items
                .len(),
            1
        );
        let (path, message) = spec_error(resolve(
            &parse(&json(9_007_199_254_740_992)),
            100,
            100,
            1.0,
            None,
        ));
        assert_eq!(path, "annotations[0].seed", "{kind}");
        assert!(message.starts_with("out of range"), "{kind}: {message}");
    }
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
fn fill_alpha_is_kept_only_when_given() {
    // The fill's default opacity is left to the drawing, so that a resolved spec whose `fill`
    // is changed takes the new fill's default.
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
    let alphas: Vec<Option<f64>> = r
        .items
        .iter()
        .map(|item| match item {
            Resolved::Rect(shape) | Resolved::Oval(shape) => shape.fill_alpha,
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(alphas, [None, None, None, Some(0.7)]);
    let spec = serde_json::to_value(r.to_spec()).expect("spec serializes");
    let given: Vec<Option<&serde_json::Value>> = spec["annotations"]
        .as_array()
        .expect("annotations array")
        .iter()
        .map(|annotation| annotation.get("fill_alpha"))
        .collect();
    assert_eq!(given, [None, None, None, Some(&json!(0.7))]);
}

#[test]
fn shape_spotlight_is_listed_only_when_true() {
    // A frame that dims the rest keeps `"spotlight": true` in the resolved spec, so feeding it
    // back dims again; `false` and a missing field resolve alike and stay out.
    let r = resolve_json(
        r#"{"annotations": [
            {"type": "rect", "rect": [0, 0, 10, 10], "spotlight": true},
            {"type": "oval", "rect": [0, 0, 10, 10], "spotlight": false},
            {"type": "oval", "rect": [0, 0, 10, 10]}
        ]}"#,
        100,
        100,
    );
    let flags: Vec<bool> = r
        .items
        .iter()
        .map(|item| match item {
            Resolved::Rect(shape) | Resolved::Oval(shape) => shape.spotlight,
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(flags, [true, false, false]);
    let spec = serde_json::to_value(r.to_spec()).expect("spec serializes");
    let listed: Vec<Option<&serde_json::Value>> = spec["annotations"]
        .as_array()
        .expect("annotations array")
        .iter()
        .map(|annotation| annotation.get("spotlight"))
        .collect();
    assert_eq!(listed, [Some(&json!(true)), None, None]);
}

#[test]
fn counter_at_places_the_center_opposite_the_tail() {
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
    // Bubble, size 2 (36 pt, R = 18 * dpr), 135°: the center sits 1.65 * R up-right of `at`;
    // 1.65 * 18 * cos 45° = 21.0011.
    assert_eq!(center("", 1.0), [121.0, 79.0]);
    assert_eq!(center("", 2.0), [142.0, 58.0]);
    assert_eq!(center(r#", "style": "pin""#, 1.0), [121.0, 79.0]);
    assert_eq!(center(r#", "style": "circle""#, 1.0), [100.0, 100.0]);
    // 0° points the tail right, so the center is 1.65 * 18 = 29.7 left of `at`.
    assert_eq!(center(r#", "angle": 0"#, 1.0), [70.0, 100.0]);
    // Size 0 is 22 pt: 1.65 * 11 * cos 45° = 12.8339.
    assert_eq!(center(r#", "size": 0"#, 1.0), [113.0, 87.0]);
}

#[test]
fn counter_bounds_check_its_at_not_its_center() {
    // A default counter at the corner of a tiny image: its center lands 1.65 radii up and to
    // the right of `at`, beyond 16× the image size, but only the `at` given is bounded. The
    // badge then sits above the image and is skipped.
    for (dpr, size) in [(1, 1), (2, 2)] {
        let json =
            format!(r#"{{"dpr": {dpr}, "annotations": [{{"type": "counter", "at": [0, 0]}}]}}"#);
        let r = resolve(&parse(&json), size, size, 1.0, None).expect("spec resolves");
        assert!(r.items.is_empty(), "{:?}", r.items);
        assert_eq!(
            r.warnings
                .iter()
                .map(|w| (w.id.as_str(), w.message.as_str()))
                .collect::<Vec<_>>(),
            [(
                "counter-1",
                format!("entirely outside the {size}×{size} image; skipped").as_str()
            )]
        );
    }
    let (path, _) = spec_error(resolve(
        &parse(r#"{"annotations": [{"type": "counter", "at": [17, 0]}]}"#),
        1,
        1,
        1.0,
        None,
    ));
    assert_eq!(path, "annotations[0].at");
    // A center past the bound that `at` put on the image (R 18; the tail points left, so the
    // center is 29.7 right of `at`, at x = 18) still resolves when the resolved spec gives it,
    // so that feeding the spec back draws the same.
    let spec = parse(r#"{"annotations": [{"type": "counter", "at": [-12, 0], "angle": 180}]}"#);
    let first = resolve(&spec, 1, 1, 1.0, None).expect("spec resolves");
    let Resolved::Counter(counter) = &first.items[0] else {
        panic!("{:?}", first.items)
    };
    assert_eq!(counter.center, [18.0, 0.0]);
    let second = resolve(&first.to_spec(), 1, 1, 1.0, None).expect("resolved spec resolves");
    assert_eq!(second.to_spec(), first.to_spec());
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
fn coords_reject_unknown_keys() {
    // Every coordinate space takes only its own keys, as the schema's `additionalProperties:
    // false` says, so a misplaced size is an error rather than silently ignored.
    for (coords, key) in [
        (r#"{"space": "pixels", "width": 801}"#, "width"),
        (r#"{"space": "normalized", "bogus": 1}"#, "bogus"),
        (
            r#"{"space": "scaled", "width": 10, "height": 10, "bogus": 1}"#,
            "bogus",
        ),
    ] {
        let json = format!(r#"{{"coords": {coords}, "annotations": []}}"#);
        let (path, message) = spec_error(Spec::from_json(&json));
        assert_eq!(path, "spec", "{coords}");
        assert!(message.contains(key), "{coords}: {message}");
    }
    // Without extra keys, each parses.
    for space in ["pixels", "normalized"] {
        let json = format!(r#"{{"coords": {{"space": "{space}"}}, "annotations": []}}"#);
        resolve_json(&json, 100, 100);
    }
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
fn blur_only_touching_the_image_is_skipped() {
    // A blur covers only its rect, so one that touches the image's edge from outside draws
    // nothing and is skipped. A rect there is still drawn: its border straddles the edge.
    let r = resolve_json(
        r#"{"annotations": [
            {"type": "blur", "rect": [100, 10, 20, 20]},
            {"type": "blur", "rect": [10, -20, 20, 20]},
            {"type": "rect", "rect": [100, 10, 20, 20]}
        ]}"#,
        100,
        100,
    );
    assert_eq!(ids(&r), ["rect-1"]);
    let warnings: Vec<(&str, &str)> = r
        .warnings
        .iter()
        .map(|w| (w.id.as_str(), w.message.as_str()))
        .collect();
    assert_eq!(
        warnings,
        [
            ("blur-1", "entirely outside the 100×100 image; skipped"),
            ("blur-2", "entirely outside the 100×100 image; skipped"),
            ("rect-1", "partly outside the 100×100 image; drawn clipped"),
        ]
    );
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
    // A default `fill_alpha` is left out (see `fill_alpha_is_kept_only_when_given`).
    assert_eq!(
        keys,
        [
            vec![
                "color", "from", "id", "seed", "size", "style", "to", "type", "variant"
            ],
            vec!["color", "fill", "id", "rect", "seed", "size", "type"],
            vec!["color", "fill", "id", "rect", "seed", "size", "type"],
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

#[test]
fn crop_maps_like_a_rect_and_is_clipped_to_the_image() {
    // `crop` is in the spec's coordinate space: on a 400×300 view of the 800×600 image,
    // [50, 40, 100, 80] is [100, 80, 200, 160] in pixels. One running off the image keeps the
    // part inside, and the resolved spec lists the pixel crop.
    let r = resolve_json(
        r#"{"coords": {"space": "scaled", "width": 400, "height": 300},
            "crop": [50, 40, 100, 80], "annotations": []}"#,
        800,
        600,
    );
    assert_eq!(r.crop, Some([100.0, 80.0, 200.0, 160.0]));
    assert_eq!(r.to_spec().crop, Some([100.0, 80.0, 200.0, 160.0]));
    let r = resolve_json(
        r#"{"crop": [700, 500, 300, 300], "annotations": []}"#,
        800,
        600,
    );
    assert_eq!(r.crop, Some([700.0, 500.0, 100.0, 100.0]));
    assert_eq!(resolve_json(r#"{"annotations": []}"#, 800, 600).crop, None);
}

#[test]
fn crop_without_area_or_off_the_image_is_an_error() {
    for (crop, message) in [
        ("[10, 10, 0, 50]", "must have a positive width and height"),
        ("[900, 0, 50, 50]", "outside the 800×600 image"),
        ("[-100, -100, 100, 100]", "outside the 800×600 image"),
    ] {
        let json = format!(r#"{{"crop": {crop}, "annotations": []}}"#);
        let (path, got) = spec_error(resolve(&parse(&json), 800, 600, 1.0, None));
        assert_eq!(path, "crop", "{crop}");
        assert!(got.starts_with(message), "{crop}: {got}");
    }
}

#[test]
fn an_annotation_outside_the_crop_warns() {
    // Drawn on the image, but none of it reaches the cropped output.
    let r = resolve_json(
        r#"{"crop": [0, 0, 100, 100], "annotations": [
            {"type": "rect", "rect": [20, 20, 40, 40]},
            {"type": "rect", "rect": [300, 300, 40, 40]}
        ]}"#,
        800,
        600,
    );
    assert_eq!(r.items.len(), 2);
    assert_eq!(
        r.warnings,
        [Warning {
            id: "rect-2".to_owned(),
            message: "entirely outside the crop; drawn but not in the output".to_owned(),
        }]
    );
}

/// The `angle` and `center` the first annotation, a counter, resolves to.
fn counter_placement(json: &str, width: u32, height: u32) -> (f64, [f64; 2]) {
    let r = resolve_json(json, width, height);
    let Resolved::Counter(counter) = &r.items[0] else {
        panic!("{:?}", r.items[0]);
    };
    (counter.angle, counter.center)
}

#[test]
fn auto_angle_keeps_the_default_where_it_fits() {
    // Nothing nearby on a blank 400×300 image: the default 135° (badge up-right of `at`).
    let (angle, _) = counter_placement(
        r#"{"annotations": [{"type": "counter", "at": [200, 150], "angle": "auto"}]}"#,
        400,
        300,
    );
    assert_eq!(angle, 135.0);
}

#[test]
fn auto_angle_keeps_the_badge_off_other_annotations_and_inside_the_image() {
    // A box right of `at` and above it covers where 135° and its neighbours would put the
    // badge (up-right): the badge goes elsewhere and its circle (radius 18 at size 2, dpr 1)
    // stays clear of the box. Near the bottom-left corner, it stays inside the image.
    let json = r#"{"annotations": [
        {"type": "counter", "at": [200, 150], "angle": "auto"},
        {"type": "rect", "rect": [200, 40, 160, 112], "fill": "outline"}
    ]}"#;
    let (angle, [x, y]) = counter_placement(json, 400, 300);
    assert_ne!(angle, 135.0);
    let near_x = x.clamp(200.0, 360.0);
    let near_y = y.clamp(40.0, 152.0);
    let (dx, dy) = (x - near_x, y - near_y);
    assert!(dx * dx + dy * dy >= 18.0 * 18.0, "{angle}: ({x}, {y})");
    let (angle, [x, y]) = counter_placement(
        r#"{"annotations": [{"type": "counter", "at": [5, 295], "angle": "auto"}]}"#,
        400,
        300,
    );
    assert!(x >= 18.0 && y <= 282.0, "{angle}: ({x}, {y})");
}

#[test]
fn auto_angle_resolves_to_a_number_and_needs_a_tail() {
    let r = resolve_json(
        r#"{"annotations": [{"type": "counter", "at": [200, 150], "angle": "auto"}]}"#,
        400,
        300,
    );
    let spec = serde_json::to_value(r.to_spec()).expect("spec serializes");
    assert_eq!(spec["annotations"][0]["angle"], json!(135.0));
    for counter in [
        r#"{"type": "counter", "center": [200, 150], "angle": "auto"}"#,
        r#"{"type": "counter", "at": [200, 150], "style": "circle", "angle": "auto"}"#,
    ] {
        let json = format!(r#"{{"annotations": [{counter}]}}"#);
        let (path, message) = spec_error(resolve(&parse(&json), 400, 300, 1.0, None));
        assert_eq!(path, "annotations[0].angle", "{counter}");
        assert!(message.contains("`at`"), "{counter}: {message}");
    }
}

#[test]
fn text_needs_characters_the_font_has() {
    // Inter covers Latin, Greek and Cyrillic; anything else would draw as empty boxes, so it is
    // an input error naming the first such character. Empty text has nothing to draw.
    for (text, message) in [
        ("설정", "U+C124"),
        ("Tab\there", "U+0009"),
        ("", "must not be empty"),
    ] {
        let json = format!(
            r#"{{"annotations": [{{"type": "text", "at": [10, 10], "text": {}}}]}}"#,
            json!(text)
        );
        let (path, got) = spec_error(resolve(&parse(&json), 400, 300, 1.0, None));
        assert_eq!(path, "annotations[0].text", "{text:?}");
        assert!(got.contains(message), "{text:?}: {got}");
    }
}

#[test]
fn text_resolves_with_its_defaults_and_its_box_bounds_it() {
    // Defaults: label style, size 2, the default red. Far right, the label runs off the
    // image and is drawn clipped, with a warning, like any annotation.
    let r = resolve_json(
        r#"{"annotations": [
            {"type": "text", "at": [10, 10], "text": "Select Allow"},
            {"type": "text", "at": [390, 10], "text": "Then Save"}
        ]}"#,
        400,
        300,
    );
    let spec = serde_json::to_value(r.to_spec()).expect("spec serializes");
    let first = &spec["annotations"][0];
    assert_eq!(
        (&first["style"], &first["size"], &first["color"]),
        (&json!("label"), &json!(2), &json!("#FF0C01"))
    );
    assert_eq!(
        r.warnings,
        [Warning {
            id: "text-2".to_owned(),
            message: "partly outside the 400×300 image; drawn clipped".to_owned(),
        }]
    );
}

#[test]
fn auto_angle_keeps_the_badge_inside_the_crop() {
    // With a crop, "outside" means outside the crop: a counter marking a point 10 px inside the
    // crop's right edge, where the default 135° would put its badge (radius 18) past that edge
    // though still on the image, gets a badge that stays within the crop.
    let json = r#"{"crop": [0, 0, 300, 300], "annotations": [
        {"type": "counter", "at": [290, 150], "angle": "auto"}
    ]}"#;
    let (angle, [x, _]) = counter_placement(json, 400, 300);
    assert!(x + 18.0 <= 300.0, "{angle}: badge centre x {x}");
}

#[test]
fn counter_auto_angle_warns_when_the_crop_cannot_hold_the_badge() {
    // The 72 px body plus two 6 px rings is 84 px tall, but the crop is only 50 px tall.
    let r = resolve_json(
        r#"{"dpr": 2, "crop": [100, 100, 200, 50], "annotations": [
            {"type": "counter", "at": [200, 125], "angle": "auto", "size": 2,
             "ring": true}
        ]}"#,
        400,
        300,
    );
    assert_eq!(warned_ids(&r), ["counter-1"]);
    let Resolved::Counter(counter) = &r.items[0] else {
        panic!("{:?}", r.items);
    };
    assert_eq!(
        counter.size, 2,
        "a failed fit must not change an explicit size"
    );
    assert!(counter.auto_at.is_none());
}

#[test]
fn counter_auto_angle_warns_when_all_directions_overlap_an_annotation() {
    // Every candidate center is 29.7 px from (200, 150). Its 18 px body is within this box.
    let r = resolve_json(
        r#"{"annotations": [
            {"type": "counter", "at": [200, 150], "angle": "auto"},
            {"type": "rect", "rect": [150, 100, 100, 100], "fill": "solid"}
        ]}"#,
        400,
        300,
    );
    assert_eq!(warned_ids(&r), ["counter-1"]);
}

#[test]
fn counter_auto_angle_warns_when_its_tail_ring_cannot_fit_the_crop() {
    // A left-side body can fit, but its rightward tail and 3 px ring reach past crop x = 200.
    let r = resolve_json(
        r#"{"crop": [0, 0, 200, 300], "annotations": [
            {"type": "counter", "at": [202, 150], "angle": "auto", "ring": true}
        ]}"#,
        400,
        300,
    );
    assert_eq!(warned_ids(&r), ["counter-1"]);
}

#[test]
fn counter_auto_angle_accepts_a_ring_touching_the_crop_edges() {
    // A 36 px body plus two 3 px rings fits a 42 px crop exactly when the tail is horizontal.
    let r = resolve_json(
        r#"{"crop": [0, 0, 400, 42], "annotations": [
            {"type": "counter", "at": [200, 21], "angle": "auto", "ring": true}
        ]}"#,
        400,
        300,
    );
    assert_eq!(warned_ids(&r), Vec::<&str>::new());
}

#[test]
fn oval_around_resolves_to_a_rect_that_contains_its_target() {
    // `around` is the target's box; the resolved spec gives the oval's own `rect`, larger than
    // the box, and no `around`, so feeding it back draws the same oval.
    let r = resolve_json(
        r#"{"annotations": [{"type": "oval", "around": [300, 200, 240, 42]}]}"#,
        800,
        600,
    );
    let Resolved::Oval(oval) = &r.items[0] else {
        panic!("{:?}", r.items[0]);
    };
    let [left, top, width, height] = oval.rect;
    assert!(
        left < 300.0 && top < 200.0 && left + width > 540.0 && top + height > 242.0,
        "{:?}",
        oval.rect
    );
    let spec = serde_json::to_value(r.to_spec()).expect("spec serializes");
    assert_eq!(spec["annotations"][0]["around"], json!(null));
    assert_eq!(spec["annotations"][0]["rect"], json!(oval.rect));
}

#[test]
fn around_is_for_ovals_and_excludes_rect() {
    for (shape, message) in [
        (
            r#"{"type": "rect", "around": [10, 10, 50, 20]}"#,
            "`around` is for ovals",
        ),
        (
            r#"{"type": "oval", "around": [10, 10, 50, 20], "rect": [0, 0, 80, 40]}"#,
            "not both",
        ),
    ] {
        let json = format!(r#"{{"annotations": [{shape}]}}"#);
        let (path, got) = spec_error(resolve(&parse(&json), 400, 300, 1.0, None));
        assert!(path.starts_with("annotations[0]"), "{shape}: {path}");
        assert!(got.contains(message), "{shape}: {got}");
    }
}

#[test]
fn a_seed_given_with_around_is_kept() {
    // Fitting may pick another seed only when the spec leaves it to markshot.
    let r = resolve_json(
        r#"{"annotations": [{"type": "oval", "seed": 5, "around": [300, 200, 240, 42]}]}"#,
        800,
        600,
    );
    let Resolved::Oval(oval) = &r.items[0] else {
        panic!("{:?}", r.items[0]);
    };
    assert_eq!(oval.seed, 5);
}
