# smarty-pants

Local AI paraphraser for Wayland. Highlight text in any app, press a hotkey, get it rewritten in place — no cloud, no telemetry.

## Status

Phase 1 (MVP, work in progress). Supports Hyprland (XDG GlobalShortcuts portal) and niri / Sway (compositor-config hotkey + CLI trigger). Single `rewrite` mode. Gemma 3 1B Q4 model auto-downloaded on first run.

The daemon's real LLM wiring (`LlamaLlm` + main loop) requires `clang` and `glslang-devel` and will be enabled once those system deps are installed. Until then the daemon binary is a placeholder; all other layers (workspace, core, daemon library, CLI, E2E socket test with the EchoLlm stub) are complete and passing.

## Build

You need:

- Rust ≥ 1.75 (stable)
- `cmake`, `clang`, `pkg-config`
- Vulkan loader + headers: `vulkan-loader`, `vulkan-headers`, `glslang`
- `wtype` and `wl-clipboard` at runtime

On openSUSE Tumbleweed:

```sh
sudo zypper install cmake clang pkg-config vulkan-headers libvulkan1 glslang-devel wtype wl-clipboard
```

Then:

```sh
cargo install --path crates/cli    --locked
cargo install --path crates/daemon --locked
```

## Configure

Optional. Defaults work for the `rewrite` mode. To customize, copy `examples/config.toml` to `~/.config/smarty-pants/config.toml` and edit.

## Run

```sh
smarty-pants daemon start
```

On first start the daemon downloads the Gemma 3 1B Q4 model (~800 MB) to `~/.local/share/smarty-pants/models/` and verifies SHA-256.

### Hyprland

The daemon registers a portal shortcut named `rewrite` at startup. Open Hyprland's shortcut settings UI (Settings → Keyboard → App shortcuts) and bind it to whatever key combo you like (suggested default: `SUPER+SHIFT+P`).

### niri

Add to `~/.config/niri/config.kdl`:

```kdl
binds {
    Mod+Shift+P { spawn "smarty-pants" "trigger" "--mode" "rewrite"; }
}
```

### Sway

Add to `~/.config/sway/config`:

```
bindsym $mod+Shift+p exec smarty-pants trigger --mode rewrite
```

Reload your compositor. Highlight text, press the hotkey.

## Status check

```sh
smarty-pants status
```

## Stop

```sh
smarty-pants daemon stop
# or
pkill smarty-pants-daemon
```

## Tests

```sh
cargo test --workspace
```

32 tests covering core types, config parsing, the daemon pipeline with mocked Wayland + a stub LLM, and end-to-end Unix-socket round-trip.

## License

Apache-2.0.
