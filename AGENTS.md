# AGENTS.md

markshot is a Rust CLI that draws screenshot annotations (arrows, boxes, ovals, blur, highlights, spotlights, counters, text labels, lines) from a JSON spec, byte for byte the same on every platform, for AI coding agents. This file holds the rules for everyone who changes the repository, agents and people alike. [`CONTRIBUTING.md`](CONTRIBUTING.md) covers the human process (issues, pull requests, review). [`docs/design.md`](docs/design.md) is the behaviour reference.

## Commands

`rust-toolchain.toml` pins Rust 1.99 with clippy and rustfmt. Run every check CI runs before you call a change done:

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps --document-private-items
cargo test --locked
```

- Release profile (what ships): `cargo test --release --locked`.
- One test file or test: `cargo test --test golden`, `cargo test --test resolve counter_`.
- Skill changes: `uvx --from skills-ref agentskills validate skills/annotating-screenshots`.
- Try the CLI: `cargo run -- schema`, `cargo run -- grid in.png -o grid.png`, `cargo run -- render in.png --spec spec.json -o out.png`.

## Where things live

| Path | What |
|---|---|
| `src/main.rs` | CLI (`render`, `schema`, `grid`), exit codes |
| `src/spec.rs` | Spec model and JSON Schema |
| `src/resolve.rs` | Defaults, coordinate mapping, bounds, ids and seeds: spec to resolved spec |
| `src/render.rs` | Draw order and the render report |
| `src/tools/` | One module per tool; `src/stroke/` is the shared freehand stroke engine |
| `src/canvas.rs`, `src/text.rs`, `src/color.rs`, `src/image_io.rs` | Y-up canvas, embedded Inter text, colour helpers, PNG in and out |
| `src/math.rs`, `src/rng.rs` | Platform-independent float functions and the seeded generator |
| `tests/` | CLI, resolver, golden-image, determinism and skill-example tests |
| `skills/annotating-screenshots/` | The Agent Skill shipped to users; `evals/` holds its scenarios |
| `.github/workflows/` | `ci.yml` (lint once, test on five native platforms), `release.yml` (`v*` tags) |

## Invariants

Breaking one of these is a bug even when every test passes.

- **Determinism.** One input gives one output, byte for byte, on macOS, Linux and Windows, arm64 and x86_64.
  - Transcendental float functions go through `markshot::math` (backed by `libm`), never `f64::sin` and friends; `clippy.toml` rejects the direct calls.
  - Randomness comes only from `rng`, seeded per annotation from its id. No clocks, environment or address-dependent values in output, and no `HashMap`/`HashSet` iteration order reaching output.
  - Keep tiny-skia on `default-features = false, features = ["no-std-float"]`.
  - `PROBE_SHA` in `tests/determinism.rs` changes only with a tiny-skia upgrade, and only once CI shows the same hash on all five platforms.
- **Golden images.** `tests/golden/*.png` pin rendering. Never regenerate them to make a failing test pass. For an intended rendering change, run `UPDATE_GOLDEN=1 cargo test --test golden` (or `--test cli_grid`), look at every changed image, and say in the commit body which images changed and why.
- **Calibrated data.** Arrow outlines, counter tails, SF Pro numeric metrics and `math::PI_TRUNCATED` are measured values. Do not round, tidy or "correct" them.
- **Spec and CLI contract.** Users and agents depend on the spec fields, defaults, the `render` report (`width`, `height`, `dpr`, `spec`, `warnings`) and the exit codes: 0 success, 1 any input error with `error: <path>: <message>` on stderr and nothing written. A resolved `spec` fed back must render the same image. A change to any of these is a breaking change unless it only adds an optional field.
- **Native CI only.** Builds and tests run on native GitHub runners. No QEMU, Docker emulation or cross-compiling test runs.

## Code

- rustfmt defaults; clippy `pedantic` is on and warnings fail CI.
- Silence a lint only with `#[expect(lint, reason = "...")]` on the narrowest item. Never `#[allow]`, never a crate-wide opt-out. The one exception is `tests/common/mod.rs`, whose helpers each test crate uses only in part, so `dead_code` fires in some crates and not others and `#[expect]` cannot work there.
- Doc comments state what an item does in the present tense, with units (px, pt, y-up) where they matter. Comments describe behaviour, not history or where an idea came from.
- No `unwrap`/`expect` on input-derived values outside tests; return `Error::Spec` with the path of the bad value (e.g. `annotations[2].size`).
- Add a dependency only when the standard library or an existing dependency cannot do the job, and say why in the commit body. `Cargo.lock` is committed; use `--locked`.

## Tests

- A behaviour change comes with a test that fails without it; a bug fix comes with a regression test.
- Tests are deterministic and offline. Prefer exact expected values with a comment deriving them over loose tolerances.
- Do not test wiring, copies of the implementation or incidental defaults.
- `tests/skill_examples.rs` renders every `json` block in the skill; keep the examples valid.

## Docs

- Repository text is English. Prose punctuation stays ASCII: no em dash (U+2014) and no middle dot (U+00B7); write products as `a * b` inside code spans. CI rejects both characters.
- When behaviour changes, update in the same change: `docs/design.md`, the README if users see it, and `skills/annotating-screenshots/` (`SKILL.md`, `references/`) if agents use it. Rerun the evals in `evals/annotating-screenshots/` after changing the skill's instructions.
- The README answers reader questions (What, Why, How); keep new sections in that form.
- There is no changelog file. Release notes are generated from commit subjects, so subjects must make sense to users.
- The skill states the oldest CLI it works with (`compatibility` in `SKILL.md`). When the skill starts using a new spec field or option, raise that version to the release that adds it and bump the skill's `metadata.version`.

## Commits and pull requests

- [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/): `type: summary`, where the summary says what changed in words a user understands, has no trailing period and stays within about 72 characters. Types: `feat`, `fix`, `perf`, `refactor`, `docs`, `test`, `ci`, `build`, `chore`. Breaking changes use `type!:` and a `BREAKING CHANGE:` footer.
- The body says why, and names changed golden images, new dependencies and spec or CLI changes.
- One logical change per commit, and every commit builds and passes `cargo test`. `main` has linear history: pull requests are squash- or rebase-merged, and the pull request title follows the same commit format.
- Pull requests fill in [the template](.github/pull_request_template.md). Behaviour or spec changes start with an issue.

## Releases

Maintainers only. Versions follow SemVer; before 1.0, a breaking spec or CLI change bumps the minor version.

1. Bump `version` in `Cargo.toml`, run `cargo check` to update `Cargo.lock`, and commit `chore: release vX.Y.Z`.
2. Tag the commit `vX.Y.Z` and push the tag. `release.yml` tests and builds the five binaries natively, then publishes them with `SHA256SUMS`, a build provenance attestation and git-cliff release notes.
3. A new platform goes into the matrices of both `ci.yml` and `release.yml` and the README's asset table together.

## CI and supply chain

- Pin every action to a full commit SHA with a `# vX.Y.Z` comment. Dependabot proposes updates weekly after a seven-day cooldown.
- Give each workflow and job the least `permissions` it needs, and check out with `persist-credentials: false`.
- Release builds use no cache.

## Boundaries

- **Always:** run the commands above before declaring a change done, and report what you ran.
- **Ask first:** new dependencies, spec or CLI contract changes, regenerated golden images, workflow changes, edits to calibrated data.
- **Never:** push, tag, publish or rewrite shared history unless a maintainer asks; commit secrets, local tool state (`.omp/`, `.superpowers/`, `CLAUDE.local.md`) or build output; bypass hooks or CI checks; weaken a test to make it pass.
