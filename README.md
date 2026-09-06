<p align="center">
  <img src="docs/assets/smartypants.png" alt="smarty-pants logo" width="320">
</p>

# smarty-pants

AI writing assistant for Wayland. Highlight text in any app, press a hotkey, get it rewritten in place. Local inference is the default; direct DeepSeek and compatible APIs are optional. No telemetry. Four modes ship by default: **general** grammar/fluency fix, **LinkedIn** voice, **academic** voice, and **condense** for LLM-prompt compression.

Runs as a daemon with a native tray menu, portal hotkeys (Hyprland), and a Unix-socket CLI (niri / Sway / other compositors). Local models run inside the daemon using llama.cpp: **Ollama is not required**. The default is Qwen 2.5 1.5B Instruct Q4_K_M (1.1 GB of weights), loaded on the first rewrite and unloaded after five idle minutes. API providers neither download local weights nor initialize llama.cpp.

For a complete DeepSeek installation or upgrade, use the [installation instructions for another agent](docs/install-deepseek.md). They include the inspected openSUSE/Hyprland environment, configuration, private key entry, systemd startup, and English/Portuguese smoke checks.

## Requirements

Local inference needs memory for the selected weights plus context and compute buffers. A Vulkan GPU is optional; CPU inference is supported. The old 7B preset has 4.7 GB of weights, while the new default has 1.1 GB. These are download sizes, not total RAM/VRAM requirements. API inference requires no GPU.

Use stable Rust/Cargo; the build was verified with rustc 1.91.1. Install missing prerequisites for an API-only build and desktop integration (openSUSE Tumbleweed package names):

```sh
sudo zypper install rustup gcc make pkgconf-pkg-config \
    wtype wl-clipboard zenity xdg-utils
rustup toolchain install stable
```

For local inference, also install `gcc-c++ cmake clang libclang13`. The default Vulkan build additionally needs `vulkan-devel vulkan-headers libvulkan1 glslang-devel shaderc`. Install `xdg-desktop-portal-hyprland` when using Hyprland portal shortcuts.

The tray needs a StatusNotifierItem host, such as KDE Plasma or Waybar with its `tray` module enabled. GNOME needs an AppIndicator extension. Optional `zenity` supplies the API connection/key dialogs; `xdg-open` opens advanced settings in your desktop editor. The other tray controls work without Zenity.

Local builds compile llama.cpp from source, which can take several minutes. API-only builds skip that dependency and its GPU toolchain. The Python/CoEdIT evaluation dependencies are separate from the application and are not needed to install it.

## Build & install

```sh
git clone https://github.com/thbertoldi/smarty-pants.git
cd smarty-pants
cargo install --path crates/cli    --locked --root "$HOME/.cargo"
cargo install --path crates/daemon --locked --root "$HOME/.cargo"
```

If you already have a development checkout, build from that directory. A fresh clone does not include another working tree's uncommitted changes. Confirm your checkout contains the provider/tray implementation before following these instructions.

For an API-only installation, skip the C++/Vulkan build entirely:

```sh
cargo install --path crates/cli --locked --root "$HOME/.cargo"
cargo install --path crates/daemon --locked --root "$HOME/.cargo" \
    --no-default-features --features tray
```

Choose one daemon build. Configure an API provider **before starting an API-only build**, using the [DeepSeek steps below](#direct-deepseek-api) or a compatible provider. For local CPU inference without Vulkan, use `--no-default-features --features local,tray`. An API-only binary must be rebuilt with `local` to use local models; switching a setting alone cannot add that capability.

The commands above install `smarty-pants` (CLI) and `smarty-pants-daemon` in `~/.cargo/bin/`, matching the packaged service. Run builds sequentially. Make sure that directory is on your PATH. In Bash/Zsh:

```sh
export PATH="$HOME/.cargo/bin:$PATH"
```

For Fish:

```fish
fish_add_path ~/.cargo/bin
```

After upgrading an existing systemd installation, run `systemctl --user restart smarty-pants.service` to use the new binary. Existing configuration is preserved; model names that older versions ignored now take effect.

## First run

For a foreground test, after configuring your provider:

```sh
smarty-pants-daemon
```

Stop this test with `Ctrl+C` before enabling the systemd service. If the application is already managed by systemd, use `systemctl --user restart smarty-pants.service` to run the upgraded binary.

The tray and socket become available immediately. On the **first local rewrite**, the daemon downloads and SHA-256 verifies the selected preset in `~/.local/share/smarty-pants/models/`, then loads it. This first request can take several minutes depending on the connection. Cached weights are reused. After idle unloading, only the load repeats.

Watch for `daemon ready` and, when supported, `portal shortcuts bound count=4`. You can use `smarty-pants status` to inspect the selected provider, model, residency, pause/busy state, and last inference error.

## Tray and configuration

Right-click the tray icon for:

- **Provider:** local, DeepSeek (cloud), or a compatible API.
- **Local model:** Qwen 2.5 1.5B, 3B, 7B, or Gemma 3 1B.
- **API connection / Set API key:** optional Zenity dialogs for endpoint, model, credentials.
- **Resource use:** GPU/CPU, keep the model loaded, or unload it now.
- **Paused**, **Restore previous clipboard**, **Edit configuration**, **Reload configuration**, and **Quit**.

Changes persist in `~/.config/smarty-pants/config.toml` and apply without a restart. Existing comments and custom prompts are preserved. If a rewrite is active, retry the change after it finishes. Invalid configuration keeps the working runtime intact. Socket location, log level, and tray enable/disable require a restart; shortcut changes are rebound on reload.

For a first API setup, use **API connection** from the local provider, or select **Compatible API** to open its setup form. Saving the form selects that provider. Set its key through **Set API key** if needed.

Opening the tray can move focus. Use your hotkey in the target application to rewrite text.

## Providers and smaller models

Local is the default:

```toml
[inference]
provider = "local"

[model]
name = "qwen-2.5-1.5b-instruct-q4_k_m"
temperature = 0.2
idle_unload_seconds = 300   # 0 = keep loaded
```

Available presets:

| Preset | Weight download | Template |
| --- | --- | --- |
| `qwen-2.5-1.5b-instruct-q4_k_m` (default) | 1.1 GB | ChatML |
| `qwen-2.5-3b-instruct-q4_k_m` | 2.1 GB | ChatML |
| `qwen-2.5-7b-instruct-q4_k_m` | 4.7 GB | ChatML |
| `gemma-3-1b-it-q4_k_m` | 0.8 GB | Gemma |

For a GGUF you already have, set `model.path = "/absolute/path/model.gguf"` and `model.chat_template = "chatml"` (or `"gemma"`). A custom path overrides the preset and skips downloads. Use a model whose template matches; arbitrary GGUF architectures/templates are not automatically supported.

Qwen's model cards list English and Portuguese support. The 1.5B preset is a resource-conscious starting point, not a claim of better factual fidelity than 7B. Smaller models still need evaluation on your writing. See [model research and evaluation](docs/models.md), including a specialized editing model and its tradeoffs.

A [24-case Qwen versus CoEdIT comparison](docs/evaluations/2026-09-06-coedit-vs-qwen/report.md) includes English and Portuguese outputs, timings, memory use, and observed failures. CoEdIT's evaluation runs separately from the application; it is not an installed provider.

### Direct DeepSeek API

For a new config, copy the [minimal DeepSeek example](examples/deepseek.toml) before starting the daemon:

```sh
smarty_config_dir="${XDG_CONFIG_HOME:-$HOME/.config}/smarty-pants"
install -d -m 700 "$smarty_config_dir"
if [ ! -e "$smarty_config_dir/config.toml" ]; then
    install -m 600 examples/deepseek.toml "$smarty_config_dir/config.toml"
fi
```

For an existing config, merge these settings while preserving custom prompts and credentials; do not append duplicate TOML tables:

```toml
[inference]
provider = "deepseek"

[deepseek]
model = "deepseek-v4-flash"
base_url = "https://api.deepseek.com"
api_key_env = "DEEPSEEK_API_KEY"
timeout_seconds = 60
```

Start or restart the daemon, then choose **Provider → DeepSeek (cloud)** and **Set API key…** in the tray. Enter a key from the [DeepSeek platform](https://platform.deepseek.com/) in the local dialog. The application writes a separate file with mode `0600` and stores only its path in TOML. Startup works before key entry; `status` confirms the active configuration, not API authentication.

Alternatively, set `DEEPSEEK_API_KEY` in the daemon's environment, or configure `api_key_file = "/absolute/path/deepseek.key"`. A key file contains the raw key only and takes precedence over the environment. For a systemd service, a variable exported in a terminal is not automatically inherited: use the tray-managed key file or an existing user service `EnvironmentFile`. Keep keys out of source files and agent messages.

The model IDs follow the [DeepSeek quick start](https://api-docs.deepseek.com/), checked September 6, 2026. To change models, edit **`deepseek.model`** or use **API connection…** in the tray; `model.name` selects local weights. The application sends `thinking = {"type": "disabled"}` internally, following [DeepSeek's thinking-mode API](https://api-docs.deepseek.com/guides/thinking_mode/); this is not a TOML option. `[model] temperature` and `max_tokens` apply to API generation too.

After editing the config of the running upgraded daemon:

```sh
smarty-pants config reload
smarty-pants status
```

Expect `provider: deepseek` and `model: deepseek-v4-flash`, then test a synthetic selection in an editor using your rewrite hotkey. The API receives your selected text and mode instructions, and usage is billed by the provider; an API balance is required. There is no automatic fallback between local and remote providers. The [agent installation guide](docs/install-deepseek.md) includes both language examples and troubleshooting.

### Compatible chat-completions APIs

```toml
[inference]
provider = "openai_compatible"

[api]
base_url = "http://localhost:8080/v1"
model = "your-served-model"
# api_key_env = "WRITING_API_KEY"  # omit for a local server without auth
# api_key_file = "/absolute/path/provider.key"
timeout_seconds = 60
```

The client appends `/chat/completions` unless already present. It supports the conventional non-streaming chat-completions request/response format, including `temperature`, `top_p`, and `max_tokens`. Remote URLs require HTTPS; HTTP works on loopback. Providers that require different fields or APIs may need an adapter. Provider errors, empty answers, and truncated completions are never pasted. Selected/generated text and API keys are not included in daemon logs.

By default, `[writing] preserve_literals = true` also blocks rewrites that change or omit numbers, links, email addresses, or code contents. This is conservative: spelling `3` as `three` or reformatting a date is rejected even when its meaning is preserved. Set `preserve_literals = false` if you prefer to review those changes yourself. The check does not verify names, negation, or meaning in general. Selections over `capture.max_chars` are rejected instead of silently truncated.

## Configure shortcuts

### Hyprland

The daemon registers four shortcut ids with the portal:

| Shortcut id                  | Mode                          | Suggested key combo |
| ---------------------------- | ----------------------------- | ------------------- |
| `surface-transient:rewrite`  | general grammar / fluency fix | `Super+R`           |
| `surface-transient:academic` | academic voice                | `Super+A`           |
| `surface-transient:linkedin` | LinkedIn voice                | `Super+I`           |
| `surface-transient:condense` | condense for fewer LLM tokens | `Super+K`           |

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
bind = SUPER, K, global, surface-transient:condense
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
    Mod+K { spawn "smarty-pants" "trigger" "--mode" "condense"; }
}
```

```
# ~/.config/sway/config
bindsym $mod+R exec smarty-pants trigger --mode rewrite
bindsym $mod+A exec smarty-pants trigger --mode academic
bindsym $mod+I exec smarty-pants trigger --mode linkedin
bindsym $mod+K exec smarty-pants trigger --mode condense
```

## Auto-start at login (systemd user unit)

Run from your graphical session. Preserve an existing unit and its overrides; install the example only for a new service:

```sh
smarty_unit_dir="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
install -d "$smarty_unit_dir"
if [ ! -e "$smarty_unit_dir/smarty-pants.service" ]; then
    install -m 644 packaging/systemd/smarty-pants.service "$smarty_unit_dir/"
fi
systemctl --user daemon-reload
systemctl --user enable smarty-pants.service
systemctl --user restart smarty-pants.service
smarty-pants status
```

The packaged service runs `~/.cargo/bin/smarty-pants-daemon`. For an existing installation, check `systemctl --user show smarty-pants.service --property=ExecStart` points to the binaries you installed. If Wayland session variables are missing from the service, follow the environment-import step in the [installation guide](docs/install-deepseek.md#4-restart-through-systemd-then-enter-the-api-key-locally).

Inspect logs with:

```sh
journalctl --user -u smarty-pants -f
```

## Usage

1. Highlight some text in any window (the **primary selection** — i.e. just mouse-highlighting is enough; you don't need to `Ctrl+C` it).
2. Keep focus on the target window where the rewrite should land.
3. Press the hotkey for the mode you want (`Super+R` for general rewrite, `Super+A` for academic, `Super+I` for LinkedIn, `Super+K` for condense).
4. Keep the selection and target window focused while the rewrite runs. The highlighted text is replaced with the result. Latency depends on model, hardware, input length, and provider; the first local call also downloads/loads weights if needed.

With the default `inject.restore_clipboard = false`, the paraphrase stays on your system clipboard, so you can manually `Ctrl+Shift+V` (terminal) / `Ctrl+V` (GUI) to paste it. Enabling **Restore previous clipboard** restores the earlier clipboard contents after injection.

The daemon auto-detects terminal windows (Ghostty, kitty, foot, Alacritty, WezTerm, gnome-terminal, Konsole, …) and uses `Ctrl+Shift+V` for them; everything else gets `Ctrl+V`.

## Customize

Optional config at `~/.config/smarty-pants/config.toml`. Defaults are sensible; override anything you don't like:

```toml
[model]
gpu_layers = -1     # -1 = offload all if GPU present, 0 = force CPU
temperature = 0.2

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
smarty-pants daemon stop             # clean shutdown via socket
smarty-pants pause
smarty-pants resume
smarty-pants unload                  # release local weights now
smarty-pants config path
smarty-pants config edit
smarty-pants config reload
smarty-pants config provider deepseek
smarty-pants config provider local
smarty-pants config model qwen-2.5-1.5b-instruct-q4_k_m
```

The CLI honors `daemon.socket_path`. For recovery from malformed configuration with a custom socket, set `SMARTY_PANTS_SOCKET=/path/to/smarty-pants.sock` for the CLI command.

## Known limitations

- **First local paraphrase includes model loading** and possibly a download. llama.cpp still creates a fresh context per request. Enable **Keep model loaded** to avoid reloading weights after idle periods.
- **Model output can change meaning or invent facts.** Lower temperature and preservation instructions reduce variation, but do not guarantee fidelity. The [comparison](docs/evaluations/2026-09-06-coedit-vs-qwen/report.md) found omitted facts and models following instructions embedded in the selected text. Literal checks do not catch every such failure. Review important writing; use the evaluation fixtures before adopting a model.
- **TUI applications inside terminals (Claude Code, helix, lazygit, …) sometimes swallow synthesized paste.** With `restore_clipboard = false`, the paraphrase stays on the clipboard for manual paste with `Ctrl+Shift+V`.
- **Models may refuse some inputs.** Choose another preset or custom GGUF through configuration. A text-only refusal cannot always be distinguished from ordinary output.
- **Hyprland's portal-shortcut id namespace is `surface-transient`** — that's xdg-desktop-portal-hyprland's fallback when an app doesn't pass a `WindowIdentifier`. Cosmetic; functional.
- **Only tested on Hyprland + wlroots-family compositors.** KDE Plasma 6 has portal support but uses different paste-chain quirks (Phase 3).

## Acknowledgments

Huge kudos to [cjpais/Handy](https://github.com/cjpais/Handy) — its battle-tested Wayland clipboard and synthetic-keystroke plumbing was the reference that made the inject path here actually work across compositors. Worth a star if you do anything Wayland-quirky in Rust.

## License

[MPL-2.0](LICENSE).
