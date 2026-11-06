//! Golden images: each tool's renders on the 600×400 fixture (dpr 2), compared byte for byte
//! with `tests/golden/<name>.png`. Regenerate with `UPDATE_GOLDEN=1 cargo test --test golden`
//! and look at the images before committing them.

mod common;

use markshot::render::render;
use markshot::spec::Spec;

/// Renders `spec` on the fixture, which must draw every annotation, and checks the golden.
fn check(name: &str, spec: &str) {
    let spec = Spec::from_json(spec).expect("spec parses");
    let (png, report) = render(common::fixture(600, 400), &spec, None).expect("renders");
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    common::assert_golden(name, &png);
}

#[test]
fn golden_line_solid() {
    check(
        "line-solid",
        r##"{"annotations": [
            {"type": "line", "size": 0, "from": [40, 30], "to": [190, 370]},
            {"type": "line", "from": [130, 30], "to": [330, 370]},
            {"type": "line", "size": 7, "color": "#2E7CF6", "from": [260, 40], "to": [480, 360]},
            {"type": "line", "size": 4, "from": [500, 200], "to": [580, 200]},
            {"type": "line", "from": [520, 60], "to": [532, 69]}
        ]}"##,
    );
}

#[test]
fn golden_line_dash() {
    check(
        "line-dash",
        r#"{"annotations": [
            {"type": "line", "dash": true, "size": 0, "from": [40, 30], "to": [190, 370]},
            {"type": "line", "dash": true, "from": [130, 30], "to": [330, 370]},
            {"type": "line", "dash": true, "size": 7, "from": [260, 40], "to": [480, 360]},
            {"type": "line", "dash": true, "from": [490, 100], "to": [590, 100]},
            {"type": "line", "dash": true, "from": [540, 150], "to": [540, 390]}
        ]}"#,
    );
}

#[test]
fn golden_line_caps() {
    check(
        "line-caps",
        r#"{"annotations": [
            {"type": "line", "from": [40, 40], "to": [300, 200],
             "start_cap": "arrow", "end_cap": "bullet"},
            {"type": "line", "from": [40, 360], "to": [300, 240],
             "start_cap": "bullet", "end_cap": "arrow"},
            {"type": "line", "from": [340, 60], "to": [355, 60],
             "start_cap": "arrow", "end_cap": "arrow"},
            {"type": "line", "from": [340, 120], "to": [358, 120],
             "start_cap": "bullet", "end_cap": "bullet"},
            {"type": "line", "size": 7, "from": [380, 150], "to": [570, 330],
             "start_cap": "bullet", "end_cap": "arrow"},
            {"type": "line", "dash": true, "from": [400, 40], "to": [580, 100],
             "start_cap": "arrow", "end_cap": "bullet"}
        ]}"#,
    );
}

#[test]
fn golden_highlight_light() {
    check(
        "highlight-light",
        r#"{"annotations": [{"type": "highlight", "from": [60, 66], "to": [540, 66]}]}"#,
    );
}

#[test]
fn golden_highlight_mid() {
    check(
        "highlight-mid",
        r#"{"annotations": [{"type": "highlight", "from": [60, 200], "to": [540, 200]}]}"#,
    );
}

#[test]
fn golden_highlight_dark() {
    check(
        "highlight-dark",
        r#"{"annotations": [{"type": "highlight", "from": [60, 333], "to": [540, 333]}]}"#,
    );
}

#[test]
fn golden_highlight_polyline() {
    check(
        "highlight-polyline",
        r##"{"annotations": [
            {"type": "highlight", "cap": 0,
             "points": [[40, 50], [120, 85], [200, 45], [280, 90]]},
            {"type": "highlight", "cap": 1, "size": 1, "color": "#ADF6FF",
             "points": [[330, 40], [400, 60], [470, 95], [560, 100]]},
            {"type": "highlight", "cap": 2, "points": [[80, 165], [80, 235]]},
            {"type": "highlight",
             "points": [[150, 200], [250, 206], [350, 197], [450, 210], [560, 204]]},
            {"type": "highlight", "points": [[100, 330]]},
            {"type": "highlight", "points": [[250, 300], [350, 360], [450, 300], [550, 360]]}
        ]}"##,
    );
}
