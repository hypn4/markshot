# Spec reference

`markshot schema` prints the authoritative JSON schema. This page explains the parts that affect where and how things are drawn.

## Contents

- [Coordinates](#coordinates)
- [Draw order](#draw-order)
- [Size and dpr](#size-and-dpr)
- [Defaults](#defaults)

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

`size` is a step from 0 to 7 (default 2) that picks a thickness in points. Each point is `dpr` pixels. dpr comes from the PNG's DPI (144 dpi, the macOS Retina capture default, gives 2; no DPI gives 1), so a Retina screenshot gets strokes twice as wide in pixels, and they look the same size on screen. Override it with `--dpr` or a top-level `"dpr"` (0.5–4) only when the PNG's DPI is wrong, e.g. a Retina capture saved without DPI.

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
| `mode` | blur | `pixelate` |
| `cap` (0–3) | highlight | 3 |
| `shape` | spotlight | `rect` |
| `angle` | counter | 135 |
| `number` | counter | one more than the largest so far |
| `dash`, `start_cap`, `end_cap` | line | `false`, `none`, `none` |
| `coords` | spec | `{"space": "pixels"}` |
| `spotlight_dim` (0–1) | spec | 0.214 |
| `dpr` (0.5–4) | spec | from the PNG's DPI (144 → 2), else 1 |

Unknown fields are errors.
