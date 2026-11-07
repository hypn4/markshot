# Contributing to markshot

Bug reports, fixes and improvements are welcome. Small fixes can go straight to a pull request; anything that changes rendering, the spec or the CLI starts with an issue so the design is agreed before you write code. Everyone taking part follows the [Code of Conduct](CODE_OF_CONDUCT.md).

## What rules does a change have to follow?

The rules in [`AGENTS.md`](AGENTS.md): the commands to run, the invariants (byte-for-byte determinism, golden images, the spec and CLI contract), code and test conventions, commit format and boundaries. They apply to people and AI agents alike, so a change reads the same whoever made it. [`docs/design.md`](docs/design.md) describes how markshot behaves today.

## How do I report a bug?

Open a [bug report](https://github.com/hypn4/markshot/issues/new?template=bug_report.yml). The most useful report is one we can run: the `markshot --version` output, your platform, the exact command, the spec, and the input PNG or a smaller one that shows the same problem. markshot is deterministic, so the same input reproduces the bug anywhere.

Security problems go through [`SECURITY.md`](SECURITY.md), not public issues.

## How do I propose a feature or a rendering change?

Open a [feature request](https://github.com/hypn4/markshot/issues/new?template=feature_request.yml) that describes what you want to annotate and why the current tools can't do it. Spec fields, defaults, the `render` report and exit codes are a contract that users and agents rely on, and rendering is pinned by golden images, so these changes are agreed in the issue first.

## How do I make a pull request?

1. Fork, branch from `main`, and make one logical change per commit.
2. Run the checks from [`AGENTS.md`](AGENTS.md#commands); CI runs the same ones and tests on macOS, Linux and Windows.
3. Write commit messages and the pull request title as [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/), e.g. `fix: keep seeds within 53 bits`. Release notes are generated from them; there is no changelog to edit.
4. Fill in the pull request template. If golden images changed, say which and why; reviewers look at them.

The maintainer squash- or rebase-merges once CI passes and the review is done, keeping `main` linear.

## Can I use an AI coding agent?

Yes. Codex, Copilot, Cursor and most other coding agents read `AGENTS.md` on their own, and Claude Code reads it through `CLAUDE.md`, so they pick up the same rules. You are still the author: you must understand every line you submit, have run the checks yourself, and be able to answer review questions about it.

## How are my contributions licensed?

markshot is [MIT licensed](LICENSE). By submitting a contribution, you agree that it is licensed under the same terms.
