# markshot

markshot draws annotations on a PNG screenshot from a JSON spec (arrows, rectangles, ovals, blur, highlighter strokes, spotlights, numbered counters, text labels and lines), so an AI coding agent can mark up a screenshot for you in one command.

## What does it do?

You (or your agent) describe the annotations as JSON and render them onto the original screenshot:

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

`render` writes `annotated.png` (or just the part a top-level `crop` names) and prints JSON to stdout: the image size, the dpr used, the fully resolved `spec` (ids, seeds, counter numbers and pixel coordinates filled in) and any `warnings`. Two more commands help along the way:

- `markshot grid screenshot.png -o grid.png` overlays a labeled pixel grid, for reading coordinates. `--region X,Y,W,H` draws just that part at full size, still labeled in whole-image pixels, for a close look at a large screenshot or a rendered result.
- `markshot schema` prints the JSON schema of the spec, with every field documented.

## What does it look like?

Each image below is one `markshot render` of a real admin-console screenshot, cropped by the spec itself to the part that matters. Click a spec to see every value.

| What the spec does | Result |
|---|---|
| **Numbered steps.** Outline boxes with `"spotlight": true` dim everything else, and each badge uses `"angle": "auto"` to find a free side. `crop` keeps only this part of the page. ([spec](assets/examples/steps.json)) | <img src="assets/examples/steps.png" alt="Settings page with three boxed, numbered steps and the rest dimmed" width="480"> |
| **Hand-drawn ovals and a label.** `sketch` ovals circle two toggles, a box frames a button group, and a `text` label says what to do. ([spec](assets/examples/ovals.json)) | <img src="assets/examples/ovals.png" alt="Security settings panel with a boxed button group, two hand-drawn ovals, numbered badges and a label" width="480"> |
| **Redact before sharing.** `blur` pixelates the project ID while boxes and an oval number the steps. ([spec](assets/examples/redact.json)) | <img src="assets/examples/redact.png" alt="Billing dialog with the project ID pixelated and three numbered steps" width="360"> |
| **Highlight and point.** A `highlight` marks a setting, and a curved `arrow` with a `text` label points at a button. ([spec](assets/examples/highlight-arrow.json)) | <img src="assets/examples/highlight-arrow.png" alt="A highlighted checkbox label and a blue curved arrow labeled Invite people here pointing at Add users" width="480"> |

## Why use it instead of drawing on the image another way?

Because an agent can't drag a mouse, but it can write JSON, look at the result and fix one value:

- **Fix-by-editing.** The resolved `spec` in the output is complete, so changing one value and rendering again from the original changes only that annotation. Feeding the `spec` back unchanged gives the same image.
- **Deterministic.** The same input, spec and binary give the same PNG bytes on every supported platform; randomness such as hand-drawn jitter and mosaic blocks is seeded from each annotation's id.
- **Made for screenshots.** Thicknesses are in points and scale with the PNG's DPI, so annotations look the same size on a Retina capture as on a standard one. Rectangles, ovals and arrows can look hand-drawn, the highlighter blends by the brightness of what is under it, and badge digits switch between white and black to stay legible. The input's DPI and ICC profile are kept.
- **Clear failures.** An invalid spec exits with code 1, writes nothing, and prints `error: <path>: <message>`, e.g. `error: annotations[0].size: must be 0 to 7, got 9`.

## How do I install it?

One command installs the latest release for your machine and checks it against the release's `SHA256SUMS`.

On macOS (Apple silicon) or Linux (x86_64 or arm64, glibc 2.34+), into `~/.local/bin`:

```sh
curl -fsSL https://github.com/hypn4/markshot/releases/latest/download/install.sh | sh
```

On Windows (x86_64 or arm64), into `%LOCALAPPDATA%\Programs\markshot`, which it adds to your user PATH:

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/hypn4/markshot/releases/latest/download/install.ps1 | iex"
```

Set `MARKSHOT_VERSION=v0.1.0` to install a particular release, or `MARKSHOT_INSTALL_DIR` to put it elsewhere. Running the command again upgrades in place. Then check with `markshot --version`.

**Or download a binary yourself.** Each release on [GitHub Releases](https://github.com/hypn4/markshot/releases) has one self-contained executable per platform, a `SHA256SUMS` file and a build provenance attestation:

| Platform | Asset |
|---|---|
| macOS, Apple silicon | `markshot-aarch64-apple-darwin` |
| Linux x86_64 (glibc 2.34+) | `markshot-x86_64-unknown-linux-gnu` |
| Linux arm64 (glibc 2.34+) | `markshot-aarch64-unknown-linux-gnu` |
| Windows x86_64 | `markshot-x86_64-pc-windows-msvc.exe` |
| Windows arm64 | `markshot-aarch64-pc-windows-msvc.exe` |

Save it as `markshot` (`markshot.exe` on Windows) in a directory on your PATH, and on macOS and Linux make it executable (`chmod +x`). A binary downloaded with a browser on macOS is quarantined and blocked as unsigned; clear that with `xattr -d com.apple.quarantine markshot`. The Windows builds link the C runtime statically, so they need no Visual C++ redistributable. `gh attestation verify markshot --repo hypn4/markshot` confirms that GitHub Actions built a file from this repository.

**Or use `cargo binstall`**, which downloads the same release binary: `cargo binstall --git https://github.com/hypn4/markshot markshot`.

**Or build from source** with Rust 1.99 or later: `cargo install --git https://github.com/hypn4/markshot --locked` installs it into `~/.cargo/bin`. In a clone, `rust-toolchain.toml` selects Rust 1.99 and `cargo build --release --locked` builds `target/release/markshot`.

The release binary is about 3 MB (fat LTO, one codegen unit, stripped).

## How do I teach my AI agent to use it?

Install the bundled skill. [`skills/annotating-screenshots/`](skills/annotating-screenshots/SKILL.md) follows the [Agent Skills](https://agentskills.io/specification) standard, which Claude Code, Codex, GitHub Copilot, VS Code, Cursor, Gemini CLI and [other agents](https://agentskills.io/clients) load. Once installed, the agent picks it up whenever you ask it to annotate a screenshot; `markshot` itself must be on the agent's PATH.

**With the `skills` CLI** (needs Node.js 22.20+). Run it in your project, or add `-g` to install it for all your projects:

```sh
npx skills add hypn4/markshot --skill annotating-screenshots           # pick the agents interactively
npx skills add hypn4/markshot --skill annotating-screenshots -a claude-code -a codex -y
```

It copies the skill into `.agents/skills/`, the location agents share, and links it into the folders of agents that use their own (such as `.claude/skills/`). `npx skills update` and `npx skills remove annotating-screenshots` keep it current or take it out. From a local clone, use the clone's path instead of `hypn4/markshot`.

**Manually.** Copy the whole directory, `SKILL.md` and its `references/`, into a skills folder your agent scans:

| Scope | Shared location (most agents) | Claude Code |
|---|---|---|
| One project | `.agents/skills/annotating-screenshots/` | `.claude/skills/annotating-screenshots/` |
| All your projects | `~/.agents/skills/annotating-screenshots/` | `~/.claude/skills/annotating-screenshots/` |

```sh
mkdir -p ~/.agents/skills && cp -R skills/annotating-screenshots ~/.agents/skills/
```

Project skills take precedence over user skills with the same name. To confirm the agent sees it, list its skills (for example `/skills` in Claude Code or VS Code) or ask it which skills it has.

The skill holds the render-and-fix workflow, coordinates, a tool picker and common mistakes; its `references/` hold every tool with an example, the spec details and defaults, and what each warning means, which the agent reads only when it needs them.

## How is it developed and released?

[`CONTRIBUTING.md`](CONTRIBUTING.md) explains how to report bugs, propose changes and open pull requests. [`AGENTS.md`](AGENTS.md) holds the rules every change follows, for people and AI agents alike: the checks to run, the determinism and spec invariants, and the commit format.

- **CI** runs fmt, clippy and rustdoc once, then the whole test suite natively on all five release platforms. The golden images in `tests/golden/` pin the rendering byte for byte, so every platform must draw the same bytes.
- **Releases** start when a `v*` tag is pushed. Each platform builds natively and runs the tests with the shipped release profile; then one job publishes the binaries, `SHA256SUMS`, the attestation and release notes generated from the commits by git-cliff's GitHub template. No changelog file is kept in the repository.
- **The skill** is validated with `uvx --from skills-ref agentskills validate skills/annotating-screenshots`, and `cargo test` checks its frontmatter, links and examples. [`evals/annotating-screenshots/`](evals/annotating-screenshots/README.md) holds the agent scenarios it was tested with; rerun them after changing the skill.

[`docs/design.md`](docs/design.md) describes the design: the CLI, coordinates, defaults, every tool, the draw order, determinism and colour handling.

## License and notices

- markshot is licensed under the [MIT License](LICENSE).
- Counter labels are laid out with SF Pro's numeric metrics (ascender, descender, digit height and tracking). No SF Pro outlines are included. The digits are drawn with Inter.
- The embedded Inter fonts (`assets/fonts/Inter-Regular.ttf`, `assets/fonts/Inter-Medium.ttf`) are © The Inter Project Authors and licensed under the SIL Open Font License 1.1. See [`assets/fonts/OFL.txt`](assets/fonts/OFL.txt). The fonts inside the binary carry the same copyright and license notice.
