# Warnings and errors

## Warnings

Warnings don't stop the render. Each names an annotation `id`, or `coords` or `image`. Read them before showing the result to the user.

| Message | Meaning and fix |
|---|---|
| `partly outside the W×H image; drawn clipped` | Fine if intended; otherwise the coordinates are off. A counter's tail and ring count, even when its circle fits. |
| `entirely outside the W×H image; skipped` | Wrong coordinates: check for a downscaled view (use `scaled`), swapped x and y, or y measured from the bottom. |
| `zero length: ...; skipped` | `from` and `to` are the same pixel. |
| `zero width or height after rounding to pixels; skipped` | A rect with no area. |
| ``could not fit an oval `around` its target ...; retry the original `around` spec ...`` | The best candidate approaches or encloses other ink, overlaps annotations, or leaves the image or crop. [Refit from the original target](../SKILL.md#fixing-a-render), omitting `size` or trying the suggested thinner size. Deleting `size` from the resolved `rect` alone does not refit it. |
| ``could not fit an oval `around` its target ...; frame it with a hand-sized `rect` or `oval` instead`` | No clean candidate was found, even at size 0 if size was automatic. Frame the target with a `rect`, or give the oval a hand-sized `rect`, and check it zoomed in. |
| `could not fit a counter within the image or crop bounds ...` | No tried direction keeps the full badge, tail and ring bounds inside the image or crop and clear of other annotation bounds. It is drawn at the best-effort angle without resizing. Retry the original `at` and `"angle": "auto"` spec with a smaller `size` (if above 0) or more room. From a report, recover `at` from `center` [before resizing](../SKILL.md#fixing-a-render) and restore `"angle": "auto"` to run placement again. A missing warning alone does not prove an edit fits. |
| `entirely outside the crop; drawn but not in the output` | The annotation misses the top-level `crop`: widen the crop or fix the coordinates. |
| `the scaled size ... does not have the aspect ratio ...` (id `coords`) | `coords.width`/`height` isn't the size you actually viewed; correct it. |
| `the PNG has no DPI, so dpr is 1; ...` (id `image`) | Decide the pixel density and set the top-level `"dpr"` ([spec.md](spec.md#size-and-dpr) says how). The stdout spec keeps it, so the warning doesn't come back. |
| `the image's colour profile is not sRGB or Display P3; treated as sRGB` (id `image`) | The PNG carries another colour profile, such as a display's own. Colours may look slightly off; nothing to fix unless exact colours matter. |

Skipped annotations are left out of the stdout `spec`; add them back with corrected values.

## Errors

An invalid spec exits with code 1, writes no PNG, and prints one line to stderr:

```
error: <path>: <message>
```

`<path>` locates the problem: a field such as `annotations[0].size`, a whole annotation such as `annotations[2]` (a value of the wrong type), `spec` (an unknown top-level or `coords` field), `dpr`, or a file that couldn't be read. Fix that value and run again. A malformed command line also exits with code 1, with the CLI's usage message.
