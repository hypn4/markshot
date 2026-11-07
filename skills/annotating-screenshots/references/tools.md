# Tools

Each example is a complete spec for an 800×600 screenshot. `markshot schema` lists every field, its range and its default.

## Contents

- [arrow — point at something](#arrow--point-at-something)
- [rect / oval — frame something](#rect--oval--frame-something)
- [blur — hide private data](#blur--hide-private-data)
- [highlight — mark text like a highlighter](#highlight--mark-text-like-a-highlighter)
- [spotlight — dim everything except an area](#spotlight--dim-everything-except-an-area)
- [counter — numbered step badges](#counter--numbered-step-badges)
- [line — a straight line](#line--a-straight-line)
- [Combining tools: hide private data, then number the steps](#combining-tools-hide-private-data-then-number-the-steps)

## arrow — point at something

`style`: `shaped` (default, a filled tapered arrow), `sketch` (hand-drawn), `line` and `double`. `variant` 0–3 picks the `shaped` outline. `via` is a point the arrow curves through. When `via` bends it strongly, a `shaped` arrow overshoots `to` slightly, so leave a few pixels of room past the target.

```json
{"annotations": [
  {"type": "arrow", "from": [120, 420], "to": [330, 260]},
  {"type": "arrow", "style": "sketch", "from": [700, 120], "to": [520, 220]},
  {"type": "arrow", "style": "line", "from": [420, 520], "to": [700, 420], "via": [620, 540]}
]}
```

## rect / oval — frame something

`fill`: `sketch` (default, a hand-drawn border), `outline`, `semi` (a border plus a fill at `fill_alpha`, default 0.4) and `solid` (an opaque box: use it with `"color": "#000000"` to redact). Shapes whose shorter side is 40 px or less get a plain border even with `sketch`, as in the reference app. An oval is the reference app's squircle, fuller than a true ellipse.

```json
{"annotations": [
  {"type": "rect", "rect": [80, 60, 260, 120]},
  {"type": "oval", "rect": [420, 60, 260, 120], "fill": "outline"},
  {"type": "rect", "rect": [80, 300, 260, 120], "fill": "semi", "color": "#2E7CF6"}
]}
```

## blur — hide private data

`mode`: `pixelate` (default, a mosaic whose block size grows with `size`) or `erase` (paints the area over with the colours along its border; best for text on a flat background). Cover the whole text with a few pixels to spare. A blur always reads the original pixels and is drawn first, under every other annotation.

```json
{"annotations": [
  {"type": "blur", "rect": [100, 80, 300, 40]},
  {"type": "blur", "rect": [100, 160, 300, 40], "mode": "erase"}
]}
```

## highlight — mark text like a highlighter

`from`/`to` draws a straight stroke, `points` a polyline; a single point draws a dot. The blend follows the background's brightness: multiply on light backgrounds, translucent on mid and dark ones. Pass the plain base colour (default `#FFF68D`), not a pre-darkened one. On mid and dark backgrounds the colour is drawn fully saturated, so grey, white or black come out navy there: pick a hue. `cap` 0–3 picks the end shape.

```json
{"annotations": [
  {"type": "highlight", "from": [100, 100], "to": [400, 100]},
  {"type": "highlight", "points": [[100, 300], [250, 320], [400, 300]], "color": "#A6F68D", "size": 4}
]}
```

## spotlight — dim everything except an area

The whole image is dimmed with black at the top-level `spotlight_dim` (default 0.214; try 0.4 for a stronger effect), and every spotlight cuts a hole in it. Overlapping holes merge. `shape` is `rect` (rounded, the default) or `oval`; `size` sets the border width, `0` for no border. A spotlight entirely outside the image is skipped, and then nothing is dimmed.

```json
{"spotlight_dim": 0.4, "annotations": [
  {"type": "spotlight", "rect": [250, 180, 300, 160]},
  {"type": "spotlight", "rect": [600, 400, 120, 120], "shape": "oval", "size": 0}
]}
```

## counter — numbered step badges

`at` is the point the badge's tail marks; the badge sits 1.65 radii away, opposite the tail. `angle` is the tail's direction in degrees: 0 right, 90 down, 135 down-left (the default). `style` is `bubble` (default), `pin` or `circle`; a `circle` has no tail and is centred on `at`. `center` places the badge itself instead of `at`. `number` defaults to one more than the largest so far. The badge diameter for `size` 0–7 is 22, 28, 36, 48, 64, 84, 112, 150 points.

To keep a badge off the thing it points at, aim the tail from the empty side: a badge left of a box points right with `"angle": 0`.

```json
{"annotations": [
  {"type": "counter", "at": [200, 200]},
  {"type": "counter", "at": [400, 200], "angle": 90, "style": "pin"},
  {"type": "counter", "center": [600, 200], "style": "circle", "number": 7}
]}
```

## line — a straight line

`dash` makes it dashed. `start_cap` and `end_cap` are each `none` (default), `arrow` or `bullet`.

```json
{"annotations": [
  {"type": "line", "from": [100, 300], "to": [700, 300], "dash": true},
  {"type": "line", "from": [100, 400], "to": [700, 400], "start_cap": "bullet", "end_cap": "arrow"}
]}
```

## Combining tools: hide private data, then number the steps

These coordinates were read off a 400×300 view of the 800×600 screenshot, hence `scaled`. Each badge marks the left edge of its frame with its tail pointing right, so it sits beside the frame instead of covering it.

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
