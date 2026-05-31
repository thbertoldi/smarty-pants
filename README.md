# smarty-pants

Local AI writing assistant for Wayland. Highlight text in any app, press a hotkey, get it rewritten in place — no cloud, no telemetry. Three modes ship by default: **general** grammar/fluency fix, **LinkedIn** voice, and **academic** voice.

Runs as a long-lived daemon that keeps a Qwen 2.5 7B Instruct model resident in GPU VRAM and serves paraphrase requests both via the XDG GlobalShortcuts portal (Hyprland) and via a Unix-socket CLI (niri / Sway / anything else with `bindsym`-style hotkeys).

## Requirements

Hardware: a Vulkan-capable GPU with ≥ 6 GB VRAM. Tested on NVIDIA RTX 1000 Ada (6 GB). Should work on Intel Arc, recent AMD, etc. Falls back to CPU automatically if no GPU is detected (slow but functional).

Software (openSUSE Tumbleweed package names):

```sh
sudo zypper install \
    cmake clang libclang13 \
    vulkan-headers libvulkan1 glslang-devel shaderc \
    wtype wl-clipboard \
    xdg-desktop-portal-hyprland   # only for Hyprland
```

The build is heavy because `llama-cpp-2` compiles llama.cpp from source with the Vulkan backend (~3–5 minutes the first time).

## Build & install

```sh
git clone <this-repo> smarty-pants && cd smarty-pants
cargo install --path crates/cli    --locked
cargo install --path crates/daemon --locked
```

`smarty-pants` (CLI, sub-100 ms cold start) and `smarty-pants-daemon` (long-lived) both land in `~/.cargo/bin/`. Make sure that's on your PATH:

```fish
fish_add_path ~/.cargo/bin
```

## First run

```sh
smarty-pants-daemon
```

On first start the daemon downloads the Qwen 2.5 7B Instruct Q4_K_M GGUF (~4.4 GB) to `~/.local/share/smarty-pants/models/` and SHA-256 verifies it. Subsequent starts skip the download.

Watch for the line `portal shortcuts bound count=3` — that's the signal the GlobalShortcuts portal accepted the three mode registrations.

## Configure shortcuts

### Hyprland

The daemon registers three shortcut ids with the portal:

| Shortcut id                  | Mode                          | Suggested key combo |
| ---------------------------- | ----------------------------- | ------------------- |
| `surface-transient:rewrite`  | general grammar / fluency fix | `Super+R`           |
| `surface-transient:academic` | academic voice                | `Super+A`           |
| `surface-transient:linkedin` | LinkedIn voice                | `Super+I`           |

(`Super+Shift+L` is a common `movewindow` bind in stock Hyprland configs, hence `Super+I` for LinkedIn — pick whatever you have free.)

Verify with:

```sh
hyprctl globalshortcuts
```

Hyprland's portal doesn't have a GUI for binding shortcuts — you write `bind = …, global, <id>` lines in your compositor config. Add to `~/.config/hypr/hyprland.conf` (or wherever your binds live):

```
bind = SUPER, R, global, surface-transient:rewrite
bind = SUPER, A, global, surface-transient:academic
bind = SUPER, I, global, surface-transient:linkedin
```

Then reload:

```sh
hyprctl reload
```

### niri / Sway / others

Bind each mode to a different hotkey that runs `smarty-pants trigger --mode <name>`:

```kdl
# ~/.config/niri/config.kdl
binds {
    Mod+R { spawn "smarty-pants" "trigger" "--mode" "rewrite"; }
    Mod+A { spawn "smarty-pants" "trigger" "--mode" "academic"; }
    Mod+I { spawn "smarty-pants" "trigger" "--mode" "linkedin"; }
}
```

```
# ~/.config/sway/config
bindsym $mod+R exec smarty-pants trigger --mode rewrite
bindsym $mod+A exec smarty-pants trigger --mode academic
bindsym $mod+I exec smarty-pants trigger --mode linkedin
```

## Auto-start at login (systemd user unit)

```sh
mkdir -p ~/.config/systemd/user
cp packaging/systemd/smarty-pants.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now smarty-pants.service
```

Inspect logs with:

```sh
journalctl --user -u smarty-pants -f
```

## Usage

1. Highlight some text in any window (the **primary selection** — i.e. just mouse-highlighting is enough; you don't need to `Ctrl+C` it).
2. Keep focus on the target window where the rewrite should land.
3. Press the hotkey for the mode you want (`Super+R` for general rewrite, `Super+A` for academic, `Super+I` for LinkedIn).
4. After a brief wait — typically 0.5–2 s once the model is warm; up to ~10 s on the first call after daemon start because llama.cpp builds the inference context — the highlighted text is replaced in place with the improved version.

The paraphrase also lands on your system clipboard, so if focus shifted during the wait you can manually `Ctrl+Shift+V` (terminal) / `Ctrl+V` (GUI) to paste it wherever you actually meant.

The daemon auto-detects terminal windows (Ghostty, kitty, foot, Alacritty, WezTerm, gnome-terminal, Konsole, …) and uses `Ctrl+Shift+V` for them; everything else gets `Ctrl+V`.

## Customize

Optional config at `~/.config/smarty-pants/config.toml`. Defaults are sensible; override anything you don't like:

```toml
[model]
gpu_layers = -1     # -1 = offload all if GPU present, 0 = force CPU
temperature = 0.7

[inject]
restore_clipboard = false   # default; set to true to keep your prior clipboard
paste_settle_ms   = 200     # bump higher if TUIs swallow the paste

[modes.rewrite]
# Override the built-in rewrite prompt
system = """
… your own prompt …
"""
shortcut = "SUPER+R"
description = "My custom mode"
```

## Status & lifecycle

```sh
smarty-pants status                  # is the daemon up?
smarty-pants daemon start            # spawn detached daemon
smarty-pants daemon stop             # send shutdown via socket (Phase 2: clean exit)
pkill smarty-pants-daemon            # universal stop
```

## Known limitations

- **First paraphrase after daemon start is slow** (~7–10 s). llama.cpp rebuilds the per-call context every time. Phase 2 will hoist the context across calls and drop typical latency to sub-second.
- **TUI applications inside terminals (Claude Code, helix, lazygit, …) sometimes swallow synthesized paste.** Workaround is built-in: the paraphrase stays on the clipboard, so you can manually paste with `Ctrl+Shift+V`.
- **Qwen's safety filter is mild but not zero.** Inputs with explicit profanity occasionally trigger a refusal instead of a rewrite. Swap to another GGUF by editing `crates/daemon/src/model_download.rs` and recompiling.
- **Hyprland's portal-shortcut id namespace is `surface-transient`** — that's xdg-desktop-portal-hyprland's fallback when an app doesn't pass a `WindowIdentifier`. Cosmetic; functional.
- **Only tested on Hyprland + wlroots-family compositors.** KDE Plasma 6 has portal support but uses different paste-chain quirks (Phase 3).

## License

[MPL-2.0](LICENSE).
