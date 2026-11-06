---
name: markshot
description: Use when annotating a PNG screenshot for the user — arrows, boxes, ovals, blurring or erasing private data, highlighter strokes, spotlights, numbered step badges or lines — with the markshot CLI, which draws reference-style annotations from a JSON spec.
---

# markshot

`markshot` draws reference-style annotations on a PNG from a JSON spec. The output is deterministic: the same input, spec and binary give the same PNG bytes.

```
markshot render <in.png> --spec <spec.json | -> -o <out.png> [--dpr N]
markshot grid   <in.png> -o <grid.png> [--step N]
markshot schema        # JSON schema of the spec, with every field documented
```

## The loop

1. Look at the screenshot and work out where each annotation goes. To get its pixel size, render an empty spec: `echo '{"annotations": []}' | markshot render in.png --spec - -o /tmp/x.png` prints `width` and `height`.
2. If you are unsure of positions, run `markshot grid in.png -o grid.png` and look at `grid.png`. Its labels are image pixel coordinates.
3. Write the spec, run `render`, then look at the output PNG.
4. To fix something, take the `spec` object from the stdout JSON, change only the values that are off, and render it again **from the original input**, not from the previous output. That `spec` is fully resolved: ids, seeds, numbers, the dpr and pixel coordinates are filled in, so everything you don't change stays exactly the same. `fill_alpha` is there only if you set it, so switching a rect's `fill` (say `outline` to `solid`, to redact what it frames) gives it the new fill's default opacity.

`render` prints `{"width", "height", "dpr", "spec", "warnings"}` to stdout. If the spec is invalid it exits with code 1, writes nothing, and prints `error: <path>: <message>` to stderr, for example `error: annotations[0].size: must be 0 to 7, got 9`.

## Coordinates

- Coordinates are pixels of the input image. The origin is the top-left corner and y grows downward. A point is `[x, y]` and a rect is `[x, y, width, height]`.
- If the image you looked at was downscaled (your viewer showed a 2880×1800 screenshot at 1440×900, say), give the size you saw and use the coordinates you read off it: `"coords": {"space": "scaled", "width": 1440, "height": 900}`. Each axis is scaled separately. If that size's aspect ratio is more than 1% off the image's, you get a warning.
- `"coords": {"space": "normalized"}` means 0–1000 across each axis.
- Every coordinate must stay within 16× the image's longer side of the origin, or the spec is rejected.
- Annotations partly outside the image are clipped. Ones entirely outside are skipped. Both cases produce a warning, and a skipped spotlight dims nothing.
- The stdout `spec` is always in image pixels, with `"coords": {"space": "pixels"}`.

## Draw order

All blurs come first, then the spotlight dimming, then everything else in spec order (spotlight borders included). An arrow listed after a rect is drawn on top of it. A blur always sits underneath, whatever its position in the list.

## Size and dpr

`size` is a step from 0 to 7 (default 2) that picks the thickness in points. Points are drawn at `dpr` pixels each. dpr comes from the PNG's DPI: 144 dpi, the macOS Retina capture default, gives 2, and no DPI gives 1. So a Retina screenshot gets strokes twice as wide in pixels, just like in the reference app. Override it with `--dpr` or a top-level `"dpr"` (0.5–4) only when the PNG's DPI is wrong, e.g. a Retina capture saved without DPI.

## Tools

Every example below is a complete spec, written for an 800×600 image.

### arrow — point at something

`style`: `shaped` (default, a filled tapered arrow), `sketch` (hand-drawn), `line` and `double`. `variant` 0–3 picks the `shaped` outline. `via` is a point the arrow curves through. When `via` bends it strongly, a `shaped` arrow overshoots `to` slightly, so leave a few pixels of room past the target.

```json
{"annotations": [
  {"type": "arrow", "from": [120, 420], "to": [330, 260]},
  {"type": "arrow", "style": "sketch", "from": [700, 120], "to": [520, 220]},
  {"type": "arrow", "style": "line", "from": [420, 520], "to": [700, 420], "via": [620, 540]}
]}
```

### rect / oval — frame something

`fill`: `sketch` (default, a hand-drawn border), `outline`, `semi` (a border plus a fill at `fill_alpha`, default 0.4) and `solid`. Shapes whose shorter side is 40 px or less are drawn with a plain border even when `sketch` is set, as in the reference app. An oval is the reference app's squircle, which is fuller than a true ellipse.

```json
{"annotations": [
  {"type": "rect", "rect": [80, 60, 260, 120]},
  {"type": "oval", "rect": [420, 60, 260, 120], "fill": "outline"},
  {"type": "rect", "rect": [80, 300, 260, 120], "fill": "semi", "color": "#2E7CF6"}
]}
```

### blur — hide private data

`mode`: `pixelate` (default, a mosaic whose block size grows with `size`) or `erase` (paints the area over with the colours along its border, which works well for text on a flat background). Make the rect cover the whole text with a few pixels to spare.

```json
{"annotations": [
  {"type": "blur", "rect": [100, 80, 300, 40]},
  {"type": "blur", "rect": [100, 160, 300, 40], "mode": "erase"}
]}
```

### highlight — mark text like a highlighter

Give `from`/`to` for a straight stroke or `points` for a polyline (a single point draws a dot). The blend depends on the background brightness: multiply on light backgrounds, translucent on mid and dark ones. So pass the plain base colour (default `#FFF68D`) and don't pre-darken it. `cap` 0–3 picks the end shape.

```json
{"annotations": [
  {"type": "highlight", "from": [100, 100], "to": [400, 100]},
  {"type": "highlight", "points": [[100, 300], [250, 320], [400, 300]], "color": "#A6F68D", "size": 4}
]}
```

### spotlight — dim everything except an area

The image is dimmed with black at `spotlight_dim` (top level, default 0.214), and every spotlight cuts a hole in that dimming. `shape` is `rect` (rounded, the default) or `oval`. `size` sets the border width, and `0` means no border.

```json
{"spotlight_dim": 0.4, "annotations": [
  {"type": "spotlight", "rect": [250, 180, 300, 160]},
  {"type": "spotlight", "rect": [600, 400, 120, 120], "shape": "oval", "size": 0}
]}
```

### counter — numbered step badges

`at` is the point the badge's tail marks, and the badge sits 1.65 radii away, opposite the tail. Use `center` to place the badge itself instead. `angle` is the tail direction in degrees: 0 right, 90 down, 135 down-left (the default). `style` is `bubble` (default), `pin` or `circle` (no tail). `number` defaults to one more than the largest number so far. The stdout `spec` gives each counter's `center`.

```json
{"annotations": [
  {"type": "counter", "at": [200, 200]},
  {"type": "counter", "at": [400, 200], "angle": 90, "style": "pin"},
  {"type": "counter", "center": [600, 200], "style": "circle", "number": 7}
]}
```

### line — a straight line

`dash` makes it dashed. `start_cap` and `end_cap` are each `none` (default), `arrow` or `bullet`.

```json
{"annotations": [
  {"type": "line", "from": [100, 300], "to": [700, 300], "dash": true},
  {"type": "line", "from": [100, 400], "to": [700, 400], "start_cap": "bullet", "end_cap": "arrow"}
]}
```

## Common combination: hide private data, then number the steps

These coordinates were read off a 400×300 preview of the 800×600 screenshot. Each badge marks the left edge of its frame with its tail pointing right (`angle: 0`), so it sits beside the frame instead of covering it. Name annotations with `id` so they are easy to find in the stdout spec when you fix them.

```json
{"coords": {"space": "scaled", "width": 400, "height": 300}, "annotations": [
  {"type": "blur", "id": "email", "rect": [40, 30, 160, 20]},
  {"type": "blur", "id": "token", "rect": [40, 60, 220, 20], "mode": "erase"},
  {"type": "rect", "id": "settings", "rect": [250, 120, 110, 40], "fill": "outline"},
  {"type": "counter", "id": "step-1", "at": [248, 140], "angle": 0},
  {"type": "rect", "id": "save", "rect": [250, 220, 110, 40], "fill": "outline"},
  {"type": "counter", "id": "step-2", "at": [248, 240], "angle": 0}
]}
```

## Defaults

| Field | Applies to | Default |
|---|---|---|
| `color` (`#RRGGBB`) | all but blur | `#FF0C01`; highlight `#FFF68D` |
| `size` (0–7) | all | 2 |
| `id` | all | `<type>-<n>`, e.g. `arrow-1` |
| `seed` | arrow, rect, oval, blur | derived from `id` |
| `style` | arrow / counter | `shaped` / `bubble` |
| `variant` (0–3) | arrow | 0 |
| `fill` | rect, oval | `sketch` |
| `fill_alpha` (0.1–1) | rect, oval | 0.4 for `semi`, 1 for `solid` |
| `mode` | blur | `pixelate` |
| `cap` (0–3) | highlight | 3 |
| `shape` | spotlight | `rect` |
| `angle` | counter | 135 |
| `number` | counter | one more than the largest so far |
| `dash`, `start_cap`, `end_cap` | line | `false`, `none`, `none` |
| `coords` | spec | `{"space": "pixels"}` |
| `spotlight_dim` (0–1) | spec | 0.214 |
| `dpr` (0.5–4) | spec | from the PNG's DPI (144 → 2), else 1 |

Unknown fields are errors. `markshot schema` documents every field.

## Warnings

Warnings don't stop the render. Each one names an annotation `id` (or `coords`). Read them before showing the result to the user.

| Message | Meaning and fix |
|---|---|
| `partly outside the W×H image; drawn clipped` | Fine if intended. Otherwise the coordinates are off. |
| `entirely outside the W×H image; skipped` | Wrong coordinates: check for a downscaled view (use `scaled`), swapped x/y, or y measured from the bottom. |
| `zero length: ...; skipped` | `from` and `to` are the same pixel. |
| `zero width or height after rounding to pixels; skipped` | A rect with no area. |
| `the scaled size ... does not have the aspect ratio ...` (id `coords`) | `coords.width`/`height` isn't the size you actually viewed. Correct it. |
