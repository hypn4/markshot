# Security policy

## Which versions get security fixes?

Only the latest release. Fixes ship as a new release; older versions are not patched.

## How do I report a vulnerability?

Report it privately through GitHub: on the repository's **Security** tab, choose **Report a vulnerability** ([direct link](https://github.com/hypn4/markshot/security/advisories/new)). Please don't open a public issue. Include the markshot version, your platform, and the input PNG, spec and command that trigger the problem.

You can expect an acknowledgement within a week. Once a fix is released, the advisory is published with credit to you unless you prefer otherwise.

## What counts as a vulnerability?

markshot reads untrusted PNG files and JSON specs, often on behalf of an AI agent. In scope:

- a crafted PNG or spec that crashes markshot, makes it run without bound or use unbounded memory despite the input limits in [`docs/design.md`](docs/design.md#input-bounds);
- writing anywhere other than the `--out` path;
- a release binary that does not match its `SHA256SUMS` entry or build provenance attestation.

The PNG decoder and the renderer are fuzzed every week ([`fuzz/`](fuzz/)) to look for the first kind.

A wrong-looking annotation is a bug; please use a [bug report](https://github.com/hypn4/markshot/issues/new?template=bug_report.yml).

## How can I check that a binary is genuine?

Each release asset has a build provenance attestation:

```sh
gh attestation verify markshot-aarch64-apple-darwin --repo hypn4/markshot
```
