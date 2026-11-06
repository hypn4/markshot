//! Golden images: each tool's renders on the 600×400 fixture (dpr 2), blurs on its
//! [`common::screen`] version, compared byte for byte with `tests/golden/<name>.png`.
//! Regenerate with `UPDATE_GOLDEN=1 cargo test --test golden` and look at the images before
//! committing them.

mod common;

use markshot::image_io::SourceImage;
use markshot::render::render;
use markshot::spec::Spec;

/// Renders `spec` on the fixture, which must draw every annotation, and checks the golden.
fn check(name: &str, spec: &str) {
    check_on(name, common::fixture(600, 400), spec);
}

/// [`check`] on `src`.
fn check_on(name: &str, src: SourceImage, spec: &str) {
    let spec = Spec::from_json(spec).expect("spec parses");
    let (png, report) = render(src, &spec, None).expect("renders");
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

/// The same five `shaped` arrows in outline `variant`: long (the head grows, the tail shrinks),
/// size 0, size 5 in blue, short (both ends shrink) and straight up at size 4.
fn shaped(variant: u8) -> String {
    format!(
        r##"{{"annotations": [
            {{"type": "arrow", "variant": {variant}, "from": [30, 40], "to": [570, 110]}},
            {{"type": "arrow", "variant": {variant}, "size": 0, "from": [60, 250], "to": [250, 170]}},
            {{"type": "arrow", "variant": {variant}, "size": 5, "color": "#2E7CF6",
             "from": [580, 385], "to": [380, 235]}},
            {{"type": "arrow", "variant": {variant}, "from": [300, 380], "to": [350, 345]}},
            {{"type": "arrow", "variant": {variant}, "size": 4, "from": [110, 390], "to": [110, 290]}}
        ]}}"##
    )
}

#[test]
fn golden_arrow_shaped_0() {
    check("arrow-shaped-0", &shaped(0));
}

#[test]
fn golden_arrow_shaped_1() {
    check("arrow-shaped-1", &shaped(1));
}

#[test]
fn golden_arrow_shaped_2() {
    check("arrow-shaped-2", &shaped(2));
}

#[test]
fn golden_arrow_shaped_3() {
    check("arrow-shaped-3", &shaped(3));
}

#[test]
fn golden_arrow_shaped_curved() {
    // The first is over 520 px long, so its head grows and its tail shrinks.
    check(
        "arrow-shaped-curved",
        r#"{"annotations": [
            {"type": "arrow", "from": [30, 120], "to": [570, 120], "via": [300, 40]},
            {"type": "arrow", "variant": 1, "size": 1, "from": [60, 200], "to": [280, 370],
             "via": [70, 330]},
            {"type": "arrow", "variant": 2, "size": 4, "from": [560, 170], "to": [330, 380],
             "via": [520, 360]},
            {"type": "arrow", "variant": 3, "from": [300, 160], "to": [470, 250],
             "via": [400, 150]}
        ]}"#,
    );
}

/// Six `line` or `double` arrows: long, size 0, size 7 in blue, short (under 20 px: 45°
/// chevrons), and two bent ways.
fn lines(style: &str) -> String {
    format!(
        r##"{{"annotations": [
            {{"type": "arrow", "style": "{style}", "from": [40, 40], "to": [560, 100]}},
            {{"type": "arrow", "style": "{style}", "size": 0, "from": [60, 200], "to": [230, 140]}},
            {{"type": "arrow", "style": "{style}", "size": 7, "color": "#2E7CF6",
             "from": [560, 380], "to": [330, 220]}},
            {{"type": "arrow", "style": "{style}", "from": [300, 160], "to": [315, 165]}},
            {{"type": "arrow", "style": "{style}", "from": [60, 370], "to": [260, 280],
             "via": [90, 280]}},
            {{"type": "arrow", "style": "{style}", "size": 4, "from": [400, 130], "to": [560, 200],
             "via": [500, 120]}}
        ]}}"##
    )
}

#[test]
fn golden_arrow_line() {
    check("arrow-line", &lines("line"));
}

#[test]
fn golden_arrow_double() {
    check("arrow-double", &lines("double"));
}

#[test]
fn golden_arrow_sketch() {
    // Seed 42 draws an open head; the others cover the random branches: 1 a closed head, 33 a
    // wobbly shaft, 23 a closed head with a short base, 2 a shorter shaft, 43 all three.
    check(
        "arrow-sketch",
        r##"{"annotations": [
            {"type": "arrow", "style": "sketch", "seed": 42, "from": [40, 60], "to": [560, 110]},
            {"type": "arrow", "style": "sketch", "seed": 1, "from": [60, 250], "to": [260, 170]},
            {"type": "arrow", "style": "sketch", "seed": 33, "from": [300, 240], "to": [560, 160]},
            {"type": "arrow", "style": "sketch", "seed": 23, "size": 5, "color": "#2E7CF6",
             "from": [560, 390], "to": [360, 300]},
            {"type": "arrow", "style": "sketch", "seed": 2, "size": 0, "from": [80, 380],
             "to": [140, 330]},
            {"type": "arrow", "style": "sketch", "seed": 43, "from": [180, 300], "to": [320, 360]}
        ]}"##,
    );
}

#[test]
fn golden_arrow_sketch_curved() {
    check(
        "arrow-sketch-curved",
        r##"{"annotations": [
            {"type": "arrow", "style": "sketch", "seed": 42, "from": [40, 120], "to": [560, 120],
             "via": [300, 40]},
            {"type": "arrow", "style": "sketch", "seed": 1, "from": [60, 200], "to": [280, 370],
             "via": [70, 330]},
            {"type": "arrow", "style": "sketch", "seed": 23, "size": 4, "color": "#2E7CF6",
             "from": [560, 170], "to": [330, 380], "via": [520, 360]},
            {"type": "arrow", "style": "sketch", "seed": 2, "from": [300, 160], "to": [470, 250],
             "via": [400, 150]}
        ]}"##,
    );
}

/// Six `kind`s (`rect` or `oval`) with fill `fill`, all seed 7: wide at the default size,
/// size 0, tall at size 7 in blue, a 30 px square (stroke capped at a tenth of its side; a
/// sketch is drawn plain there), across the grey and dark bands at size 4, and on the dark
/// band at `fill_alpha` 0.7 (ignored by `outline` and `sketch`).
fn shapes(kind: &str, fill: &str) -> String {
    format!(
        r##"{{"annotations": [
            {{"type": "{kind}", "fill": "{fill}", "seed": 7, "rect": [30, 25, 260, 90]}},
            {{"type": "{kind}", "fill": "{fill}", "seed": 7, "size": 0, "rect": [330, 30, 90, 60]}},
            {{"type": "{kind}", "fill": "{fill}", "seed": 7, "size": 7, "color": "#2E7CF6",
             "rect": [460, 30, 110, 250]}},
            {{"type": "{kind}", "fill": "{fill}", "seed": 7, "rect": [40, 160, 30, 30]}},
            {{"type": "{kind}", "fill": "{fill}", "seed": 7, "size": 4,
             "rect": [110, 150, 300, 220]}},
            {{"type": "{kind}", "fill": "{fill}", "seed": 7, "fill_alpha": 0.7,
             "rect": [440, 300, 140, 80]}}
        ]}}"##
    )
}

#[test]
fn golden_rect_outline() {
    check("rect-outline", &shapes("rect", "outline"));
}

#[test]
fn golden_rect_semi() {
    check("rect-semi", &shapes("rect", "semi"));
}

#[test]
fn golden_rect_solid() {
    check("rect-solid", &shapes("rect", "solid"));
}

#[test]
fn golden_rect_size5() {
    // 16 px strokes with 32 px corners, and on a 40 px side an 8 px stroke.
    check(
        "rect-size5",
        r##"{"annotations": [
            {"type": "rect", "fill": "outline", "size": 5, "rect": [40, 30, 330, 340]},
            {"type": "rect", "fill": "outline", "size": 5, "rect": [420, 60, 140, 40]},
            {"type": "rect", "fill": "outline", "size": 5, "color": "#2E7CF6",
             "rect": [420, 160, 140, 200]}
        ]}"##,
    );
}

#[test]
fn golden_rect_sketch() {
    check("rect-sketch", &shapes("rect", "sketch"));
}

#[test]
fn golden_oval_outline() {
    check("oval-outline", &shapes("oval", "outline"));
}

#[test]
fn golden_oval_semi() {
    check("oval-semi", &shapes("oval", "semi"));
}

#[test]
fn golden_oval_solid() {
    check("oval-solid", &shapes("oval", "solid"));
}

#[test]
fn golden_oval_sketch() {
    // The size-7 oval's 22 px stroke is a fifth of its shorter side: a closed ring; the
    // others spiral.
    check("oval-sketch", &shapes("oval", "sketch"));
}

#[test]
fn golden_blur_pixelate() {
    // Seed 3: a large rect (blocks keep their own colour), one at size 5 and a strip at
    // size 0 (both small for their blocks, which then take neighbours' colours).
    check_on(
        "blur-pixelate",
        common::screen(600, 400),
        r#"{"annotations": [
            {"type": "blur", "seed": 3, "rect": [20, 40, 270, 200]},
            {"type": "blur", "seed": 3, "size": 5, "rect": [330, 60, 240, 130]},
            {"type": "blur", "seed": 3, "size": 0, "rect": [40, 290, 520, 60]}
        ]}"#,
    );
}

#[test]
fn golden_blur_erase() {
    // Words erased: along rows inside one bar, along columns across two bars (whose edge
    // stays), and along rows across three bars and two bands, as ramps.
    check_on(
        "blur-erase",
        common::screen(600, 400),
        r#"{"annotations": [
            {"type": "blur", "mode": "erase", "rect": [171, 50, 50, 20]},
            {"type": "blur", "mode": "erase", "rect": [139, 92, 34, 20]},
            {"type": "blur", "mode": "erase", "rect": [43, 110, 145, 50]}
        ]}"#,
    );
}
