//! Cross-platform determinism gate: a probe scene exercising the tiny-skia paths the
//! renderer relies on must hash identically on every target platform.

use std::fmt::Write as _;

use markshot::math;
use sha2::{Digest, Sha256};
use tiny_skia::{
    BlendMode, FillRule, LineCap, Paint, PathBuilder, Pixmap, PixmapPaint, Stroke, StrokeDash,
    Transform,
};

const PROBE_SHA: &str = "3daaec120d33aac51c1e9f198bab5b085bbba7f56cba88cb6ae61971743468e8";

const SIZE: u32 = 512;

#[expect(
    clippy::cast_possible_truncation,
    reason = "geometry is f64; tiny-skia takes f32"
)]
fn px(v: f64) -> f32 {
    v as f32
}

fn paint(r: u8, g: u8, b: u8, a: u8, blend_mode: BlendMode) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color_rgba8(r, g, b, a);
    paint.anti_alias = true;
    paint.blend_mode = blend_mode;
    paint
}

fn render_probe() -> Pixmap {
    let mut pixmap = Pixmap::new(SIZE, SIZE).expect("probe pixmap");
    let identity = Transform::identity();

    // (a) Round-capped cubic curve, stroke width 7.3.
    let mut pb = PathBuilder::new();
    pb.move_to(40.0, 400.0);
    pb.cubic_to(120.0, 60.0, 300.0, 470.0, 470.0, 90.0);
    let curve = pb.finish().expect("curve path");
    let stroke = Stroke {
        width: 7.3,
        line_cap: LineCap::Round,
        ..Stroke::default()
    };
    pixmap.stroke_path(
        &curve,
        &paint(255, 12, 1, 255, BlendMode::SourceOver),
        &stroke,
        identity,
        None,
    );

    // (b) Dashed stroke [3, 13].
    let mut pb = PathBuilder::new();
    pb.move_to(30.0, 60.0);
    pb.line_to(480.0, 140.0);
    pb.line_to(260.0, 300.0);
    let polyline = pb.finish().expect("dash path");
    let dashed = Stroke {
        width: 4.0,
        dash: StrokeDash::new(vec![3.0, 13.0], 0.0),
        ..Stroke::default()
    };
    pixmap.stroke_path(
        &polyline,
        &paint(20, 90, 230, 255, BlendMode::SourceOver),
        &dashed,
        identity,
        None,
    );

    // (c) Semi-transparent circle filled with Multiply.
    let circle = PathBuilder::from_circle(200.0, 220.0, 95.5).expect("multiply circle");
    pixmap.fill_path(
        &circle,
        &paint(255, 246, 141, 160, BlendMode::Multiply),
        FillRule::Winding,
        identity,
        None,
    );

    // (d) Curtain: black at alpha 0.214 over the whole layer, a circular hole cleared
    // with BlendMode::Clear, then composited onto the scene.
    let mut curtain = Pixmap::new(SIZE, SIZE).expect("curtain pixmap");
    let full = PathBuilder::from_rect(
        tiny_skia::Rect::from_xywh(0.0, 0.0, 512.0, 512.0).expect("curtain rect"),
    );
    let mut dim = Paint::default();
    dim.set_color(tiny_skia::Color::from_rgba(0.0, 0.0, 0.0, 0.214).expect("dim color"));
    curtain.fill_path(&full, &dim, FillRule::Winding, identity, None);
    let hole = PathBuilder::from_circle(330.0, 330.0, 77.7).expect("hole circle");
    curtain.fill_path(
        &hole,
        &paint(0, 0, 0, 255, BlendMode::Clear),
        FillRule::Winding,
        identity,
        None,
    );
    pixmap.draw_pixmap(
        0,
        0,
        curtain.as_ref(),
        &PixmapPaint::default(),
        identity,
        None,
    );

    // (e) Polygon whose vertices come from math::sin / math::cos / math::atan2.
    let (cx, cy) = (380.0_f64, 400.0_f64);
    let start = math::atan2(-37.0, 61.0);
    let mut pb = PathBuilder::new();
    for i in 0..14_u32 {
        let angle = start + f64::from(i) * math::PI / 7.0;
        let radius = if i % 2 == 0 { 92.3 } else { 41.7 };
        let x = cx + radius * math::cos(angle);
        let y = cy + radius * math::sin(angle);
        if i == 0 {
            pb.move_to(px(x), px(y));
        } else {
            pb.line_to(px(x), px(y));
        }
    }
    pb.close();
    let star = pb.finish().expect("star path");
    pixmap.fill_path(
        &star,
        &paint(40, 180, 90, 200, BlendMode::SourceOver),
        FillRule::EvenOdd,
        identity,
        None,
    );

    draw_off_edge_curves(&mut pixmap);
    pixmap
}

/// Probe elements that leave the pixmap, so that tiny-skia clips their curves at its edge.
fn draw_off_edge_curves(pixmap: &mut Pixmap) {
    let identity = Transform::identity();

    // (f) A shape bounded by two cubics that runs off the bottom edge, filled. Filling clips
    // each cubic where it crosses the edge by solving a cubic with acos and cos; with
    // tiny-skia's `std` floats those come from the platform's libm, and this shape (found
    // among 20,000 random ones) then renders differently on macOS, on Linux and with libm.
    let mut pb = PathBuilder::new();
    pb.move_to(75.52801, 468.87222);
    pb.cubic_to(299.24667, 238.5326, 427.59296, 603.164, 533.7636, 586.24963);
    pb.line_to(322.05682, 555.8867);
    pb.cubic_to(
        339.94424, 419.34305, 447.19772, 85.73394, 82.52801, 473.87222,
    );
    pb.close();
    let band = pb.finish().expect("band path");
    pixmap.fill_path(
        &band,
        &paint(150, 60, 220, 255, BlendMode::SourceOver),
        FillRule::Winding,
        identity,
        None,
    );

    // (g) Round-capped cubic stroke running off the right edge; the stroker turns it into
    // quads before it is clipped.
    let mut pb = PathBuilder::new();
    pb.move_to(330.0, 70.0);
    pb.cubic_to(470.0, 10.0, 560.0, 210.0, 640.0, 260.0);
    let off_edge = pb.finish().expect("off-edge curve path");
    let stroke = Stroke {
        width: 9.5,
        line_cap: LineCap::Round,
        ..Stroke::default()
    };
    pixmap.stroke_path(
        &off_edge,
        &paint(20, 160, 200, 255, BlendMode::SourceOver),
        &stroke,
        identity,
        None,
    );
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hex = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        write!(hex, "{byte:02x}").expect("write to String");
    }
    hex
}

#[test]
fn probe_scene_hash_is_stable() {
    let pixmap = render_probe();
    let actual = sha256_hex(pixmap.data());
    assert_eq!(
        actual, PROBE_SHA,
        "probe scene RGBA SHA-256 differs from the recorded value"
    );
}
