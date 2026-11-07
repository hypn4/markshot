#!/bin/sh
# Installs the markshot release binary for this machine (macOS on Apple silicon, Linux on
# x86_64 or arm64 with glibc) and checks it against the release's SHA256SUMS.
#
#   curl -fsSL https://github.com/hypn4/markshot/releases/latest/download/install.sh | sh
#
# Environment:
#   MARKSHOT_VERSION      release tag to install, e.g. v0.1.0 (default: the latest release)
#   MARKSHOT_INSTALL_DIR  where to put the binary (default: ~/.local/bin)
#   MARKSHOT_DOWNLOAD_URL base URL holding the assets, overriding the GitHub release (for tests)
set -eu

repo="hypn4/markshot"
version="${MARKSHOT_VERSION:-latest}"
dir="${MARKSHOT_INSTALL_DIR:-$HOME/.local/bin}"

fail() {
    echo "markshot install: $*" >&2
    exit 1
}

os=$(uname -s)
arch=$(uname -m)
case "$os/$arch" in
    Darwin/arm64) target=aarch64-apple-darwin ;;
    Darwin/x86_64)
        if [ "$(sysctl -n sysctl.proc_translated 2>/dev/null || echo 0)" = 1 ]; then
            target=aarch64-apple-darwin # a Rosetta shell on Apple silicon
        else
            fail "macOS on Intel has no release binary; build from source: cargo install --git https://github.com/$repo --locked"
        fi
        ;;
    Linux/x86_64 | Linux/amd64) target=x86_64-unknown-linux-gnu ;;
    Linux/aarch64 | Linux/arm64) target=aarch64-unknown-linux-gnu ;;
    *) fail "no release binary for $os $arch; build from source: cargo install --git https://github.com/$repo --locked" ;;
esac
if [ "$os" = Linux ] && ldd --version 2>&1 | grep -qi musl; then
    fail "the Linux binaries need glibc 2.34 or later, not musl; build from source: cargo install --git https://github.com/$repo --locked"
fi

if [ -n "${MARKSHOT_DOWNLOAD_URL:-}" ]; then
    base="$MARKSHOT_DOWNLOAD_URL"
elif [ "$version" = latest ]; then
    base="https://github.com/$repo/releases/latest/download"
else
    base="https://github.com/$repo/releases/download/$version"
fi

fetch() {
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL --retry 3 -o "$2" "$1" || fail "could not download $1"
    elif command -v wget >/dev/null 2>&1; then
        wget -q -O "$2" "$1" || fail "could not download $1"
    else
        fail "needs curl or wget"
    fi
}

sha256() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | cut -d ' ' -f 1
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$1" | cut -d ' ' -f 1
    else
        fail "needs sha256sum or shasum to check the download"
    fi
}

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT INT TERM
asset="markshot-$target"
echo "Downloading $asset ($version)"
fetch "$base/$asset" "$tmp/$asset"
fetch "$base/SHA256SUMS" "$tmp/SHA256SUMS"
want=$(awk -v name="$asset" '$2 == name || $2 == "*" name { print $1 }' "$tmp/SHA256SUMS")
[ -n "$want" ] || fail "SHA256SUMS has no entry for $asset"
got=$(sha256 "$tmp/$asset")
[ "$got" = "$want" ] || fail "checksum mismatch for $asset: expected $want, got $got"

mkdir -p "$dir"
chmod 755 "$tmp/$asset"
mv "$tmp/$asset" "$dir/markshot"
echo "Installed $("$dir/markshot" --version) to $dir/markshot"
case ":$PATH:" in
    *":$dir:"*) ;;
    *) echo "Add $dir to your PATH, e.g. in your shell profile: export PATH=\"$dir:\$PATH\"" ;;
esac
