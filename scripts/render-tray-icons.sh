#!/usr/bin/env bash
# Maintainer-only exports. Building and running the app needs no image tools.
set -euo pipefail
repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
source_image=$repo_root/docs/assets/tray-icon.png
output_dir=$repo_root/crates/daemon/assets/tray
mkdir -p "$output_dir"
for size in 16 20 22 24 32 48 64; do
    magick "$source_image" -resize "${size}x${size}" -strip -depth 8 "RGBA:$output_dir/icon-$size.rgba"
done
magick "$source_image" -resize 256x256 -strip "$repo_root/docs/assets/app-icon.png"
