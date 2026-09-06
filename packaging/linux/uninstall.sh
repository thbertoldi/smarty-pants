#!/usr/bin/env bash
set -euo pipefail
prefix=${HOME}/.local
if [[ ${1:-} == --prefix && $# == 2 && $2 == /* ]]; then
    prefix=$2
elif [[ $# != 0 ]]; then
    echo "Usage: $0 [--prefix /absolute/path]" >&2
    exit 2
fi
data_dir=${XDG_DATA_HOME:-$HOME/.local/share}
unit_path=${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user/smarty-pants.service
if [[ -f $unit_path ]] && [[ $(head -n 1 "$unit_path") == '# Managed by smarty-pants user installer' ]]; then
    if command -v systemctl >/dev/null; then
        systemctl --user disable --now smarty-pants.service 2>/dev/null || true
    fi
    rm -f -- "$unit_path"
fi
rm -f -- "$prefix/bin/smarty-pants" "$prefix/bin/smarty-pants-daemon" \
    "$data_dir/applications/computer.smarty-pants.desktop" \
    "$data_dir/pixmaps/computer.smarty-pants.png" \
    "$data_dir/smarty-pants/uninstall.sh"
if command -v systemctl >/dev/null; then systemctl --user daemon-reload 2>/dev/null || true; fi
echo 'Application files removed. Configuration, API keys and downloaded models were kept.'
echo 'Quit any separately started tray process to finish uninstalling.'
