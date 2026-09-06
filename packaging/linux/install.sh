#!/usr/bin/env bash
# Installs an extracted release for the current user. Never downloads or runs as root.
set -euo pipefail
bundle_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
prefix=${HOME}/.local
if [[ ${1:-} == --prefix && $# == 2 ]]; then
    prefix=$2
elif [[ $# != 0 ]]; then
    echo "Usage: $0 [--prefix /absolute/path]" >&2
    exit 2
fi
if [[ $prefix != /* ]]; then
    echo 'Install prefix must be an absolute path.' >&2
    exit 2
fi
# These characters need multiple, different escaping rules in desktop and systemd files.
for path in "$prefix" "${XDG_CONFIG_HOME:-$HOME/.config}" "${XDG_DATA_HOME:-$HOME/.local/share}"; do
    if [[ $path == *['"\`$%']* || $path == *$'\n'* || $path == *$'\r'* ]]; then
        echo 'Installation paths cannot contain quotes, backslashes, backticks, $, %, or newlines.' >&2
        exit 2
    fi
done
[[ -x $bundle_dir/bin/smarty-pants && -x $bundle_dir/bin/smarty-pants-daemon ]]
# Verify platform compatibility before changing an existing installation.
"$bundle_dir/bin/smarty-pants" --version
"$bundle_dir/bin/smarty-pants-daemon" --version
data_dir=${XDG_DATA_HOME:-$HOME/.local/share}
config_dir=${XDG_CONFIG_HOME:-$HOME/.config}
install -d "$prefix/bin" "$data_dir/applications" "$data_dir/pixmaps" "$data_dir/smarty-pants" "$config_dir/systemd/user"
for bin in smarty-pants smarty-pants-daemon; do
    staged=$(mktemp "$prefix/bin/.$bin.XXXXXX")
    install -m 755 "$bundle_dir/bin/$bin" "$staged"
    mv -f -- "$staged" "$prefix/bin/$bin"
done
install -m 644 "$bundle_dir/share/pixmaps/computer.smarty-pants.png" "$data_dir/pixmaps/"
# Absolute Exec works even when a desktop session hasn't inherited ~/.local/bin.
while IFS= read -r line; do
    if [[ $line == Exec=* ]]; then
        printf 'Exec="%s/bin/smarty-pants" daemon start\n' "$prefix"
    else
        printf '%s\n' "$line"
    fi
done < "$bundle_dir/share/applications/computer.smarty-pants.desktop" > "$data_dir/applications/computer.smarty-pants.desktop"
install -m 755 "$bundle_dir/uninstall.sh" "$data_dir/smarty-pants/uninstall.sh"
unit_path=$config_dir/systemd/user/smarty-pants.service
unit_target=$unit_path
# Preserve hand-written service files and their drop-ins.
if [[ -e $unit_path ]] && [[ $(head -n 1 "$unit_path") != '# Managed by smarty-pants user installer' ]]; then
    unit_target=$unit_path.new
fi
{
    echo '# Managed by smarty-pants user installer'
    cat <<UNIT
[Unit]
Description=Smarty Pants writing assistant
After=graphical-session.target
PartOf=graphical-session.target

[Service]
Type=simple
ExecStart="$prefix/bin/smarty-pants-daemon"
Restart=on-failure
RestartSec=5
Nice=5

[Install]
WantedBy=graphical-session.target
UNIT
} > "$unit_target"
# API bundles must be usable on first launch without a local inference backend.
if [[ $(cat "$bundle_dir/FLAVOR") == api && ! -e $config_dir/smarty-pants/config.toml ]]; then
    install -d -m 700 "$config_dir/smarty-pants"
    install -m 600 "$bundle_dir/share/smarty-pants/deepseek.toml" "$config_dir/smarty-pants/config.toml"
fi
if command -v update-desktop-database >/dev/null; then
    update-desktop-database "$data_dir/applications" >/dev/null 2>&1 || true
fi
printf '\nInstalled in %s/bin. Open Smarty Pants from your application launcher.\n' "$prefix"
printf 'For terminal commands, add this directory to PATH: %s/bin\n' "$prefix"
if [[ $unit_target != "$unit_path" ]]; then
    printf 'Existing service preserved. Review %s and update ExecStart before restarting that service.\n' "$unit_target"
else
    printf 'Optional login startup: systemctl --user daemon-reload && systemctl --user enable --now smarty-pants.service\n'
fi
printf 'Uninstall: bash "%s/smarty-pants/uninstall.sh" --prefix "%s"\n' "$data_dir" "$prefix"
