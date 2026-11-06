#!/usr/bin/env bash
# Run the whole test suite on linux/arm64 and linux/amd64 via docker. Its golden images and
# determinism probe hold PNG and pixel bytes recorded on macOS, so passing means the platforms
# draw the same bytes. The repository is mounted read-only; build output stays inside the
# container.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
image="${RUST_IMAGE:-rust:1.99}"
status=0

for platform in linux/arm64 linux/amd64; do
    echo "==> ${platform} (${image})"
    if docker run --rm --platform "${platform}" \
        --volume "${repo_root}:/src:ro" \
        --workdir /src \
        --env CARGO_TARGET_DIR=/tmp/target \
        "${image}" \
        cargo test --locked; then
        echo "==> ${platform}: PASS"
    else
        echo "==> ${platform}: FAIL"
        status=1
    fi
done

exit "${status}"
