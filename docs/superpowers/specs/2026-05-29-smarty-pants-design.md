# smarty-pants — Design Spec

**Date:** 2026-05-29
**Status:** Draft, awaiting user review
**Target platform:** Linux on wlroots-based Wayland compositors (Hyprland, niri, Sway)

## 1. Overview

smarty-pants is a local AI paraphraser for Wayland. The user highlights text in any application, presses a hotkey, and the highlighted text is replaced in place with an AI-rewritten version. All inference runs locally — no cloud calls, no telemetry.

The interaction model is intentionally similar to [Handy](https://github.com/cjpais/Handy), which performs the same trick for speech (audio → text via Parakeet). smarty-pants replaces the audio side with a text-selection side and the STT model with a small instruction-tuned LLM.

## 2. Goals & non-goals

### Goals

- One-keypress paraphrase of the user's current text selection.
- Multiple paraphrase modes (rewrite, formal, shorten, fix-grammar, user-defined) selectable via different hotkeys.
- Multilingual quality on at least the languages a small modern instruct LLM covers well (Gemma 3 ships with 140+).
- Snappy: target round-trip < 3 s for a single sentence on a typical laptop with a Vulkan-capable GPU.
- Local-only: model weights live on disk, no network calls during inference.
- Pluggable model: user can swap the active GGUF model via config without touching code.
- GPU acceleration enabled by default via Vulkan; works on any GPU vendor on Linux.
- **First-class Hyprland integration** via `org.freedesktop.portal.GlobalShortcuts` (ashpd) — shortcuts are app-managed and user-rebindable in Hyprland's shortcut UI, no `hyprland.conf` editing required. niri/Sway fall back to compositor-config + CLI trigger (universal path).

### Non-goals (explicitly)

- Windows / macOS support. Wayland-specific by design.
- Tauri / GTK / Qt settings GUI. TOML config is the UI.
- Cloud LLM backends (OpenAI, Anthropic, etc.).
- Speech-to-text or other modalities.
- GNOME and KDE Plasma support in v1. Their hotkey + injection stories differ enough to warrant separate work later.

## 3. High-level architecture

The daemon accepts paraphrase requests from **two parallel input paths**, both feeding the same internal pipeline:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  Path A — Hyprland (portal-managed shortcuts)                               │
│                                                                             │
│   org.freedesktop.portal.GlobalShortcuts (D-Bus)                            │
│       BindShortcuts ─▶ user rebinds in Hyprland's shortcut UI               │
│       Activated signals → daemon.shortcuts dispatcher                       │
│                                                                             │
└──────────────┬──────────────────────────────────────────────────────────────┘
               │ D-Bus signal carrying shortcut id (== mode name)
               │
┌─────────────────────────────────────────────────────────────────────────────┐
│  Path B — niri / Sway and as universal fallback                             │
│                                                                             │
│   bindsym  Super+Shift+P  exec  smarty-pants trigger --mode rewrite         │
│                                                                             │
└──────────────┬──────────────────────────────────────────────────────────────┘
               │ spawn (cheap; exits in <100 ms after sending request)
               ▼
       ┌───────────────┐                     ┌──────────────────────────────┐
       │   trigger     │  ── unix socket ──▶ │           daemon             │
       │ (tiny CLI)    │  Request{mode}      │                              │
       └───────────────┘ ◀── Response ───────┤  ┌────────────────────────┐  │
                                             │  │ shortcuts (ashpd)      │  │  Path A entry
                                             │  ├────────────────────────┤  │
                                             │  │ server (unix socket)   │  │  Path B entry
                                             │  ├────────────────────────┤  │
                                             │  │ pipeline               │  │  Shared
                                             │  │   selection            │  │  PRIMARY → Ctrl+C fallback
                                             │  │   llm inference        │  │  llama-cpp-2 + Gemma 3 1B Q4
                                             │  │   inject               │  │  set clipboard → Ctrl+V → restore
                                             │  │   notify               │  │  notify-rust
                                             │  │   history (JSONL)      │  │  for undo
                                             │  └────────────────────────┘  │
                                             └──────────────────────────────┘
```

**Process model**: long-lived daemon, tiny CLI trigger.

- **On Hyprland**: daemon registers shortcut ids (one per configured mode) with the XDG GlobalShortcuts portal at startup. The user binds those ids to key combos in Hyprland's settings UI. Activations arrive as D-Bus signals — no `trigger` CLI, no `hyprland.conf` edit.
- **On niri / Sway** (and as universal fallback): user adds `bindsym … exec smarty-pants trigger --mode X` to their compositor config. The CLI is a sub-100 ms cold-start binary that sends one request over a Unix socket and exits.
- **Both paths** dispatch into the same `pipeline::run(mode)` — only the entry point differs.
- Daemon keeps the model resident in RAM (and on GPU) → paraphrase latency is dominated by inference, not loading.
- Socket protocol: length-prefixed JSON over `$XDG_RUNTIME_DIR/smarty-pants.sock`.
- Portal detection: at startup the daemon probes `org.freedesktop.portal.GlobalShortcuts` via D-Bus. If the interface is present and `BindShortcuts` succeeds, the portal path is active; otherwise the daemon logs a single info line and serves the socket-only path.

## 4. Component boundaries

Cargo workspace, three crates:

```
smarty-pants/
├── Cargo.toml                       (workspace)
├── crates/
│   ├── core/                        smarty-pants-core
│   │   ├── lib.rs
│   │   ├── config.rs                Config, Mode (serde TOML)
│   │   ├── protocol.rs              Request / Response / HistoryRecord
│   │   └── paths.rs                 XDG dirs, socket path
│   │
│   ├── daemon/                      smarty-pants-daemon (bin)
│   │   ├── main.rs                  wiring + signal handling
│   │   ├── server.rs                Unix-socket accept loop, request dispatch
│   │   ├── shortcuts.rs             ashpd GlobalShortcuts session, activation dispatch
│   │   ├── pipeline.rs              orchestrates: capture → llm → inject
│   │   ├── selection.rs             read PRIMARY, fallback Ctrl+C, restore
│   │   ├── inject.rs                set clipboard, synth Ctrl+V, restore
│   │   ├── wayland.rs               wtype / wl-clipboard-rs adapters
│   │   ├── llm.rs                   llama-cpp-2 wrapper, model load/generate
│   │   ├── prompt.rs                render mode-prompt + selection
│   │   ├── history.rs               append-only JSONL writer/reader
│   │   ├── notify.rs                desktop toast
│   │   └── reload.rs                SIGHUP → re-read config
│   │
│   └── cli/                         smarty-pants (bin, "smarty-pants" binary)
│       ├── main.rs                  clap subcommand router
│       ├── trigger.rs               connect socket, send request, print
│       ├── status.rs                ping daemon, print health
│       ├── undo.rs                  read latest history, inject original
│       ├── models.rs                list / download / verify GGUFs
│       └── daemon.rs                start / stop / reload daemon
```

### Crate-level rules

- `core` owns every type that crosses the socket. No I/O lives here.
- `daemon` is the only crate that depends on `llama-cpp-2`. `cli` never links the LLM runtime → tiny binary, fast startup.
- `wayland.rs` is the single thin abstraction over `wl-clipboard-rs` + a `wtype` subprocess. Anything compositor-quirky lives behind this façade so tests can mock it.
- `pipeline.rs` is the only place where capture, llm, and inject meet — end-to-end flow is readable in one file. Both `shortcuts.rs` (portal) and `server.rs` (socket) terminate by calling `pipeline::run(mode)`.
- `shortcuts.rs` is the *only* place that touches D-Bus / ashpd. If the portal probe fails at startup, this module is dormant and the daemon serves the socket path only.

### Dependencies

| Crate | Purpose |
|---|---|
| `llama-cpp-2` | LLM runtime; GGUF; default feature `vulkan`; optional `cuda`, `rocm` |
| `wl-clipboard-rs` | PRIMARY + regular clipboard read/write |
| `ashpd` (≥0.13) | XDG portal client; `global_shortcuts` feature for GlobalShortcuts session |
| `zbus` | Pulled in transitively by ashpd; used directly for the portal-presence probe |
| `tokio`, `tokio-util` | Async runtime, Unix socket framing |
| `serde`, `serde_json`, `toml` | Config + IPC serialization |
| `clap` (derive) | CLI parsing |
| `tracing`, `tracing-subscriber` | Structured logs |
| `notify-rust` | Desktop toasts via libnotify |
| `directories` | XDG path resolution |
| `anyhow`, `thiserror` | Errors |
| `insta` | Snapshot tests for prompt rendering |

`wtype` is shelled out, not linked. Implementing the `virtual-keyboard-unstable-v1` Wayland protocol ourselves is not worth it for v1.

## 5. End-to-end data flow

Both entry paths converge on the same `pipeline::run(mode)`. Two short traces of the entry steps, then a shared trace for the pipeline itself.

### Path A — Hyprland portal-activated (warm daemon)

```
t=0ms    User presses Super+Shift+P
t=5ms    Hyprland fires GlobalShortcuts Activated{shortcut_id:"rewrite"} on D-Bus
t=10ms   daemon.shortcuts receives the signal, calls pipeline::run("rewrite")
         (continues to the shared trace below)
```

### Path B — niri/Sway compositor-config (warm daemon)

```
t=0ms    Compositor fires bindsym → spawns `smarty-pants trigger --mode rewrite`
t=20ms   trigger connects to $XDG_RUNTIME_DIR/smarty-pants.sock
t=25ms   trigger sends:  {"kind":"paraphrase","mode":"rewrite"}\n
t=30ms   daemon accepts, hands off to pipeline::run("rewrite")
```

### Shared pipeline (times shown anchored to Path B's t=30 ms entry; subtract ~20 ms for Path A)

```
         ─────────────────────────────────────────────────────
t=35ms   selection::capture()
           1. read PRIMARY via wl-clipboard-rs       ── ~5 ms
           2. if non-empty: return it
           3. else: stash regular clipboard
                    spawn `wtype -M ctrl c`          ── ~15 ms
                    sleep ctrl_c_settle_ms (40 ms default)
                    read regular clipboard
                    restore clipboard later (deferred)
t=60ms   pipeline got selection
         ─────────────────────────────────────────────────────
t=65ms   prompt::render("rewrite", text) → chat-templated prompt
t=70ms   llm::generate(prompt) — llama-cpp-2 streaming tokens
           first token at t≈120 ms      (warm cache, Vulkan)
           ~80 tok/s on iGPU, 1B Q4
           60-token reply finishes at  ~t=900 ms
t=900ms  generated text ready
         ─────────────────────────────────────────────────────
t=910ms  inject::write(generated)
           1. save current clipboard (whatever it is now)
           2. wl-copy <generated>
           3. spawn `wtype -M ctrl v`                ── ~20 ms
           4. sleep paste_settle_ms (80 ms default)
           5. wl-copy <saved>  (restore)
t=1020ms history::append({mode, original, generated, ts})
t=1025ms notify::toast("✓ Paraphrased (rewrite)")
t=1030ms Path B only: Response::Ok sent to trigger CLI; trigger exits 0
         Path A only: pipeline returns; shortcuts task awaits next Activated signal
```

Timings assume a warm daemon with the model GPU-resident. Cold first request takes longer due to model load (Section 8).

### Failure branches

| Where | What happens | User sees |
|---|---|---|
| Empty selection | pipeline returns early | toast: "No text selected" |
| Daemon not running | `trigger` writes to stderr; exit 2 | terminal output, no toast |
| Model not loaded | reply with `Response::ModelLoading` before pipeline runs | toast: "Model loading… try again shortly" |
| LLM generation timeout | abort generation, return `Response::Error{Timeout}` | toast: "Paraphrase timed out" |
| Inject failed (`wtype` missing) | history recorded, generated text left in clipboard | toast: "Paraphrase in clipboard — paste manually" |
| PRIMARY + Ctrl+C both empty | pipeline returns early | toast: "No text selected" |
| Second trigger while one in flight | reply with `Response::Busy` (or drop the Activated event with a warn log on Path A) | toast (optional): "Busy" |
| Portal `BindShortcuts` fails (no compositor support) | log info, disable shortcuts task, serve socket only | (no user-visible error; documented behavior) |
| D-Bus connection drops mid-session | log warn, exponential-backoff reconnect, re-bind on success | (no user-visible error unless a shortcut press lands during downtime) |

**Concurrency**: single-flight. A second trigger arriving during inference gets `Busy` — daemon refuses to run two paste races into the same window.

## 6. Configuration

Single TOML file at `$XDG_CONFIG_HOME/smarty-pants/config.toml`. Hot-reloadable via `smarty-pants reload` (sends SIGHUP).

```toml
# ── daemon ──────────────────────────────────────────────────────────
[daemon]
socket_path   = "$XDG_RUNTIME_DIR/smarty-pants.sock"
log_level     = "info"                  # trace|debug|info|warn|error
busy_response = "reject"                # reject | queue  (MVP: reject)

# ── shortcuts (XDG portal) ──────────────────────────────────────────
[shortcuts]
enabled         = true                  # register modes with GlobalShortcuts portal
require_portal  = false                 # if true, daemon refuses to start when portal absent;
                                        # default false means "use portal if available, else
                                        # socket-only" so the same config works on niri/Sway.
app_id          = "computer.smarty-pants"   # used by the portal session

# ── model ───────────────────────────────────────────────────────────
[model]
name            = "gemma-3-1b-it-q4_k_m"   # key into [models.*] registry
context_size    = 4096                     # tokens
threads         = 0                        # 0 = auto (num CPUs - 1)
gpu_layers      = -1                       # -1 = offload all if Vulkan device
                                           #  0 = force CPU
                                           # >0 = explicit layer count
gpu_main_device = 0                        # which GPU index if multiple
seed            = 0                        # 0 = random per request
max_tokens      = 512                      # cap on generated tokens
temperature     = 0.7
top_p           = 0.9

# ── selection / inject behavior ─────────────────────────────────────
[capture]
prefer_primary    = true
ctrl_c_settle_ms  = 40
max_chars         = 8000

[inject]
restore_clipboard = true
paste_settle_ms   = 80

# ── notifications & history ─────────────────────────────────────────
[notify]
enabled       = true
success_toast = true
error_toast   = true

[history]
enabled     = true
path        = "$XDG_STATE_HOME/smarty-pants/history.jsonl"
max_entries = 200                       # ring-buffer; oldest pruned

# ── model registry ──────────────────────────────────────────────────
[models.gemma-3-1b-it-q4_k_m]
url           = "https://huggingface.co/unsloth/gemma-3-1b-it-GGUF/resolve/main/gemma-3-1b-it-Q4_K_M.gguf"
sha256        = "<pinned at implementation time>"
size          = 806000000
chat_template = "gemma"

[models.qwen2.5-1.5b-instruct-q4_k_m]
url           = "https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct-GGUF/resolve/main/qwen2.5-1.5b-instruct-q4_k_m.gguf"
sha256        = "<pinned at implementation time>"
size          = 940000000
chat_template = "chatml"

# ── paraphrase modes ────────────────────────────────────────────────
# Each mode is invoked as: smarty-pants trigger --mode <name>
# On compositors with portal support, each mode also registers as a
# GlobalShortcut with id == mode name. The `shortcut` field is a *preferred*
# default that the portal MAY present to the user as a starting suggestion;
# the user re-binds in their compositor's shortcut UI. Set to "" to register
# the shortcut with no suggested binding (still rebindable).

[modes.rewrite]
shortcut    = "SUPER+SHIFT+P"
description = "Paraphrase: rewrite in different words"
system = """
You are a paraphrasing assistant. Rewrite the user's text so it has
the same meaning but different wording and sentence structure. Rules:
- Preserve meaning exactly. Do not add, remove, or invent facts.
- Reply in the SAME LANGUAGE as the input.
- Output ONLY the paraphrased text. No preamble, no quotes, no notes.
"""

[modes.formal]
shortcut    = "SUPER+SHIFT+F"
description = "Paraphrase: formal register"
system = """
Rewrite the user's text in a formal, professional register.
Same meaning, same language. Output only the rewritten text.
"""

[modes.shorten]
shortcut    = "SUPER+SHIFT+S"
description = "Paraphrase: shorten"
system      = """
Rewrite the user's text as concisely as possible without losing meaning.
Same language. Output only the rewritten text.
"""
temperature = 0.4

[modes.fix-grammar]
shortcut    = "SUPER+SHIFT+G"
description = "Paraphrase: fix grammar"
system      = """
Correct grammar, spelling, and punctuation in the user's text. Do not
change meaning, style, or word choice beyond what's needed. Same language.
Output only the corrected text.
"""
temperature = 0.2
```

### Schema notes

- All `$XDG_*` placeholders are expanded by `core::paths` at load time.
- `[models.*]` is a registry. `smarty-pants models list` enumerates, `models download <key>` fetches and SHA-verifies, `[model] name = <key>` activates one.
- Modes are user-extensible by adding `[modes.my_mode]` blocks. Per-mode `temperature`, `top_p`, `max_tokens` override `[model]` defaults.
- `chat_template` is a string switch into a small enum in `daemon::prompt` (`Gemma`, `ChatML`, `Llama3`). New templates are added in code, not config.
- `[shortcuts] enabled = false` disables the portal session entirely (useful if the user wants to force compositor-config + CLI on Hyprland for any reason).
- The portal `shortcut` strings follow ashpd / portal convention: `+`-separated modifier names then key (`CTRL`, `SHIFT`, `ALT`, `SUPER`, plus an X11-style keysym). They are *suggestions* only — the user's compositor settings always win.

## 7. Error handling & testing

### Error taxonomy

Two layers — daemon-internal vs. errors that cross the IPC boundary.

```rust
// crates/core/src/protocol.rs  — what the CLI sees
#[derive(Serialize, Deserialize)]
pub enum Response {
    Ok { generated_chars: usize, ms: u64 },
    Empty,
    Busy,
    ModelLoading,
    Error { kind: ErrorKind, message: String },
}

#[derive(Serialize, Deserialize)]
pub enum ErrorKind {
    Capture, Inference, Timeout, Inject, Internal,
}
```

```rust
// crates/daemon — internal type (never crosses socket directly)
#[derive(thiserror::Error, Debug)]
pub enum DaemonError {
    #[error("wayland: {0}")]            Wayland(String),
    #[error("llm: {0}")]                Llm(#[from] llama_cpp_2::Error),
    #[error("config: {0}")]             Config(String),
    #[error("io: {0}")]                 Io(#[from] std::io::Error),
    #[error("spawn {tool}: {source}")]  Spawn { tool: &'static str, source: std::io::Error },
}
```

`pipeline.rs` converts `DaemonError → Response::Error` at the boundary so the daemon's internal vocabulary stays rich while the wire protocol stays small.

### Rules

1. **Daemon never crashes on a request.** Every pipeline step is `.map_err(DaemonError::…)`; the dispatcher catches and converts. Panics inside the pipeline are caught by `tokio::task` and reported as `Error{kind: Internal}`.
2. **User's clipboard is sacred.** Restore is wrapped in a `defer!`-style guard: if anything between save and restore panics or returns Err, the guard restores on drop.
3. **No silent retries on inference.** A model that fails once will probably fail again the same way.
4. **`wtype` / `wl-copy` missing at startup** → daemon refuses to start with a clear error. Detected via `which` at boot.
5. **Generation timeout** enforced with `tokio::time::timeout` around the streaming token loop; on expiry, the llama context is aborted.

### Logging

`tracing` with JSON output. Privacy defaults: selected and generated text logged at `debug` only, never `info`. A `--log-text` daemon flag opts in to plaintext for debugging.

### Testing strategy

| Layer | Test type | Mocks |
|---|---|---|
| `core::protocol` | Unit (serde roundtrip) | — |
| `core::config` | Unit (TOML parse, defaults, env expansion) | — |
| `daemon::prompt` | Unit, snapshot via `insta` | — |
| `daemon::wayland` | `trait Wayland` with real + mock impls | mock for tests |
| `daemon::selection` | Unit via mock Wayland | Wayland trait |
| `daemon::inject` | Unit via mock Wayland | Wayland trait |
| `daemon::llm` | `trait Llm` with real (`llama-cpp-2`) + stub impls | stub for tests |
| `daemon::pipeline` | Integration: mock Wayland + stub LLM, all failure branches | both |
| `daemon::server` + `cli::trigger` | E2E on temp socket | LLM + Wayland |
| Live model | Manual smoke checklist, gated behind `--features live-model` | — |

TDD discipline per the `superpowers:test-driven-development` workflow: every new feature lands as a failing test first. The mock Wayland and stub LLM are the load-bearing pieces that make TDD viable without a graphical session.

### CI matrix

Linux only.
- `cargo build` (default = `vulkan`) — runner needs `vulkan-loader`, `glslang`, `cmake`, `clang`.
- `cargo build --no-default-features` — validates the CPU-only build path.
- `cargo check --features cuda` — no actual run; no NVIDIA GPU on CI.
- `cargo check --features rocm` — no actual run; no AMD GPU on CI.
- `cargo test`, `cargo clippy -D warnings`, `cargo fmt --check`.

## 8. GPU acceleration

### Backends

`llama-cpp-2` cargo features select the backend:

| Feature | Coverage | Build deps | Notes |
|---|---|---|---|
| `vulkan` (default) | Any GPU vendor on Linux | `vulkan-loader`, `glslang`, `cmake`, `clang` | Path of least resistance for laptop iGPUs. |
| `cuda` (opt-in) | NVIDIA only | CUDA toolkit, `nvcc` | Best perf on NVIDIA; pricier build. |
| `rocm` (opt-in) | AMD only | ROCm, `hipcc` | Painful install, best perf on supported AMD dGPUs. |
| no feature | CPU only | — | Tiny build, used for smoke testing. |

### Runtime behavior

Daemon startup:

1. `llama_backend_init()` enumerates devices.
2. Devices logged at `info`. Example: `gpu: Intel(R) Arc(TM) Graphics — Vulkan, 8 GB VRAM`.
3. If `[model].gpu_layers != 0` but no GPU device is found, log a `warn` and fall back to CPU rather than refusing to start.
4. If `gpu_layers = -1`, set to the model's layer count so all layers offload.

`[model].gpu_main_device` selects between multiple GPUs by index.

### Acceptance for GPU path

On a machine with a Vulkan-capable GPU, paraphrase generation runs ≥3× faster than CPU-only on the same hardware for Gemma 3 1B Q4 (paragraph-length input).

## 9. Phasing & MVP scope

### Phase 1 — MVP (the loop works)

Minimum to make the keypress→paraphrase loop usable on Hyprland/niri/Sway with GPU acceleration.

- Cargo workspace skeleton (`core`, `daemon`, `cli` crates).
- TOML config loader; schema from Section 6 minus `[models.*]` registry.
- Single hardcoded model: `gemma-3-1b-it-q4_k_m`, downloaded on first daemon start with SHA-256 verify, stored in `$XDG_DATA_HOME/smarty-pants/models/`.
- Single fixed `rewrite` mode (defined in code, exposed in `[modes.rewrite]` with a suggested shortcut).
- Daemon: Unix-socket server, single-flight pipeline, SIGTERM clean shutdown.
- Pipeline: PRIMARY → Ctrl+C fallback capture; LLM generate via `llama-cpp-2` with Vulkan default + CPU fallback; clipboard+Ctrl+V inject; restore clipboard.
- Runtime GPU detection; honor `gpu_layers` and `gpu_main_device`.
- **Portal `GlobalShortcuts` integration via `ashpd`**: probe at startup, register one shortcut for the `rewrite` mode if portal is available, dispatch `Activated` signals into `pipeline::run`. Graceful fallback to socket-only if portal is absent or `BindShortcuts` fails.
- CLI: `smarty-pants trigger`, `smarty-pants daemon start|stop`, `smarty-pants status`.
- Logging (tracing) with redaction defaults.
- Pre-flight check for `wtype` + `wl-copy`; clear error if missing.
- Tests per Section 7; ≥1 happy-path E2E with stub LLM via the socket; ≥1 unit test for the shortcuts-dispatch handler with a faked `Activated` stream.
- README documents build deps and basic install, including a one-line "rebind in Hyprland settings → Keyboard → App shortcuts" note.

**Acceptance**:
1. On Hyprland: after `cargo install` + writing the minimal config + starting the daemon, the `rewrite` shortcut appears in Hyprland's shortcut UI as "Paraphrase: rewrite in different words". Pressing the assigned key combo paraphrases the current selection in place within ~3 s on CPU / ≤1 s on a Vulkan-capable GPU (warm — first invocation after daemon start includes one-time model load).
2. On niri or Sway: same setup, but the user adds one `bindsym/binds` line to their compositor config that invokes `smarty-pants trigger --mode rewrite`. Same latency targets. The daemon logs that portal is unavailable and serves the socket path only.
3. No crashes across 50 invocations on either path.

### Phase 2 — usable daily driver

Everything that turns a demo into a tool you keep installed.

- Multi-mode config table (`[modes.*]`); CLI `--mode <name>`.
- **Multi-shortcut portal registration**: one portal `BindShortcuts` entry per enabled mode, ids matching mode names; re-binding on `reload`.
- Model registry in config (`[models.*]`); `smarty-pants models {list,download,remove,active,set}`.
- Switching active model at runtime triggers reload.
- Hot reload on SIGHUP (`smarty-pants reload`) — including re-registering portal shortcuts if the mode set changed.
- Desktop notifications via `notify-rust` (toggle in config).
- History (append-only JSONL, ring-buffered) + `smarty-pants undo`.
- Per-mode generation overrides (`temperature`, `top_p`, `max_tokens`).
- Generation timeout + `Timeout` response end-to-end.
- `Busy` response when a second trigger lands during inference.
- Systemd user unit (`packaging/smarty-pants.service`) + `smarty-pants daemon install-unit`.

**Acceptance**: on Hyprland, all four default modes (rewrite, formal, shorten, fix-grammar) appear as separate entries in the compositor's shortcut UI; each rebindable; each triggers the correct mode. On niri/Sway, the same four modes work via four `bindsym` lines. `undo` restores the original text; daemon survives a `models set` swap without restart; `reload` after adding a fifth mode registers the new shortcut without daemon restart.

### Phase 3 — polish

Strictly nice-to-have; cut any of it if it gets in the way.

- `cargo build --features cuda` documented and tested on a real NVIDIA box.
- `cargo build --features rocm` documented and tested on a real AMD box.
- Streaming partial results back over the socket (CLI shows progress in a terminal).
- TUI status panel (`smarty-pants tui`): daemon health, recent history, mode list, portal status.
- Packaging: `.tar.gz` release artifact, RPM spec for openSUSE / `PKGBUILD` for Arch, optional Flatpak.
- KDE Plasma 6 support (portal shortcuts work; verify selection/injection chain on KWin).

### Out of scope (won't build unless asked)

- Tauri / GTK / iced settings GUI.
- Windows / macOS support.
- Cloud LLM backends.
- Speech-to-text or other modalities.
- GNOME and KDE Plasma support.

## 10. Open questions

These can be answered during implementation, but are worth flagging:

- **Exact SHA-256 pins**: Section 6 placeholders for `[models.*].sha256` need real values from the chosen GGUF artifacts at implementation time.
- **wtype availability on SUSE Tumbleweed**: confirm `wtype` is packaged or document an install path. Same for `wl-clipboard`.
- **Vulkan loader on minimal SUSE setups**: confirm `libvulkan1` is present after default install on the target machine.
- **`xdg-desktop-portal-hyprland` version**: confirm the user's installed version implements `GlobalShortcuts` v1+ (it has for a while, but pin a minimum in README).
- **Portal shortcut activation on Path A — single-instance vs. per-press session**: ashpd 0.13 `GlobalShortcuts::create_session` creates a session tied to the daemon process; verify on Hyprland that the session survives idle periods and that re-binding on `reload` works without leaking the previous session.
- **Concurrency on `busy_response = "queue"`**: deferred to Phase 2+. MVP rejects.
- **Whether to add a small `smarty-pants demo "some text"` command** for testing the LLM path without involving Wayland — useful for development; tentatively yes, decide during Phase 1 implementation.

## 11. Reference patterns from Handy

For implementers, the most directly transferable Handy code:

- `src-tauri/src/clipboard.rs` — Wayland injection fallback chain.
- `src-tauri/src/shortcut/handy_keys.rs` — hotkey manager thread pattern (we don't use it directly, but the daemon's request-loop pattern is similar).
- `src-tauri/src/input.rs` — paste primitives, fallback ordering.
- `src-tauri/src/utils.rs` — `is_wayland`, `is_kde_wayland` detection helpers.
- `src-tauri/src/managers/transcription.rs` — engine load/idle/unload lifecycle, directly applicable to the LLM lifecycle in `daemon::llm`.

**Where we go further than Handy**: Handy does not use the XDG GlobalShortcuts portal — it relies on `tauri-plugin-global-shortcut` or its own `handy-keys` crate (libinput / uinput style). smarty-pants makes the portal the *primary* shortcut backend on Hyprland, falling back to compositor-config + CLI on niri/Sway. The `ashpd` GlobalShortcuts API is exercised in the [ashpd-demo](https://github.com/bilelmoussaoui/ashpd) repo (`src/portals/desktop/global_shortcuts.rs`) — that demo is the closest reference for our `daemon::shortcuts` module.
