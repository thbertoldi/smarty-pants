# Install Smarty Pants

## Downloaded binaries

Release binaries support Linux x86_64 with glibc 2.35 or newer. Check with `uname -m` and `ldd --version`. Hyprland is the primary tested desktop; see [compositor limitations](limitations.md).

Install desktop dependencies:

```sh
# openSUSE Tumbleweed
sudo zypper install wl-clipboard wtype zenity xdg-utils
```

```sh
# Ubuntu 22.04+ / Debian with a sufficiently recent glibc
sudo apt install wl-clipboard wtype zenity xdg-utils
```

A StatusNotifierItem host is needed to display the tray (for example, Waybar with its tray module enabled). Zenity provides API settings and review dialogs. Install your compositor's `xdg-desktop-portal` backend if you use portal shortcuts.

Download an archive and matching `.sha256` from [GitHub Releases](https://github.com/thbertoldi/smarty-pants/releases/latest). Pick `api` for DeepSeek/compatible APIs, or `cpu` for local inference plus APIs. The CPU build uses a generic x86_64 target; it does not require Vulkan or a GPU.

```sh
sha256sum -c smarty-pants-0.2.0-linux-x86_64-api.tar.gz.sha256
tar -xzf smarty-pants-0.2.0-linux-x86_64-api.tar.gz
bash smarty-pants-0.2.0-linux-x86_64-api/install.sh
```

Use `cpu` in those filenames for the other variant. The installer verifies the binaries run before copying them. It installs:

| File | Default location |
| --- | --- |
| CLI and daemon | `~/.local/bin/` |
| Launcher and icon | `$XDG_DATA_HOME/applications/` and `$XDG_DATA_HOME/pixmaps/` |
| Optional user service | `$XDG_CONFIG_HOME/systemd/user/smarty-pants.service` |
| Uninstaller | `$XDG_DATA_HOME/smarty-pants/uninstall.sh` |

XDG defaults are `~/.config` and `~/.local/share`. `install.sh --prefix /absolute/path` changes the binary prefix. The installer supports spaces and Unicode in paths, but rejects quoting/control characters that cannot be safely inserted into both desktop and service files.

Add `~/.local/bin` to your shell and compositor session PATH for CLI shortcuts:

```sh
export PATH="$HOME/.local/bin:$PATH"
```

Open **Smarty Pants** from your application launcher, or run `smarty-pants daemon start`. A fresh API installation writes a minimal DeepSeek config if none exists; enter a key through **Tray → Set API key…**. It preserves an existing config, so an API bundle replacing a local build needs `inference.provider = "deepseek"` before startup. A CPU installation defaults to Qwen 2.5 1.5B and downloads its weights on the first rewrite.

## Upgrades and login startup

Run the new bundle's installer to replace binaries. Existing configuration, keys, models and service drop-ins are preserved. A custom user service is left intact; the installer writes a suggested `smarty-pants.service.new` alongside it. Review its `ExecStart` if migrating from `~/.cargo/bin` to `~/.local/bin`, and check existing drop-ins:

```sh
systemctl --user cat smarty-pants.service
systemctl --user show smarty-pants.service --property=ExecStart
```

Quit a manually started tray process, or stop the old user service, before launching the new version. Starting the CLI again reuses a running daemon; it does not upgrade an already running process.

For optional systemd login startup, run from your graphical session after confirming the service points to the correct binary:

```sh
systemctl --user import-environment WAYLAND_DISPLAY XDG_CURRENT_DESKTOP XDG_SESSION_TYPE
# Import only the compositor variable present in this session:
# systemctl --user import-environment HYPRLAND_INSTANCE_SIGNATURE
# systemctl --user import-environment SWAYSOCK
# systemctl --user import-environment NIRI_SOCKET
systemctl --user daemon-reload
systemctl --user enable --now smarty-pants.service
smarty-pants status
```

The unit is attached to `graphical-session.target`. Some minimal compositor sessions do not activate that target or import environment automatically; use the compositor's startup configuration in that case. Never run both a systemd service and a separate compositor autostart command. Do not import API keys into the service environment when using the tray's private key file.

On subsequent upgrades, use `systemctl --user restart smarty-pants.service`. Logs are in `journalctl --user -u smarty-pants.service -f`; detached CLI launches log to `$XDG_STATE_HOME/smarty-pants/daemon.log` (default `~/.local/state/...`).

## Keyboard shortcuts

Use unused key combinations. Running the trigger from a terminal normally focuses the terminal, so bind it to a compositor shortcut to rewrite text in another application. Absolute binary paths avoid session PATH issues.

### Hyprland

For direct CLI bindings, add to the relevant Hyprland config (classic syntax):

```ini
bind = SUPER, R, exec, ~/.local/bin/smarty-pants trigger --mode rewrite
bind = SUPER, A, exec, ~/.local/bin/smarty-pants trigger --mode academic
bind = SUPER, I, exec, ~/.local/bin/smarty-pants trigger --mode linkedin
bind = SUPER, K, exec, ~/.local/bin/smarty-pants trigger --mode condense
```

Adapt to your Hyprland version's config syntax, then reload. These bindings also work with `[shortcuts] enabled = false`. Portal bindings are another option: run `hyprctl globalshortcuts`, then bind the **actual IDs returned** using the `global` dispatcher. Some portal versions use `surface-transient:rewrite` etc.; do not assume that namespace without checking.

### Sway

```ini
bindsym $mod+r exec ~/.local/bin/smarty-pants trigger --mode rewrite
bindsym $mod+a exec ~/.local/bin/smarty-pants trigger --mode academic
bindsym $mod+i exec ~/.local/bin/smarty-pants trigger --mode linkedin
bindsym $mod+k exec ~/.local/bin/smarty-pants trigger --mode condense
```

### niri

Merge entries into your existing `binds` block; replace the binary path with your actual home directory:

```kdl
binds {
    Mod+R { spawn "/home/YOUR_USER/.local/bin/smarty-pants" "trigger" "--mode" "rewrite"; }
    Mod+A { spawn "/home/YOUR_USER/.local/bin/smarty-pants" "trigger" "--mode" "academic"; }
    Mod+I { spawn "/home/YOUR_USER/.local/bin/smarty-pants" "trigger" "--mode" "linkedin"; }
    Mod+K { spawn "/home/YOUR_USER/.local/bin/smarty-pants" "trigger" "--mode" "condense"; }
}
```

## Build from source

Use Rust 1.88+ through rustup; 1.91.1 is used for release builds. On openSUSE:

```sh
sudo zypper install rustup gcc make pkgconf-pkg-config wl-clipboard wtype zenity xdg-utils
rustup toolchain install 1.91.1 --component rustfmt,clippy
git clone https://github.com/thbertoldi/smarty-pants.git
cd smarty-pants
cargo +1.91.1 install --path crates/cli --locked
cargo +1.91.1 install --path crates/daemon --locked --no-default-features --features tray
```

Those commands install the **API** build in `~/.cargo/bin`. Configure DeepSeek **before starting it**: copy [examples/deepseek.toml](../examples/deepseek.toml) to `~/.config/smarty-pants/config.toml` only if no config exists. Preserve an existing config and edit its provider instead. See the [agent handoff](install-deepseek.md) for the complete migration procedure.

For local CPU builds, install `gcc-c++ cmake clang libclang13` and change daemon features to `--no-default-features --features local,tray`.

The default source build includes **Vulkan**: also install `vulkan-devel vulkan-headers libvulkan1 glslang-devel shaderc`, then omit `--no-default-features --features tray`. CUDA and ROCm builds use `--no-default-features --features cuda,tray` or `rocm,tray`, with their respective development toolchains. Local builds compile llama.cpp and may take several minutes.

Run Cargo jobs sequentially. For a source installation, [the example user unit](../packaging/systemd/smarty-pants.service) points to `~/.cargo/bin/smarty-pants-daemon`; do not overwrite an existing unit blindly.

## Uninstall

Quit Smarty Pants, then run the installed uninstaller (with `--prefix` if you selected a custom prefix):

```sh
bash "${XDG_DATA_HOME:-$HOME/.local/share}/smarty-pants/uninstall.sh"
```

It removes installed application files and its managed user service. Custom service files are preserved. Configuration, API keys and downloaded models remain under the Smarty Pants XDG directories for reuse; delete those separately only if you intend to discard them. Source installations can use `cargo uninstall smarty-pants-cli smarty-pants-daemon` and remove their user service manually.
