# Install smarty-pants with DeepSeek: instructions for another agent

## Quick path: RPM on Tumbleweed, release binary elsewhere

On a current **openSUSE Tumbleweed x86_64** machine, follow [the RPM installation steps](install.md#opensuse-tumbleweed-rpm). This gives Zypper-managed upgrades and includes both local CPU and API support. Open Smarty Pants, select **Provider → DeepSeek (cloud)** before triggering a rewrite, enter the key through **Set API key…**, and choose **API model → DeepSeek V4 Flash**, **DeepSeek V4 Pro**, or **Custom model…**. API mode downloads no local model. The RPM ships additional instructions at `/usr/share/doc/packages/smarty-pants/README.openSUSE`.

When migrating an existing installation, inspect the service and shortcut paths first. The RPM binaries are in `/usr/bin`; a user-owned service may still select `~/.cargo/bin` or `~/.local/bin`. Preserve custom settings and drop-ins, update `ExecStart` to `/usr/bin/smarty-pants-daemon`, reload the user manager, and restart when idle. Use absolute `/usr/bin/smarty-pants` paths for verification and bindings if an older binary shadows it. Keep configuration and key files intact.

For other supported Linux systems, or an API-only installation, follow [the binary installer guide](install.md#downloaded-binaries) and choose the **API** bundle. This skips Rust, C++, Vulkan and model downloads. Open Smarty Pants, select **Provider → DeepSeek**, enter the key through **Set API key…**, and choose **API model → DeepSeek V4 Flash** or **DeepSeek V4 Pro**. Model changes apply immediately and preserve the endpoint and key. Use **Rewrite delivery → Review, then copy…** for the first English/Portuguese checks.

If upgrading the existing environment described below, inspect the current service first. Release binaries install under `~/.local/bin`; the earlier source installation uses `~/.cargo/bin`. The installer preserves a custom service and writes a `.service.new` suggestion. Update `ExecStart` deliberately and restart the service to run the new binary. Keep user configuration, service drop-ins, and API keys.

The detailed procedure below remains available for **source installation**, existing-service diagnosis, and agent handoff.

Install or upgrade smarty-pants on the user's openSUSE Tumbleweed / Hyprland
desktop, enable the tray, and configure direct DeepSeek inference for English
and Portuguese writing. Use the API-only build unless the user also wants local
inference. Carry the installation through verification and report any step that
could not be tested.

## Environment and source

These facts were inspected on September 6, 2026; recheck them before acting:

| Item | Observed value |
| --- | --- |
| Checkout | `/home/thbertoldi/suse/smarty-pants` |
| OS / desktop | openSUSE Tumbleweed, Hyprland on Wayland |
| Binaries used by the service | `/home/thbertoldi/.cargo/bin/` |
| Existing service | `smarty-pants.service`, active under the user's systemd manager |
| User configuration | `/home/thbertoldi/.config/smarty-pants/config.toml` exists; DeepSeek V4 Flash configured |
| Tray host | Waybar running; `tray` already enabled in `~/.config/waybar/config.jsonc` |
| Dialog / clipboard tools | `zenity`, `wtype`, `wl-copy`, `wl-paste` installed |
| Writing bindings in config files | Super+R rewrite, Super+A academic, Super+I LinkedIn, Super+C condense |
| Shortcut registration | `hyprctl globalshortcuts` returned `none`; verify after upgrading |

On this machine, build the existing checkout. For another machine, clone the
repository and use a revision containing this guide and the provider/tray
implementation:

```bash
git clone https://github.com/thbertoldi/smarty-pants.git
cd smarty-pants
```

Adjust the machine-specific paths below for that installation. Model caches
and `target/` are not required. If an existing development checkout has newer
uncommitted changes, preserve and build that working tree; cloning the remote
does not transfer those additional local edits.

Read the applicable `AGENTS.md`, `knowledge/CERTAINS.md`, and `README.md`.
Preserve the working tree, existing user settings, custom mode prompts, API
key files, service overrides, and compositor bindings. Back up files you edit.
Do not replace an existing config with an example file. The full
`examples/config.toml` deliberately overrides the rewrite prompt; the smaller
`examples/deepseek.toml` retains the built-in writing prompts.

Run the following shell blocks in **Bash**, as the desktop user. Use `sudo`
only for OS packages; use `systemctl --user` for the application service.

## 1. Check prerequisites

```bash
cd /home/thbertoldi/suse/smarty-pants
git status --short
test -f crates/daemon/src/api_llm.rs
test -f crates/daemon/src/tray.rs
rustc --version
cargo --version
command -v wtype wl-copy wl-paste zenity xdg-open
systemctl --user show smarty-pants.service --property=FragmentPath --property=ExecStart
```

This environment already has the needed tools. On a fresh openSUSE host,
install missing prerequisites:

```bash
sudo zypper install rustup gcc make pkgconf-pkg-config \
  wtype wl-clipboard zenity xdg-utils
rustup toolchain install stable
```

For Hyprland portal shortcuts, also install `xdg-desktop-portal-hyprland`.
A working Waybar `tray` module is sufficient for the tray icon; preserve the
existing panel configuration. The Rust build was verified with rustc 1.91.1.
The API-only application does not need Ollama, llama.cpp weights, Vulkan,
PyTorch, or the CoEdIT evaluation environment.

## 2. Install the CLI and API-only daemon

```bash
cargo install --path crates/cli --locked --root "$HOME/.cargo"
cargo install --path crates/daemon --locked --root "$HOME/.cargo" \
  --no-default-features --features tray
export PATH="$HOME/.cargo/bin:$PATH"
smarty-pants --help
```

Run these builds sequentially. `--root` makes the install location agree with
the packaged systemd unit. Local path installs rebuild the checkout even if
its package version is unchanged. Leave the existing service running during
the build; restart it after the configuration is ready. Do not launch a second
daemon alongside the service.

If local inference is also required, replace the **daemon** install command
with one of these instead:

```bash
# Local CPU inference, direct APIs, and tray; requires the C++ build tools.
cargo install --path crates/daemon --locked --root "$HOME/.cargo" \
  --no-default-features --features local,tray

# Or local Vulkan inference, direct APIs, and tray; default features.
cargo install --path crates/daemon --locked --root "$HOME/.cargo"
```

These are alternatives, not additional sequential installation steps.
Local builds need `gcc-c++ cmake clang libclang13`; the Vulkan build also needs
`vulkan-devel vulkan-headers libvulkan1 glslang-devel shaderc`. An API-only
binary cannot switch to Local until rebuilt with the `local` feature. Direct
DeepSeek inference loads no local model even in a binary that supports both.

## 3. Configure DeepSeek before the first restart

```bash
smarty_config_dir="${XDG_CONFIG_HOME:-$HOME/.config}/smarty-pants"
install -d -m 700 "$smarty_config_dir"
if [ ! -e "$smarty_config_dir/config.toml" ]; then
  install -m 600 examples/deepseek.toml "$smarty_config_dir/config.toml"
fi
```

If configuration already exists, merge these fields into it, keeping existing
credentials and custom modes. TOML tables must appear only once:

```toml
[inference]
provider = "deepseek"

[deepseek]
base_url = "https://api.deepseek.com"
model = "deepseek-v4-flash"
api_key_env = "DEEPSEEK_API_KEY"
timeout_seconds = 60

[tray]
enabled = true
```

`deepseek-v4-flash` and `deepseek-v4-pro` are current IDs in the
[official DeepSeek quick start](https://api-docs.deepseek.com/), checked on
September 6, 2026. Flash is the application's default. If changing the model
later, edit **`deepseek.model`**, not `model.name`, which selects a local GGUF.
The tray's **API connection…** form can also change the API model.

The application calls `https://api.deepseek.com/chat/completions` directly and
sets `thinking = {"type": "disabled"}` internally. Do not add a `thinking`
field to the TOML; it is not a configuration option. The request toggle is
documented in [DeepSeek's thinking guide](https://api-docs.deepseek.com/guides/thinking_mode/).
`[model] temperature` and `max_tokens` are shared generation settings and also
apply to API requests.

## 4. Restart through systemd, then enter the API key locally

Preserve the existing service on this machine: its `ExecStart` already points
to `~/.cargo/bin/smarty-pants-daemon`. On a fresh installation, create the unit
only if one does not exist:

```bash
smarty_unit_dir="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
install -d "$smarty_unit_dir"
if [ ! -e "$smarty_unit_dir/smarty-pants.service" ]; then
  install -m 644 packaging/systemd/smarty-pants.service "$smarty_unit_dir/"
fi
systemctl --user daemon-reload
systemctl --user enable smarty-pants.service
systemctl --user restart smarty-pants.service
systemctl --user is-active smarty-pants.service
smarty-pants status
```

Run this from the graphical session. If the service lacks the current Wayland
session variables, import those variables from a terminal inside that session,
then restart:

```bash
systemctl --user import-environment WAYLAND_DISPLAY XDG_CURRENT_DESKTOP XDG_SESSION_TYPE
systemctl --user restart smarty-pants.service
```

The expected status includes `provider: deepseek`, `model: deepseek-v4-flash`,
`paused: false`, and four modes unless the user has customized them. Startup
and `status` do **not** verify the API key or account balance. The daemon can
start before a key is supplied, allowing the tray to configure it.

Use an API key from the [DeepSeek platform](https://platform.deepseek.com/)
and an account with available API balance:

1. Have the user right-click the smarty-pants tray icon.
2. Confirm **Provider → DeepSeek (cloud)**.
3. Choose **Set API key…** and enter the key in the local password dialog.

The application writes a private file with permissions `0600` and persists
only its path as `[deepseek].api_key_file`. It applies the setting immediately.
The raw key belongs in the local dialog, not in the agent conversation,
repository, shell command arguments, or report. Do not print existing key
files while verifying setup.

If the user already has a key file, point `deepseek.api_key_file` at its
absolute path instead and run `smarty-pants config reload`. The file contains
the **raw key only**, with no `DEEPSEEK_API_KEY=` prefix. Keep its permissions
at `0600`. Paths in TOML support `$HOME` and `$XDG_CONFIG_HOME` expansion;
do not use a literal `~` or Bash's `${VAR:-default}` syntax there.

An explicit key file takes precedence over `api_key_env`. If using an
environment variable instead, it must exist in the **daemon's** environment.
A terminal `export DEEPSEEK_API_KEY=...` is not automatically inherited by the
systemd service. An existing service `EnvironmentFile` can provide it; an
agent should preserve that setup if present. The tray-managed file avoids
needing a service environment override.

## 5. Verify the tray, shortcuts, and real rewrites

Inspect the startup messages:

```bash
smarty-pants status
journalctl --user -u smarty-pants.service -n 50 --no-pager
hyprctl globalshortcuts
```

This machine's configuration already contains portal bindings for
`surface-transient:rewrite`, `academic`, `linkedin`, and `condense` in
`~/.config/hypr/lua/keybinds.lua` and `~/.config/hypr/conf.d/90-keybinds.conf`.
Determine which configuration is actually loaded. Keep the user's existing
Super+R/A/I/C choices rather than duplicating the README's suggested bindings.
`hyprctl globalshortcuts` returned `none` before this upgrade, so do not assume
the on-disk bindings are functioning just because they exist.

If portal registration remains unavailable, bind the same modes to CLI
commands in the active Hyprland configuration. For a classic `.conf` file,
the rewrite binding on this machine would be:

```text
bind = SUPER, R, exec, /home/thbertoldi/.cargo/bin/smarty-pants trigger --mode rewrite
```

Use `academic`, `linkedin`, or `condense` for the other modes. Replace the
corresponding old binding rather than adding a competing one. If a Lua
configuration is active, use that configuration's existing binding API.
Reload the compositor configuration with `hyprctl reload` and verify the
binding in the active session.

After the key is entered, test two synthetic selections in a disposable
editor document, one at a time:

```text
Yesterday, Ana send the report to Bruno. We have not approved the release yet.
```

```text
Ontem, a Ana enviou os relatório para o Bruno. A gente ainda não aprovou a versão.
```

Keep the editor focused and use the rewrite hotkey. Check that the result
stays in the source language, corrects grammar, retains names and negation,
and contains no explanation or answer to the text. Each rewrite sends the
selection and mode instructions to DeepSeek and uses billed API tokens.
Neither the startup check nor the previous local model comparison tested
live DeepSeek writing quality.

For a CLI trigger, schedule it with enough time to return focus to the editor;
the CLI has no `--text` argument and operates on the current selection:

```bash
sleep 3 && smarty-pants trigger --mode rewrite
```

Verify Pause/Resume and the tray's API connection display. API mode should not
download or keep a local model resident. Existing cached GGUF files may stay
on disk; their presence does not mean they are loaded. Leave those caches
available for any later local use.

## Troubleshooting and completion report

| Symptom | What to check |
| --- | --- |
| `this build has no local inference` at startup | Set `[inference] provider = "deepseek"` before starting an API-only build. |
| Missing API-key environment variable / unreadable key file | Use Set API key in the tray, or correct the configured credential path. A configured key file overrides the environment. |
| HTTP 401/403 | API key and account permissions. |
| HTTP 402 | DeepSeek API account balance. |
| HTTP 404 | API base URL and `deepseek.model`; use the API endpoint rather than the chat website. |
| HTTP 429 / timeout | Rate limit or network/provider delay; retry deliberately after resolving it. |
| `status` still shows old behavior | Verify the service's executable path and restart after installing both binaries. |
| No tray icon | Confirm a tray-enabled build, `[tray] enabled = true`, Waybar's `tray` module, and session D-Bus access. |
| Hotkey does nothing | Check actual portal registration and loaded bindings; use a CLI binding if necessary. |
| Literal-preservation error | The output changed a protected number/link/code literal. The original text was left untouched; this is not an installation failure. |

No automatic local/cloud fallback exists. The literal check does not verify
all meaning or prevent a model from following instructions embedded in text.

Report the installed build features, executable path, active provider/model,
service state, tray visibility, shortcut results, and both rewrite outcomes.
Report key-entry or live-API steps as untested if they could not be completed;
do not infer authentication success from `status`. Never include the key in
the completion report.
