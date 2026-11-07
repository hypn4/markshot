# Spec reference

`markshot schema` prints the authoritative JSON schema. This page explains the parts that affect where and how things are drawn.

## Contents

- [Coordinates](#coordinates)
- [Draw order](#draw-order)
- [Size and dpr](#size-and-dpr)
- [Defaults](#defaults)
- [Colours](#colours)
- [Cropping the output](#cropping-the-output)

## Coordinates

- Pixels of the input image: the origin is the top-left corner and y grows downward. A point is `[x, y]`, a rect `[x, y, width, height]`. Fractions are rounded to whole pixels.
- `"coords": {"space": "scaled", "width": W, "height": H}`: the coordinates were read on a W×H view of the image. Each axis is scaled separately; a W×H whose aspect ratio is more than 1 % off the image's gives a warning. Use it only for positions estimated on that view; numbers read off `grid` labels are already image pixels.
- `"coords": {"space": "normalized"}`: 0–1000 across each axis.
- Every coordinate must stay within 16× the image's longer side of the origin, or the spec is rejected.
- Annotations partly outside the image are clipped; ones entirely outside are skipped. Both give a warning.
- The stdout `spec` is always in image pixels with `"coords": {"space": "pixels"}`.

## Draw order

1. All blurs, whatever their position in the list.
2. The spotlight dimming (once, with every spotlight's hole cut out).
3. Everything else in spec order, spotlight borders included: an annotation listed later is drawn on top.

## Size and dpr

`size` is a step from 0 to 7 (default 2) that picks a thickness in points. Each point is `dpr` pixels. dpr comes from the PNG's DPI (144 dpi, the macOS Retina capture default, gives 2), so a Retina screenshot gets strokes twice as wide in pixels, and they look the same size on screen. Override it with `--dpr` or a top-level `"dpr"` (0.5–4) when the PNG's DPI is wrong.

A PNG without DPI gets dpr 1 and a warning, because many tools save Retina captures that way. Decide from the pixels and set `"dpr"`: ordinary UI text (menu items, button labels) with capital letters about 9–12 px tall is dpr 1, about 18–24 px is dpr 2. A full-window capture wider than about 2000 px is usually dpr 2.

## Defaults

| Field | Applies to | Default |
|---|---|---|
| `color` (`#RRGGBB`) | all but blur | `#FF0C01`; highlight `#FFF68D` |
| `size` (0–7) | all | 2 |
| `id` | all | `<type>-<n>`, e.g. `arrow-1` |
| `seed` (0 to 2^53 − 1) | arrow, rect, oval, blur | derived from `id` |
| `style` | arrow / counter | `shaped` / `bubble` |
| `variant` (0–3) | arrow | 0 |
| `fill` | rect, oval | `sketch` |
| `fill_alpha` (0.1–1) | rect, oval | 0.4 for `semi`, 1 for `solid` |
| `spotlight` | rect, oval | `false` |
| `mode` | blur | `pixelate` |
| `cap` (0–3) | highlight | 3 |
| `shape` | spotlight | `rect` |
| `angle` | counter | 135 |
| `number` | counter | one more than the largest so far |
| `dash`, `start_cap`, `end_cap` | line | `false`, `none`, `none` |
| `coords` | spec | `{"space": "pixels"}` |
| `spotlight_dim` (0–1) | spec | 0.214 |
| `crop` (`[x, y, width, height]`) | spec | the whole image |
| `dpr` (0.5–4) | spec | from the PNG's DPI (144 → 2), else 1 |

Unknown fields are errors.

## Colours

`color` takes `#RRGGBB` only. When the user names a colour, use the same value for it every time:

| Asked for | `color` |
|---|---|
| red (the default) | `#FF0C01` |
| red-orange, orange-red | `#FF4500` |
| orange | `#FF9500` |
| yellow | `#FFDC2C` |
| green | `#34C759` |
| blue | `#2E7CF6` |
| purple | `#AF52DE` |
| black (also for redaction) | `#000000` |
| white | `#FFFFFF` |

For a highlighter, pass the plain colour; the blend darkens it on light backgrounds.

## Cropping the output

A top-level `"crop": [x, y, width, height]` writes only that part of the annotated image, for a guide that shows one panel. It uses the same coordinates as the annotations, so keep reading positions on the whole image; the report's `width` and `height` stay the whole image's. Leave room around the marks: badges and hand-drawn lines reach a little past their targets.

```json
{"crop": [60, 40, 520, 300], "annotations": [
  {"type": "rect", "rect": [120, 100, 200, 80], "fill": "outline"},
  {"type": "counter", "at": [118, 140], "angle": 0}
]}
```
