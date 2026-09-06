#!/usr/bin/env bash
# Prepare locked sources for an offline RPM/OBS build. Requires a clean committed checkout.
set -euo pipefail
version=$(sed -n 's/^version *= *"\([^"]*\)"/\1/p' Cargo.toml | head -n 1)
output=${1:-target/rpm-sources}
if [[ -n $(git status --porcelain --untracked-files=normal) ]]; then
    echo 'Commit or stash changes before preparing a source release.' >&2
    exit 1
fi
mkdir -p "$output"
output=$(cd "$output" && pwd)
stage=$(mktemp -d)
trap 'rm -rf -- "$stage"' EXIT
git archive --format=tar.gz --prefix="smarty-pants-$version/" HEAD > "$output/smarty-pants-$version.tar.gz"
mkdir -p "$stage/.cargo"
# Generate portable relative paths inside the source tree, not paths to this temporary directory.
(cd "$stage" && cargo vendor --locked --versioned-dirs --manifest-path "$OLDPWD/Cargo.toml" vendor > .cargo/config.toml)
tar --zstd -cf "$output/vendor.tar.zst" -C "$stage" .cargo vendor
cp packaging/rpm/smarty-pants.spec "$output/"
printf 'RPM/OBS sources prepared in %s\n' "$output"
