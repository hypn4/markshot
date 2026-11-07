## What and why

<!-- What does this change, and why? Link the issue: "Closes #123". -->

## Contract and rendering

<!-- Delete the lines that don't apply. -->
- Spec, defaults, `render` report or exit codes change: <!-- how, and whether it breaks existing specs -->
- Golden images regenerated: <!-- which ones, and why the new pixels are right -->
- New dependency: <!-- which, and why std or an existing one can't do it -->

## Checklist

- [ ] The title and commits follow Conventional Commits (`type: summary`).
- [ ] I ran `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, rustdoc with `-D warnings` and `cargo test --locked`.
- [ ] New behaviour or a fixed bug has a test that fails without this change.
- [ ] `docs/design.md`, the README and the skill (`skills/annotating-screenshots/`) match the new behaviour, or nothing user-visible changed.
- [ ] I understand every line of this change, including any an AI agent wrote.
