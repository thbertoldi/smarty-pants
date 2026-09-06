#!/usr/bin/env bash
# Bundle previously built binaries. Invoke from the repository root.
set -euo pipefail
flavor=${1:?Usage: scripts/package-release.sh cpu|api [binary-directory] [output-directory]}
[[ $flavor == cpu || $flavor == api ]] || exit 2
binary_dir=${2:-target/release}
output_dir=${3:-target/dist}
version=$(sed -n 's/^version *= *"\([^"]*\)"/\1/p' Cargo.toml | head -n 1)
arch=$(uname -m)
[[ $arch == x86_64 ]] || { echo 'Release bundles currently support x86_64 only.' >&2; exit 1; }
name=smarty-pants-$version-linux-$arch-$flavor
mkdir -p "$output_dir"
stage=$(mktemp -d)
trap 'rm -rf -- "$stage"' EXIT
root=$stage/$name
install -d "$root/bin" "$root/share/applications" "$root/share/pixmaps" "$root/share/smarty-pants"
install -m 755 "$binary_dir/smarty-pants" "$binary_dir/smarty-pants-daemon" "$root/bin/"
install -m 755 packaging/linux/install.sh packaging/linux/uninstall.sh "$root/"
install -m 644 packaging/linux/computer.smarty-pants.desktop "$root/share/applications/"
install -m 644 docs/assets/app-icon.png "$root/share/pixmaps/computer.smarty-pants.png"
install -m 644 examples/deepseek.toml "$root/share/smarty-pants/"
install -m 644 LICENSE packaging/linux/README.md "$root/"
printf '%s\n' "$flavor" > "$root/FLAVOR"
printf 'Source: https://github.com/thbertoldi/smarty-pants\nCommit: %s\n' "$(git rev-parse HEAD)" > "$root/SOURCE"
# Include resolved Cargo dependency licenses, including native llama.cpp sources.
python3 scripts/bundle-licenses.py "$flavor" "$root/licenses"
tar -czf "$output_dir/$name.tar.gz" -C "$stage" "$name"
(cd "$output_dir" && sha256sum "$name.tar.gz" > "$name.tar.gz.sha256")
printf '%s\n' "$output_dir/$name.tar.gz"
