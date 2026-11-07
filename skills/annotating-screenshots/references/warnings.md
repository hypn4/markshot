# Warnings and errors

## Warnings

Warnings don't stop the render. Each names an annotation `id`, or `coords` or `image`. Read them before showing the result to the user.

| Message | Meaning and fix |
|---|---|
| `partly outside the W×H image; drawn clipped` | Fine if intended; otherwise the coordinates are off. |
| `entirely outside the W×H image; skipped` | Wrong coordinates: check for a downscaled view (use `scaled`), swapped x and y, or y measured from the bottom. |
| `zero length: ...; skipped` | `from` and `to` are the same pixel. |
| `zero width or height after rounding to pixels; skipped` | A rect with no area. |
| `the scaled size ... does not have the aspect ratio ...` (id `coords`) | `coords.width`/`height` isn't the size you actually viewed; correct it. |
| `the image's colour profile is not sRGB or Display P3; treated as sRGB` (id `image`) | The PNG carries another colour profile, such as a display's own. Colours may look slightly off; nothing to fix unless exact colours matter. |

Skipped annotations are left out of the stdout `spec`; add them back with corrected values.

## Errors

An invalid spec exits with code 1, writes no PNG, and prints one line to stderr:

```
error: <path>: <message>
```

`<path>` locates the problem: a field such as `annotations[0].size`, a whole annotation such as `annotations[2]` (a value of the wrong type), `spec` (an unknown top-level or `coords` field), `dpr`, or a file that couldn't be read. Fix that value and run again. A malformed command line also exits with code 1, with the CLI's usage message.
