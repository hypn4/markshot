# markshot design

markshot draws annotations on PNG screenshots for AI coding agents. This document records the design as built: what it is for, its interface and the rules the renderer keeps. `markshot schema` is the field-by-field reference, and [`skills/annotating-screenshots/`](../skills/annotating-screenshots/SKILL.md) is the guide agents read.

## Intent

A user gives a coding agent a screenshot and a request in plain words: "put an arrow on the login button and hide the email address". The agent writes a JSON spec, renders it with markshot, looks at the output and fixes values until the image is right. The agent does the editing; the person asks and gets the image back.

- **A CLI and a skill.** Coding agents already run shell commands, a CLI costs little context, and it is easy to test. The [Agent Skills](https://agentskills.io/specification) skill teaches the workflow.
- **Render, look, fix.** markshot does not detect UI elements. The agent picks the coordinates, renders, views the PNG and edits the resolved spec it got back, which keeps drawing the same image until a value changes.

## Goals and non-goals

Goals:

- Eight tools: arrow, rect, oval, blur (pixelate or erase), highlight, spotlight, counter and line.
- Annotations that suit screenshots: thicknesses in points scaled to the screenshot's pixel density, hand-drawn styles, a highlighter that adapts to the background, badges whose digits stay legible.
- A PNG and a spec in; a PNG and the resolved spec out.
- Deterministic output: the same input, spec and binary give the same PNG bytes on every supported platform.
- One self-contained native binary for macOS (Apple silicon), Linux (arm64, x86_64) and Windows (arm64, x86_64), with no runtime, network or system-font dependencies. Windows builds link the MSVC C runtime statically (`.cargo/config.toml`).

Non-goals:

- A GUI, mouse input, selection handles, snapping or undo.
- An MCP server. The library could sit under one later.
- OCR or UI-element detection, and so no text-aware blurring.
- Text, freehand, magnifier, ruler and backdrop tools.
- Growing the canvas past the image: annotations are clipped at its edges.

These count as failures even when the program runs: different bytes for the same input (random patterns included), annotations off target because the agent looked at a resized copy, and an install that needs a compiler, Node or Python.

markshot is written in Rust (edition 2024) on tiny-skia, with ttf-parser for glyph outlines, png for I/O, clap, serde, schemars and libm. Inter Regular and Medium are embedded and drawn as outlines.

## CLI

```
markshot render <input.png> --spec <spec.json | -> -o <out.png> [--dpr N]
markshot grid   <input.png> -o <grid.png> [--step N] [--region X,Y,W,H]
markshot schema
```

- `render` draws the spec on the input, writes the PNG and prints a JSON report to stdout: `width`, `height`, the `dpr` used, the resolved `spec` and `warnings`. The resolved spec has every default filled in (ids, seeds, counter numbers, dpr) and every coordinate in image pixels; rendering it again draws the same bytes. Only alternatives not taken stay out: counters come back as `center`, highlights as `points`, straight arrows without `via`, and a `fill_alpha` the spec left out stays out, so switching `fill` gives the new fill its own default opacity.
- `grid` draws a 1 px `#FF00FF` line at α 0.6 on every column and row that is a positive multiple of `step`, labelled with its coordinate in Inter Medium `12 * dpr` px, white on black at α 0.6: columns along the top edge, rows along the left edge, each label just past its line and pulled back inside the image at the far end. The default step is the smallest of 50, 100, 200, 250, 500 and 1000 that splits the longer side into at most 20 intervals and is at least as wide as the last column label, so labels never overlap, else 1000. `--step 0` is an error. `--region X,Y,W,H` draws only that part of the image, clipped to it, with the whole image's lines and labels, so a small area can be read at full size on a large screenshot; the default step then follows the region's size. A region without area, outside the image or not four whole numbers is an error at `--region`.
- `schema` prints the spec's JSON schema. Its descriptions are the doc comments in `src/spec.rs`.
- Exit status 0 on success. Invalid input exits 1 and writes no PNG: a bad spec or an unreadable file prints one `error: <path>: <message>` line on stderr (e.g. `error: annotations[0].size: must be 0 to 7, got 9`), and a malformed command line prints the usage message. Coordinates off the image are not errors (see [Input bounds](#input-bounds)).

## Coordinates

- The default space is pixels of the input image: origin top-left, y down. A point is `[x, y]`, a rect `[x, y, width, height]`.
- `"coords": {"space": "scaled", "width": W, "height": H}` maps coordinates read on a W×H view of the image back to image pixels, each axis separately. A W×H whose aspect ratio is more than 1 % off the image's gets a warning.
- `"coords": {"space": "normalized"}` spans 0–1000 on each axis.
- Resolved coordinates are image pixels rounded half away from zero, and the resolved spec says `"coords": {"space": "pixels"}`.
- Angles are on screen: 0° points right, 90° down. The drawing code works in y-up pixels and converts at the edges, keeping text upright.

## dpr

Thicknesses, badge diameters and arrow scales are in points, and one point is `dpr` pixels, so an annotation has the same size on screen whatever the capture's pixel density. The dpr is the first of:

1. `--dpr`;
2. the spec's top-level `dpr`;
3. the PNG's pHYs DPI as `clamp(round(dpi / 72), 1, 3)`: 144 dpi, the macOS Retina capture default, gives 2;
4. 1.

The report and the resolved spec record the dpr used, so feeding the spec back keeps it. When the dpr falls back to 1 because the PNG has no pHYs, the report warns (id `image`): a Retina capture saved without DPI would otherwise get annotations half their intended size. Any `dpr` in the spec, such as the resolved spec's, or `--dpr` answers the warning.

## Defaults

Every annotation has `type`, `id`, `color` and `size`; arrow, rect, oval and blur also take a `seed`.

| Field | Applies to | Default |
|---|---|---|
| `id` | all | `<type>-<n>`, skipping ids already taken |
| `color` (`#RRGGBB`) | all but blur | `#FF0C01`; highlight `#FFF68D` |
| `size` (0–7) | all | 2 |
| `seed` (0 to 2^53 − 1) | arrow, rect, oval, blur | the low 53 bits of the 64-bit FNV-1a hash of `id` |
| `style` | arrow / counter | `shaped` / `bubble` |
| `variant` (0–3) | arrow | 0 |
| `fill` | rect, oval | `sketch` |
| `fill_alpha` (0.1–1) | rect, oval | 0.4 for `semi`, 1 for `solid` |
| `mode` | blur | `pixelate` |
| `cap` (0–3) | highlight | 3 |
| `shape` | spotlight | `rect` |
| `angle` | counter | 135 (down-left) |
| `number` | counter | one more than the largest before it |
| `dash`, `start_cap`, `end_cap` | line | `false`, `none`, `none` |
| `coords` | spec | `{"space": "pixels"}` |
| `spotlight_dim` (0–1) | spec | 0.214 |
| `dpr` (0.5–4) | spec | from the PNG's DPI, else 1 |

Seeds come from ids rather than the clock so that one input always gives one output. They stop at 2^53 − 1 so that a resolved spec edited with tools that read JSON numbers as doubles (JavaScript, jq) keeps them exact. Unknown fields are errors.

## Tools

`size` picks a value in points from a per-tool table:

| size | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
|---|---|---|---|---|---|---|---|---|
| rect, oval and line stroke | 1 | 2 | 3 | 4 | 6 | 8 | 11 | 15 |
| `sketch`, `line` and `double` arrow stroke | 1 | 2 | 3 | 4 | 6 | 10 | 14 | 20 |
| `shaped` arrow outline scale | 0.33 | 0.5 | 0.7 | 0.9 | 1.1 | 1.4 | 2.0 | 2.8 |
| highlight width | 14 | 20 | 28 | 38 | 54 | 76 | 108 | 154 |
| spotlight border | 0 | 2 | 3 | 4 | 6 | 8 | 11 | 15 |
| counter diameter | 22 | 28 | 36 | 48 | 64 | 84 | 112 | 150 |

- **arrow** points from `from` (tail) to `to` (tip). `style` is `shaped` (a filled arrow with a tapered shaft, one of four bundled outlines picked by `variant`), `sketch` (hand-drawn, seeded), `line` (a stroke with an arrowhead) or `double` (arrowheads at both ends). `via` bends it along the circle through the tail, `via` and the tip; a strongly bent `shaped` arrow overshoots `to` slightly. Shaft and head are filled one after the other in the opaque colour, which looks the same as filling their union.
- **rect** and **oval** frame `rect`. `fill` is `sketch` (a hand-drawn border), `outline`, `semi` (the border over a fill at `fill_alpha`) or `solid`. A shape whose shorter side is 40 px or less gets a plain border even with `sketch`. The border width is at most a tenth of the shorter side. A rect has slightly rounded corners; an oval is a squircle through the midpoints of the rect's sides, fuller than an ellipse, whose ends stay round when it is stretched. `"spotlight": true` also dims everything outside the frame, as a spotlight does, with the hole cut along the frame's own centre line: the hand-drawn line when there is one, its spiral closed by a straight line between its ends. The dimming therefore ends under the border instead of beside it. The resolved spec lists `spotlight` only when it is true.
- **blur** hides `rect`. `pixelate` paints a mosaic of mean colours over a seeded random partition into blocks `s` to `3s` px a side, where `s = max(2, round(min(round(min(w, h) / 6), 5 * dpr) * 1.35^size))`; `erase` fills the area with ramps between the colours along its border, which suits text on a flat background.
- **highlight** draws a marker stroke from `from` to `to` or through `points`; a single point draws a dot. `cap` picks the ends: 0 round, 1 flat round, 2 notched, 3 double lobe. The blend follows the luminance of the image under the stroke: above 0.66 the colour is multiplied as it is; above 0.33 its fully saturated version is laid over at α 0.25; at 0.33 or below that is screened at α 0.33. Grey, white and black therefore come out navy on mid and dark backgrounds.
- **spotlight** leaves `rect` undimmed and dims the rest under one black curtain at `spotlight_dim`, kept as 8-bit alpha (the default 0.214 is 55/255). `shape` is `rect` (rounded) or `oval` (the squircle); overlapping holes merge. `size` is the border width, 0 for none. The borders are drawn with the other annotations, in spec order.
- **counter** draws a numbered badge: `bubble` (rounded tail), `pin` (pointed tail) or `circle` (no tail). `at` is the point the tail marks, and the centre sits 1.65 radii from it on the far side (a `circle` is centred on `at`); `center` places the badge directly. `angle` is the tail's direction. The digits are Inter laid out with SF Pro's numeric metrics (ascender, descender, digit height and tracking): white Medium on fills with Rec. 601 luma up to 0.6, black Regular on lighter ones, `0.55 * d * dpr` px for one digit and 10 % smaller for more.
- **line** strokes `from` to `to`, optionally dashed, with `start_cap` and `end_cap` each `none`, `arrow` or `bullet`.

Some formulas are irregular on purpose, because the established look depends on them; the code comments mark each one, and changing any of them changes the goldens. Examples: the hand-drawn rect's corner jitter scales x with the height and y with the width; the sketch arrow's wings turn π times their angle, spreading about 28–35°; the border cap above compares a tenth of the side in pixels with a width in points; some angle conversions use π truncated to 3.1415926.

## Draw order

1. The input image.
2. Every blur, in spec order.
3. The spotlight curtain, once, with the union of all spotlight holes (and the holes of rects and ovals with `spotlight` set) cut out. It dims the image and the blurs, not the annotations.
4. Everything else in spec order, spotlight borders included: a later annotation is drawn on top.

Blur and highlight read the input image's pixels, never another annotation's.

## Determinism

- Randomness (hand-drawn jitter, mosaic partitions) comes from a xorshift-multiply generator seeded per annotation, so it never changes between runs.
- tiny-skia is built without its default features, so it has no SIMD paths, and with `no-std-float`, so its float functions come from the `libm` crate; the renderer's own trigonometry and powers use `libm` as well. With the platform's libm, curves clipped at the image edge differed by a pixel between macOS and Linux.
- The PNG encoder's settings are fixed, and the output carries only the pixels, the input's pHYs and its iCCP.
- The golden images and the determinism probe hash were recorded on macOS; CI runs the whole test suite natively on every release platform (no emulation), so every platform must draw the same bytes. The release workflow repeats the suite with the shipped release profile on each platform before packaging.

## Colour handling

- Annotation colours are sRGB `#RRGGBB`, without alpha; only `fill_alpha`, the highlighter blends and the spotlight curtain are translucent.
- An input without an ICC profile or with an sRGB one is drawn on as is.
- An input tagged Display P3, as macOS captures often are, gets its annotation colours converted from sRGB to Display P3 with a fixed matrix (linear sRGB to linear P3, both D65, sharing the sRGB transfer curve), so they don't look oversaturated. The profile is kept.
- Any other profile is treated as sRGB, with a warning for `image`.
- Blur and highlight use the image's bytes as they are. There is no colour-management engine.

## Input bounds

- `--dpr` and the spec's `dpr` must be 0.5–4, and every coordinate, once mapped to pixels, must lie within 16× the image's longer side of the origin. Otherwise the spec is rejected (exit 1). Far-off coordinates or a tiny dpr would otherwise make curve sampling run practically forever.
- A counter's `center` may lie up to 1.65R + 0.5 px past that limit, R being the badge radius in pixels, since an `at` inside the limit can resolve to such a centre and the resolved spec must render again.
- An annotation partly off the image is clipped, with a warning. One entirely off the image, a zero-length arrow or line, or a `rect` with zero width or height after rounding is skipped with a warning and left out of the resolved spec. A spotlight that is skipped cuts no hole; with no spotlight left, nothing is dimmed.

## Testing

`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` (pedantic lints on) and `cargo doc` with warnings denied run once in CI; `cargo test` runs on all five release platforms.

- `tests/golden.rs`: every tool and mode rendered on a synthetic 600×400 fixture at dpr 2 (white, `#808080` and `#1E1E1E` bands) and compared byte for byte with `tests/golden/`. Regenerate with `UPDATE_GOLDEN=1 cargo test --test golden` only for an intended change, after looking at the images.
- `tests/determinism.rs`: a probe scene of the tiny-skia paths the renderer uses must hash to a recorded SHA-256.
- `tests/resolve.rs` and the `cli_*` tests: coordinate spaces, rounding, defaults, ids and seeds, bounds, warnings, error locations, exit codes, the grid, the schema, and that rendering a report's spec again gives identical bytes.
- `tests/skill_examples.rs`: the skill's frontmatter and links, and that every JSON example in it renders on an 800×600 screenshot without warnings.
- [`evals/annotating-screenshots/`](../evals/annotating-screenshots/README.md): agent scenarios for the skill, scored by `check.py`; rerun them after changing the skill.
