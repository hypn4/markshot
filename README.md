# markshot

A single-binary CLI that draws [the reference app](https://reference.cc)-style annotations on a PNG screenshot: arrows, rectangles, ovals, blur, highlighter strokes, spotlights, numbered counters and lines. It is built for AI coding agents. The agent writes a JSON spec, renders it, looks at the result, then fixes values in the resolved spec it gets back and renders again.

- Matches the reference app's rendering: stroke widths, hand-drawn jitter, arrow outlines, highlighter blending, spotlight dimming and counter badges.
- Deterministic. The same input, spec and binary give the same PNG bytes, and randomness such as hand-drawn jitter and mosaic blocks is seeded from each annotation's id.
- Runs on macOS and Linux (arm64 and x86_64) as one self-contained binary; the Linux builds need glibc 2.34 or later. It keeps the input's DPI and ICC profile.

## Install

**Release binaries.** Each `v*` tag publishes one archive per platform on the repository's GitHub Releases page, with a SHA-256 checksum next to it:

| Platform | Asset |
|---|---|
| macOS, Apple silicon | `markshot-aarch64-apple-darwin.tar.gz` |
| macOS, Intel | `markshot-x86_64-apple-darwin.tar.gz` |
| Linux x86_64 (glibc) | `markshot-x86_64-unknown-linux-gnu.tar.gz` |
| Linux arm64 (glibc) | `markshot-aarch64-unknown-linux-gnu.tar.gz` |

```sh
shasum -a 256 -c markshot-aarch64-apple-darwin.tar.gz.sha256
tar -xzf markshot-aarch64-apple-darwin.tar.gz
install markshot-aarch64-apple-darwin/markshot ~/.local/bin/
```

Each archive holds the binary, this README, the agent skill (`skills/annotating-screenshots/`) and the font license (`OFL.txt`).

**From source.** You need Rust 1.99, which `rust-toolchain.toml` selects. From a clone:

```sh
cargo install --path . --locked   # installs markshot into ~/.cargo/bin
cargo build --release --locked    # or build target/release/markshot
```

The release binary is 3,226,432 bytes (3.1 MiB), measured on aarch64-apple-darwin with `cargo build --release`.

## Example

```sh
cat > spec.json <<'EOF'
{"annotations": [
  {"type": "blur", "rect": [40, 60, 320, 28]},
  {"type": "rect", "rect": [480, 400, 220, 64], "fill": "outline"},
  {"type": "counter", "at": [476, 432], "angle": 0},
  {"type": "arrow", "from": [590, 250], "to": [590, 390]}
]}
EOF
markshot render screenshot.png --spec spec.json -o annotated.png
```

`render` writes `annotated.png` and prints JSON to stdout: the image size, the dpr used, the fully resolved `spec` (ids, seeds, counter numbers and pixel coordinates filled in) and any `warnings`. If you feed that `spec` back in, you get the same image. Invalid specs exit with code 1 and print `error: <path>: <message>`, e.g. `error: annotations[0].size: must be 0 to 7, got 9`.

The other commands:

- `markshot grid screenshot.png -o grid.png` overlays a labeled pixel grid, for reading coordinates.
- `markshot schema` prints the JSON schema of the spec, with every field documented.

## Agent skill

[`skills/annotating-screenshots/`](skills/annotating-screenshots/SKILL.md) is an [Agent Skills](https://agentskills.io/specification) skill. `SKILL.md` holds the workflow, coordinates, a tool picker, how to fix a render and common mistakes; `references/` holds every tool with an example, the spec details and defaults, and what each warning means. Agents load the references only when they need them.

Install it by copying the whole directory into a skills folder your agent reads, for example:

```sh
mkdir -p ~/.claude/skills && cp -R skills/annotating-screenshots ~/.claude/skills/   # Claude Code, all projects
mkdir -p .claude/skills && cp -R skills/annotating-screenshots .claude/skills/       # Claude Code, one project
mkdir -p ~/.agents/skills && cp -R skills/annotating-screenshots ~/.agents/skills/   # Codex, Gemini CLI, Copilot CLI
```

The skill needs `markshot` on the agent's PATH. Validate it against the specification with `uvx --from skills-ref agentskills validate skills/annotating-screenshots`; `cargo test` checks its frontmatter, links and examples too. [`evals/annotating-screenshots/`](evals/annotating-screenshots/README.md) holds the agent scenarios it was tested with; rerun them after changing the skill.

## Development

```sh
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

`docs/` holds the design.

## Notices

- markshot is not affiliated with the reference app. the reference app design assets are copied from the reference app: the four shaped-arrow outlines and the counter tail paths.
- Counter labels are laid out with SF Pro's numeric metrics (ascender, descender, digit height and tracking). No SF Pro outlines are included. The digits are drawn with Inter.
- The embedded Inter fonts (`assets/fonts/Inter-Regular.ttf`, `assets/fonts/Inter-Medium.ttf`) are © The Inter Project Authors and licensed under the SIL Open Font License 1.1. See [`assets/fonts/OFL.txt`](assets/fonts/OFL.txt).
