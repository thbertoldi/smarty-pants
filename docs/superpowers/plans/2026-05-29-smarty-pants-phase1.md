# smarty-pants Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the MVP from [the spec](../specs/2026-05-29-smarty-pants-design.md) Section 9 / Phase 1 — a working keypress→paraphrase loop on Hyprland (via the XDG GlobalShortcuts portal) and niri/Sway (via compositor-config + CLI trigger), with GPU acceleration via Vulkan and a hardcoded Gemma 3 1B Q4 model.

**Architecture:** Cargo workspace with three crates (`core`, `daemon`, `cli`). Long-lived daemon keeps the model resident; two parallel input paths (D-Bus portal signal, Unix socket) both call into a single-flight `pipeline::run(mode)` that reads the selection, asks the LLM to paraphrase, and writes the result back.

**Tech Stack:** Rust 1.75+, `llama-cpp-2` (vulkan), `wl-clipboard-rs`, `ashpd` (global_shortcuts), `tokio`, `clap`, `tracing`, `reqwest`, `sha2`, `insta`.

---

## File structure (created over the course of Phase 1)

```
smarty-pants/
├── Cargo.toml                                  # workspace
├── rust-toolchain.toml                         # pin to stable
├── crates/
│   ├── core/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs                          # re-exports
│   │       ├── paths.rs                        # XDG resolution + $VAR expansion
│   │       ├── protocol.rs                    # Request, Response, ErrorKind, frame helpers
│   │       └── config.rs                       # Config + Mode (serde TOML)
│   │
│   ├── daemon/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs                         # wiring + signal handling + tracing init
│   │       ├── wayland.rs                      # trait Wayland + MockWayland + RealWayland
│   │       ├── selection.rs                    # capture() pipeline step
│   │       ├── inject.rs                       # write() pipeline step
│   │       ├── prompt.rs                       # chat template rendering (Gemma)
│   │       ├── llm.rs                          # trait Llm + EchoLlm + LlamaLlm
│   │       ├── model_download.rs               # ensure_model() with SHA-256 verify
│   │       ├── pipeline.rs                     # run(mode) single-flight orchestrator
│   │       ├── server.rs                       # Unix-socket accept loop
│   │       └── shortcuts.rs                    # ashpd GlobalShortcuts session
│   │
│   └── cli/
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs                         # clap subcommand router
│           ├── trigger.rs                      # connect → send → print
│           ├── status.rs                       # ping daemon
│           └── daemon_cmd.rs                   # start | stop
│
├── tests/                                       (workspace-level integration tests)
│   └── e2e_socket_path.rs                      # daemon-with-stubs + cli, real socket
│
├── docs/superpowers/                           (already exists)
│   ├── specs/2026-05-29-smarty-pants-design.md
│   └── plans/2026-05-29-smarty-pants-phase1.md
│
├── examples/
│   └── config.toml                             # ships in repo for users to copy
│
└── README.md                                   (rewritten in final task)
```

---

## Task 1: Cargo workspace skeleton

**Files:**
- Create: `Cargo.toml` (workspace)
- Create: `rust-toolchain.toml`
- Create: `crates/core/Cargo.toml`, `crates/core/src/lib.rs`
- Create: `crates/daemon/Cargo.toml`, `crates/daemon/src/main.rs`
- Create: `crates/cli/Cargo.toml`, `crates/cli/src/main.rs`

- [ ] **Step 1: Write workspace Cargo.toml**

```toml
[workspace]
resolver = "2"
members  = ["crates/core", "crates/daemon", "crates/cli"]

[workspace.package]
version    = "0.1.0"
edition    = "2021"
license    = "Apache-2.0"
repository = "https://github.com/thbertoldi/smarty-pants"
rust-version = "1.75"

[workspace.dependencies]
# pinned in workspace so every crate picks up the same versions
anyhow              = "1"
thiserror           = "1"
serde               = { version = "1", features = ["derive"] }
serde_json          = "1"
toml                = "0.8"
tracing             = "0.1"
tracing-subscriber  = { version = "0.3", features = ["env-filter", "json"] }
tokio               = { version = "1", features = ["rt-multi-thread", "macros", "net", "io-util", "sync", "signal", "time", "process"] }
tokio-util          = { version = "0.7", features = ["codec"] }
clap                = { version = "4", features = ["derive"] }
directories         = "5"
futures             = "0.3"
async-trait         = "0.1"
```

- [ ] **Step 2: Write `rust-toolchain.toml`**

```toml
[toolchain]
channel    = "stable"
components = ["rustfmt", "clippy"]
```

- [ ] **Step 3: Write `crates/core/Cargo.toml`**

```toml
[package]
name         = "smarty-pants-core"
version.workspace      = true
edition.workspace      = true
license.workspace      = true
repository.workspace   = true
rust-version.workspace = true

[lib]
name = "smarty_pants_core"
path = "src/lib.rs"

[dependencies]
serde       = { workspace = true }
serde_json  = { workspace = true }
toml        = { workspace = true }
directories = { workspace = true }
thiserror   = { workspace = true }
anyhow      = { workspace = true }
```

- [ ] **Step 4: Write empty `crates/core/src/lib.rs`**

```rust
//! Shared types for smarty-pants daemon and CLI.
//!
//! No I/O lives here — only the data definitions that cross the socket
//! and the few path/config helpers both binaries need.
```

- [ ] **Step 5: Write `crates/daemon/Cargo.toml`**

```toml
[package]
name         = "smarty-pants-daemon"
version.workspace      = true
edition.workspace      = true
license.workspace      = true
repository.workspace   = true
rust-version.workspace = true

[[bin]]
name = "smarty-pants-daemon"
path = "src/main.rs"

[features]
default     = ["vulkan"]
vulkan      = ["llama-cpp-2/vulkan"]
cuda        = ["llama-cpp-2/cuda"]
rocm        = ["llama-cpp-2/hipblas"]
live-model  = []   # gates real-LLM smoke tests

[dependencies]
smarty-pants-core   = { path = "../core" }
anyhow              = { workspace = true }
thiserror           = { workspace = true }
serde               = { workspace = true }
serde_json          = { workspace = true }
tracing             = { workspace = true }
tracing-subscriber  = { workspace = true }
tokio               = { workspace = true }
tokio-util          = { workspace = true }
futures             = { workspace = true }
async-trait         = { workspace = true }
directories         = { workspace = true }

llama-cpp-2         = { version = "0.1", default-features = false }
wl-clipboard-rs     = "0.9"
ashpd               = { version = "0.13", default-features = false, features = ["tokio", "global_shortcuts"] }

reqwest             = { version = "0.12", default-features = false, features = ["rustls-tls", "stream"] }
sha2                = "0.10"
hex                 = "0.4"

[dev-dependencies]
insta               = "1"
tempfile            = "3"
```

- [ ] **Step 6: Write minimal `crates/daemon/src/main.rs`**

```rust
fn main() {
    println!("smarty-pants-daemon placeholder; see Task 16 for real wiring");
}
```

- [ ] **Step 7: Write `crates/cli/Cargo.toml`**

```toml
[package]
name         = "smarty-pants-cli"
version.workspace      = true
edition.workspace      = true
license.workspace      = true
repository.workspace   = true
rust-version.workspace = true

[[bin]]
name = "smarty-pants"
path = "src/main.rs"

[dependencies]
smarty-pants-core   = { path = "../core" }
anyhow              = { workspace = true }
serde               = { workspace = true }
serde_json          = { workspace = true }
tokio               = { workspace = true }
clap                = { workspace = true }
```

- [ ] **Step 8: Write minimal `crates/cli/src/main.rs`**

```rust
fn main() {
    println!("smarty-pants CLI placeholder; see Task 17 for real wiring");
}
```

- [ ] **Step 9: Verify it builds**

Run: `cargo check --workspace`
Expected: `Finished … target(s) in …s` with no errors. (Some warnings about unused crates are fine.)

- [ ] **Step 10: Commit**

```bash
git add Cargo.toml rust-toolchain.toml crates/
git commit -m "scaffold cargo workspace with core/daemon/cli crates"
```

---

## Task 2: `core::paths` — XDG resolution + env expansion

**Files:**
- Create: `crates/core/src/paths.rs`
- Modify: `crates/core/src/lib.rs` (add `pub mod paths;`)
- Test: same file (`#[cfg(test)]` module)

- [ ] **Step 1: Write the failing test**

```rust
// crates/core/src/paths.rs
//! XDG directory resolution and `$VAR` expansion for paths in config.

use std::path::PathBuf;

pub fn expand(s: &str) -> PathBuf {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'$' {
            // peek var name (alphanumeric + underscore)
            let start = i + 1;
            let mut end = start;
            while end < bytes.len()
                && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_')
            {
                end += 1;
            }
            if end > start {
                let key = &s[start..end];
                let resolved = resolve_var(key);
                out.push_str(&resolved);
                i = end;
                continue;
            }
        }
        out.push(s.as_bytes()[i] as char);
        i += 1;
    }
    PathBuf::from(out)
}

fn resolve_var(name: &str) -> String {
    if let Ok(v) = std::env::var(name) {
        return v;
    }
    // XDG defaults per spec
    let home = std::env::var("HOME").unwrap_or_default();
    match name {
        "XDG_CONFIG_HOME"  => format!("{home}/.config"),
        "XDG_DATA_HOME"    => format!("{home}/.local/share"),
        "XDG_STATE_HOME"   => format!("{home}/.local/state"),
        "XDG_RUNTIME_DIR"  => format!("/run/user/{}", nix_uid()),
        _                  => format!("${name}"), // leave unknown vars literal
    }
}

fn nix_uid() -> String {
    // SAFETY: getuid is always safe.
    unsafe { libc::getuid() }.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_xdg_config_home_default() {
        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::set_var("HOME", "/home/tester");
        let p = expand("$XDG_CONFIG_HOME/smarty-pants/config.toml");
        assert_eq!(p, PathBuf::from("/home/tester/.config/smarty-pants/config.toml"));
    }

    #[test]
    fn explicit_xdg_var_wins_over_default() {
        std::env::set_var("XDG_CONFIG_HOME", "/somewhere/else");
        let p = expand("$XDG_CONFIG_HOME/smarty-pants");
        assert_eq!(p, PathBuf::from("/somewhere/else/smarty-pants"));
    }

    #[test]
    fn leaves_unknown_vars_literal() {
        let p = expand("$NOT_A_VAR/x");
        assert_eq!(p, PathBuf::from("$NOT_A_VAR/x"));
    }
}
```

Add to `crates/core/Cargo.toml`:

```toml
[dependencies]
# … existing …
libc = "0.2"
```

Add to `crates/core/src/lib.rs`:

```rust
pub mod paths;
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test -p smarty-pants-core --lib paths::`
Expected: 3 passed.

(For TDD strictness you can comment out the function body to see them fail first; the helper is small enough that one cycle suffices.)

- [ ] **Step 3: Commit**

```bash
git add crates/core/
git commit -m "core: XDG path resolution with \$VAR expansion"
```

---

## Task 3: `core::protocol` — wire types + length-prefixed framing

**Files:**
- Create: `crates/core/src/protocol.rs`
- Modify: `crates/core/src/lib.rs`

- [ ] **Step 1: Write the failing test**

```rust
// crates/core/src/protocol.rs
//! Wire types shared between daemon and CLI.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Request {
    Paraphrase { mode: String },
    Status,
    Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Response {
    Ok { generated_chars: usize, ms: u64 },
    Empty,
    Busy,
    ModelLoading,
    Status { healthy: bool, model_loaded: bool, mode_count: usize },
    Error { kind: ErrorKind, message: String },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    Capture,
    Inference,
    Timeout,
    Inject,
    Internal,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_paraphrase_roundtrip() {
        let req = Request::Paraphrase { mode: "rewrite".into() };
        let s = serde_json::to_string(&req).unwrap();
        assert_eq!(s, r#"{"kind":"paraphrase","mode":"rewrite"}"#);
        assert_eq!(serde_json::from_str::<Request>(&s).unwrap(), req);
    }

    #[test]
    fn response_ok_roundtrip() {
        let resp = Response::Ok { generated_chars: 42, ms: 900 };
        let s = serde_json::to_string(&resp).unwrap();
        assert_eq!(serde_json::from_str::<Response>(&s).unwrap(), resp);
    }

    #[test]
    fn response_error_roundtrip() {
        let resp = Response::Error {
            kind: ErrorKind::Inference,
            message: "boom".into(),
        };
        let s = serde_json::to_string(&resp).unwrap();
        assert_eq!(serde_json::from_str::<Response>(&s).unwrap(), resp);
    }
}
```

Add to `crates/core/src/lib.rs`:

```rust
pub mod protocol;
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p smarty-pants-core --lib protocol::`
Expected: 3 passed.

- [ ] **Step 3: Commit**

```bash
git add crates/core/
git commit -m "core: protocol types (Request/Response/ErrorKind) with serde"
```

---

## Task 4: `core::config` — Config + Mode + TOML loading

**Files:**
- Create: `crates/core/src/config.rs`
- Modify: `crates/core/src/lib.rs`
- Create: `crates/core/tests/config_sample.rs` (integration test with a real TOML doc)

- [ ] **Step 1: Write the failing test (integration)**

```rust
// crates/core/tests/config_sample.rs
use smarty_pants_core::config::Config;

#[test]
fn parses_minimal_phase1_config() {
    let toml = r#"
[daemon]
socket_path   = "$XDG_RUNTIME_DIR/smarty-pants.sock"
log_level     = "info"
busy_response = "reject"

[shortcuts]
enabled        = true
require_portal = false
app_id         = "computer.smarty-pants"

[model]
name            = "gemma-3-1b-it-q4_k_m"
context_size    = 4096
threads         = 0
gpu_layers      = -1
gpu_main_device = 0
seed            = 0
max_tokens      = 512
temperature     = 0.7
top_p           = 0.9

[capture]
prefer_primary   = true
ctrl_c_settle_ms = 40
max_chars        = 8000

[inject]
restore_clipboard = true
paste_settle_ms   = 80

[modes.rewrite]
shortcut    = "SUPER+SHIFT+P"
description = "Paraphrase: rewrite in different words"
system      = "Rewrite in different words. Same meaning. Same language."
"#;

    let cfg: Config = toml::from_str(toml).expect("parse");
    assert_eq!(cfg.daemon.log_level, "info");
    assert!(cfg.shortcuts.enabled);
    assert!(!cfg.shortcuts.require_portal);
    assert_eq!(cfg.model.name, "gemma-3-1b-it-q4_k_m");
    assert_eq!(cfg.model.gpu_layers, -1);
    assert_eq!(cfg.capture.ctrl_c_settle_ms, 40);
    assert_eq!(cfg.inject.paste_settle_ms, 80);

    let mode = cfg.modes.get("rewrite").expect("mode present");
    assert_eq!(mode.shortcut.as_deref(), Some("SUPER+SHIFT+P"));
    assert!(mode.system.contains("Same language"));
}

#[test]
fn applies_default_when_minimal_toml() {
    // every field has a sensible default; an empty TOML should parse to the defaults
    let cfg: Config = toml::from_str("").expect("parse defaults");
    assert_eq!(cfg.daemon.log_level, "info");
    assert_eq!(cfg.model.context_size, 4096);
    assert_eq!(cfg.capture.ctrl_c_settle_ms, 40);
}
```

- [ ] **Step 2: Write the implementation**

```rust
// crates/core/src/config.rs
//! TOML config types for smarty-pants. Defaults match the spec, Section 6.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub daemon:    DaemonCfg,
    pub shortcuts: ShortcutsCfg,
    pub model:     ModelCfg,
    pub capture:   CaptureCfg,
    pub inject:    InjectCfg,
    pub modes:     BTreeMap<String, ModeCfg>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct DaemonCfg {
    pub socket_path:   String,   // expanded with paths::expand at use site
    pub log_level:     String,
    pub busy_response: String,   // "reject" only in Phase 1
}
impl Default for DaemonCfg {
    fn default() -> Self {
        Self {
            socket_path:   "$XDG_RUNTIME_DIR/smarty-pants.sock".into(),
            log_level:     "info".into(),
            busy_response: "reject".into(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ShortcutsCfg {
    pub enabled:        bool,
    pub require_portal: bool,
    pub app_id:         String,
}
impl Default for ShortcutsCfg {
    fn default() -> Self {
        Self {
            enabled:        true,
            require_portal: false,
            app_id:         "computer.smarty-pants".into(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ModelCfg {
    pub name:            String,
    pub context_size:    u32,
    pub threads:         u32,
    pub gpu_layers:      i32,    // -1 = all, 0 = CPU
    pub gpu_main_device: u32,
    pub seed:            u32,
    pub max_tokens:      u32,
    pub temperature:     f32,
    pub top_p:           f32,
}
impl Default for ModelCfg {
    fn default() -> Self {
        Self {
            name:            "gemma-3-1b-it-q4_k_m".into(),
            context_size:    4096,
            threads:         0,
            gpu_layers:      -1,
            gpu_main_device: 0,
            seed:            0,
            max_tokens:      512,
            temperature:     0.7,
            top_p:           0.9,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct CaptureCfg {
    pub prefer_primary:   bool,
    pub ctrl_c_settle_ms: u64,
    pub max_chars:        usize,
}
impl Default for CaptureCfg {
    fn default() -> Self {
        Self { prefer_primary: true, ctrl_c_settle_ms: 40, max_chars: 8000 }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct InjectCfg {
    pub restore_clipboard: bool,
    pub paste_settle_ms:   u64,
}
impl Default for InjectCfg {
    fn default() -> Self { Self { restore_clipboard: true, paste_settle_ms: 80 } }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModeCfg {
    pub system:      String,
    #[serde(default)]
    pub shortcut:    Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    // per-mode generation overrides (Phase 2 will read these; tolerated here)
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub top_p:       Option<f32>,
    #[serde(default)]
    pub max_tokens:  Option<u32>,
}

impl Config {
    pub fn from_path(path: &std::path::Path) -> anyhow::Result<Self> {
        let raw = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("read {}: {e}", path.display()))?;
        Ok(toml::from_str(&raw)?)
    }
}
```

Add to `crates/core/src/lib.rs`:

```rust
pub mod config;
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p smarty-pants-core`
Expected: all unit + integration tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/core/
git commit -m "core: TOML Config schema with sane defaults"
```

---

## Task 5: `daemon::wayland` — trait + in-memory mock

**Files:**
- Create: `crates/daemon/src/wayland.rs`
- Modify: `crates/daemon/src/main.rs` (declare module so it compiles)

- [ ] **Step 1: Write the failing test**

```rust
// crates/daemon/src/wayland.rs
//! Abstraction over Wayland clipboard + keystroke synthesis.
//!
//! `RealWayland` lives in the same file behind `cfg(not(test))` so unit
//! tests can be written against `MockWayland` without linking
//! `wl-clipboard-rs`.

use async_trait::async_trait;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardKind { Primary, Regular }

#[async_trait]
pub trait Wayland: Send + Sync + 'static {
    async fn read(&self, kind: ClipboardKind) -> anyhow::Result<Option<String>>;
    async fn write_regular(&self, text: &str) -> anyhow::Result<()>;
    /// Synthesize a single key combo like "ctrl+c" or "ctrl+v".
    async fn type_combo(&self, combo: &str) -> anyhow::Result<()>;
}

// ── in-memory mock for unit tests ─────────────────────────────────────
#[cfg(any(test, feature = "live-model"))]
pub mod mock {
    use super::*;
    use std::sync::Mutex;

    /// Records every interaction; lets a test seed clipboards and assert
    /// the daemon issued the expected key sequence.
    #[derive(Default)]
    pub struct MockWayland {
        pub primary:        Mutex<Option<String>>,
        pub regular:        Mutex<Option<String>>,
        pub combos:         Mutex<Vec<String>>,
        /// If set, a Ctrl+C combo causes `primary` to be copied into `regular`
        /// so the selection capture loop sees something.
        pub ctrl_c_copies_primary_into_regular: bool,
    }

    impl MockWayland {
        pub fn new() -> Self { Self::default() }

        pub fn set_primary(&self, s: Option<&str>) {
            *self.primary.lock().unwrap() = s.map(str::to_owned);
        }
        pub fn set_regular(&self, s: Option<&str>) {
            *self.regular.lock().unwrap() = s.map(str::to_owned);
        }
        pub fn combos(&self) -> Vec<String> {
            self.combos.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl Wayland for MockWayland {
        async fn read(&self, kind: ClipboardKind) -> anyhow::Result<Option<String>> {
            Ok(match kind {
                ClipboardKind::Primary => self.primary.lock().unwrap().clone(),
                ClipboardKind::Regular => self.regular.lock().unwrap().clone(),
            })
        }

        async fn write_regular(&self, text: &str) -> anyhow::Result<()> {
            *self.regular.lock().unwrap() = Some(text.to_owned());
            Ok(())
        }

        async fn type_combo(&self, combo: &str) -> anyhow::Result<()> {
            self.combos.lock().unwrap().push(combo.to_owned());
            if combo == "ctrl+c" && self.ctrl_c_copies_primary_into_regular {
                let p = self.primary.lock().unwrap().clone();
                if let Some(p) = p {
                    *self.regular.lock().unwrap() = Some(p);
                }
            }
            Ok(())
        }
    }
}

// ── tests ─────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use mock::MockWayland;

    #[tokio::test]
    async fn mock_read_returns_seeded_primary() {
        let w = MockWayland::new();
        w.set_primary(Some("hello"));
        assert_eq!(w.read(ClipboardKind::Primary).await.unwrap().as_deref(), Some("hello"));
    }

    #[tokio::test]
    async fn mock_write_then_read_regular() {
        let w = MockWayland::new();
        w.write_regular("paraphrased").await.unwrap();
        assert_eq!(w.read(ClipboardKind::Regular).await.unwrap().as_deref(), Some("paraphrased"));
    }

    #[tokio::test]
    async fn mock_records_combos() {
        let w = MockWayland::new();
        w.type_combo("ctrl+v").await.unwrap();
        assert_eq!(w.combos(), vec!["ctrl+v"]);
    }
}
```

Add to `crates/daemon/src/main.rs`:

```rust
mod wayland;

fn main() {
    println!("smarty-pants-daemon placeholder; see Task 16 for real wiring");
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p smarty-pants-daemon --lib wayland::`
Expected: 3 passed.

- [ ] **Step 3: Commit**

```bash
git add crates/daemon/
git commit -m "daemon: Wayland trait + in-memory mock for unit tests"
```

---

## Task 6: `daemon::selection` — capture pipeline step

**Files:**
- Create: `crates/daemon/src/selection.rs`
- Modify: `crates/daemon/src/main.rs`

- [ ] **Step 1: Write the failing tests**

```rust
// crates/daemon/src/selection.rs
//! Capture the user's currently selected text:
//!   1. read PRIMARY
//!   2. if empty, save regular clipboard, synth Ctrl+C, sleep, read regular
//!   3. caller is responsible for clipboard restore via the returned guard

use crate::wayland::{ClipboardKind, Wayland};
use std::sync::Arc;
use std::time::Duration;

pub struct Captured {
    pub text: String,
}

pub async fn capture(
    wl:                    Arc<dyn Wayland>,
    prefer_primary:        bool,
    ctrl_c_settle_ms:      u64,
    max_chars:             usize,
) -> anyhow::Result<Option<Captured>> {
    if prefer_primary {
        if let Some(s) = wl.read(ClipboardKind::Primary).await? {
            let s = trim_and_cap(s, max_chars);
            if !s.is_empty() {
                return Ok(Some(Captured { text: s }));
            }
        }
    }
    // Fall back to synthesize Ctrl+C. inject::write is responsible for
    // save-and-restore of the regular clipboard, so we don't need to thread
    // a "prior" value through Captured.
    wl.type_combo("ctrl+c").await?;
    tokio::time::sleep(Duration::from_millis(ctrl_c_settle_ms)).await;
    let after = wl.read(ClipboardKind::Regular).await?;
    let captured = after.and_then(|s| {
        let s = trim_and_cap(s, max_chars);
        (!s.is_empty()).then_some(s)
    });
    Ok(captured.map(|text| Captured { text }))
}

fn trim_and_cap(mut s: String, max_chars: usize) -> String {
    let trimmed = s.trim();
    if trimmed.len() != s.len() {
        s = trimmed.to_owned();
    }
    if s.chars().count() > max_chars {
        s = s.chars().take(max_chars).collect();
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wayland::mock::MockWayland;

    fn arc(w: MockWayland) -> Arc<dyn Wayland> { Arc::new(w) }

    #[tokio::test]
    async fn returns_primary_when_present() {
        let w = MockWayland::new();
        w.set_primary(Some("from primary"));
        let result = capture(arc(w), true, 0, 8000).await.unwrap().unwrap();
        assert_eq!(result.text, "from primary");
    }

    #[tokio::test]
    async fn falls_back_to_ctrl_c_when_primary_empty() {
        // Capture should issue Ctrl+C and then read what the app placed
        // on the regular clipboard. We simulate that by pre-loading the
        // mock with primary empty and the regular clipboard containing
        // what the user actually had highlighted (as if the app already
        // responded to Ctrl+C).
        let w = MockWayland::new();
        w.set_primary(None);
        w.set_regular(Some("highlighted text"));
        let arc_w = Arc::new(w);
        let result = capture(arc_w.clone(), true, 0, 8000).await.unwrap().unwrap();
        assert_eq!(result.text, "highlighted text");
        assert!(arc_w.combos().contains(&"ctrl+c".to_owned()));
    }

    #[tokio::test]
    async fn returns_none_when_nothing_selected() {
        let w = MockWayland::new();
        w.set_primary(None);
        w.set_regular(None);
        let result = capture(arc(w), true, 0, 8000).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn trims_and_caps_long_selection() {
        let w = MockWayland::new();
        let big = "x".repeat(10_000);
        w.set_primary(Some(&big));
        let result = capture(arc(w), true, 0, 8000).await.unwrap().unwrap();
        assert_eq!(result.text.chars().count(), 8000);
    }
}
```

Add to `crates/daemon/src/main.rs`:

```rust
mod selection;
mod wayland;
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p smarty-pants-daemon --lib selection::`
Expected: 4 passed.

- [ ] **Step 3: Commit**

```bash
git add crates/daemon/
git commit -m "daemon: selection capture with PRIMARY → Ctrl+C fallback"
```

---

## Task 7: `daemon::inject` — write back

**Files:**
- Create: `crates/daemon/src/inject.rs`
- Modify: `crates/daemon/src/main.rs`

- [ ] **Step 1: Write the failing tests**

```rust
// crates/daemon/src/inject.rs
//! Write the paraphrased text back: save clipboard → set → Ctrl+V → restore.
//!
//! Inject owns the full save/restore cycle for the regular clipboard.
//! `selection::capture` does NOT thread the prior clipboard through —
//! whatever is on the clipboard at the moment write() is called is what
//! we restore. This is correct whether capture went via PRIMARY (in which
//! case the user's clipboard is untouched) or via Ctrl+C (in which case
//! the user's clipboard already holds the captured selection).

use crate::wayland::{ClipboardKind, Wayland};
use std::sync::Arc;
use std::time::Duration;

pub async fn write(
    wl:                Arc<dyn Wayland>,
    generated:         &str,
    paste_settle_ms:   u64,
    restore_clipboard: bool,
) -> anyhow::Result<()> {
    let prior = if restore_clipboard {
        wl.read(ClipboardKind::Regular).await?
    } else {
        None
    };
    wl.write_regular(generated).await?;
    wl.type_combo("ctrl+v").await?;
    tokio::time::sleep(Duration::from_millis(paste_settle_ms)).await;

    if restore_clipboard {
        if let Some(prior) = prior {
            wl.write_regular(&prior).await?;
        }
        // If prior was None, the user had nothing on the clipboard; we leave
        // the generated text on it. This is a deliberate trade-off — the
        // wl-clipboard protocol has no "clear clipboard" primitive that
        // wl-clipboard-rs exposes cleanly, and leaving the latest paste on
        // the clipboard is consistent with normal Ctrl+V behavior.
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wayland::mock::MockWayland;

    #[tokio::test]
    async fn writes_then_pastes() {
        let w = Arc::new(MockWayland::new());
        write(w.clone(), "paraphrased", 0, false).await.unwrap();
        assert_eq!(
            w.read(ClipboardKind::Regular).await.unwrap().as_deref(),
            Some("paraphrased")
        );
        assert_eq!(w.combos(), vec!["ctrl+v"]);
    }

    #[tokio::test]
    async fn restores_prior_clipboard_when_enabled() {
        let w = Arc::new(MockWayland::new());
        w.write_regular("ORIGINAL").await.unwrap();
        write(w.clone(), "paraphrased", 0, true).await.unwrap();
        assert_eq!(
            w.read(ClipboardKind::Regular).await.unwrap().as_deref(),
            Some("ORIGINAL")
        );
    }

    #[tokio::test]
    async fn does_not_restore_when_disabled() {
        let w = Arc::new(MockWayland::new());
        w.write_regular("ORIGINAL").await.unwrap();
        write(w.clone(), "paraphrased", 0, false).await.unwrap();
        assert_eq!(
            w.read(ClipboardKind::Regular).await.unwrap().as_deref(),
            Some("paraphrased")
        );
    }
}
```

Add to `crates/daemon/src/main.rs`:

```rust
mod inject;
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p smarty-pants-daemon --lib inject::`
Expected: 3 passed.

- [ ] **Step 3: Commit**

```bash
git add crates/daemon/
git commit -m "daemon: inject (set clipboard, paste, restore)"
```

---

## Task 8: `daemon::wayland::real` — real Wayland implementation

**Files:**
- Modify: `crates/daemon/src/wayland.rs` (append the real impl)

- [ ] **Step 1: Append the real implementation**

```rust
// crates/daemon/src/wayland.rs — append at end

// ── real implementation backed by wl-clipboard-rs + wtype subprocess ──
pub mod real {
    use super::*;
    use std::io::Read;
    use tokio::process::Command;

    pub struct RealWayland;

    impl RealWayland {
        pub fn new() -> Self { Self }
    }

    #[async_trait]
    impl Wayland for RealWayland {
        async fn read(&self, kind: ClipboardKind) -> anyhow::Result<Option<String>> {
            use wl_clipboard_rs::paste::{
                get_contents, ClipboardType, Error, MimeType, Seat,
            };
            let target = match kind {
                ClipboardKind::Primary => ClipboardType::Primary,
                ClipboardKind::Regular => ClipboardType::Regular,
            };
            // wl-clipboard-rs is sync — run on blocking pool
            let result = tokio::task::spawn_blocking(move || {
                match get_contents(target, Seat::Unspecified, MimeType::Text) {
                    Ok((mut pipe, _)) => {
                        let mut buf = String::new();
                        pipe.read_to_string(&mut buf).map_err(|e| {
                            anyhow::anyhow!("read clipboard pipe: {e}")
                        })?;
                        Ok::<Option<String>, anyhow::Error>(Some(buf))
                    }
                    Err(Error::NoSeats) | Err(Error::ClipboardEmpty) | Err(Error::NoMimeType) => Ok(None),
                    Err(e) => Err(anyhow::anyhow!("wl-clipboard: {e}")),
                }
            })
            .await
            .map_err(|e| anyhow::anyhow!("join: {e}"))??;
            Ok(result)
        }

        async fn write_regular(&self, text: &str) -> anyhow::Result<()> {
            use wl_clipboard_rs::copy::{MimeType, Options, Source};
            let text = text.to_owned();
            tokio::task::spawn_blocking(move || {
                let opts = Options::new();
                opts.copy(Source::Bytes(text.into_bytes().into()), MimeType::Text)
                    .map_err(|e| anyhow::anyhow!("wl-copy: {e}"))
            })
            .await
            .map_err(|e| anyhow::anyhow!("join: {e}"))?
        }

        async fn type_combo(&self, combo: &str) -> anyhow::Result<()> {
            // combo formatted as "ctrl+v" or "ctrl+c"
            let parts: Vec<&str> = combo.split('+').collect();
            // Build wtype args: -M MOD ... -P KEY -p KEY -m MOD ...
            // For Ctrl+V: wtype -M ctrl v -m ctrl
            // We use the simpler invocation that wtype supports.
            let (mods, key): (Vec<&str>, &str) = match parts.split_last() {
                Some((last, rest)) => (rest.to_vec(), *last),
                None => return Err(anyhow::anyhow!("empty combo")),
            };
            let mut cmd = Command::new("wtype");
            for m in &mods {
                cmd.arg("-M").arg(m);
            }
            cmd.arg(key);
            for m in &mods {
                cmd.arg("-m").arg(m);
            }
            let status = cmd.status().await
                .map_err(|e| anyhow::anyhow!("spawn wtype: {e}"))?;
            if !status.success() {
                return Err(anyhow::anyhow!("wtype exited {status}"));
            }
            Ok(())
        }
    }
}
```

- [ ] **Step 2: Sanity-check it compiles**

Run: `cargo check -p smarty-pants-daemon`
Expected: builds clean.

(We do not unit-test the real impl — it requires a running Wayland session. The E2E test in Task 21 covers it indirectly when running under a real compositor.)

- [ ] **Step 3: Commit**

```bash
git add crates/daemon/src/wayland.rs
git commit -m "daemon: real Wayland impl (wl-clipboard-rs + wtype subprocess)"
```

---

## Task 9: `daemon::prompt` — chat template rendering

**Files:**
- Create: `crates/daemon/src/prompt.rs`
- Create: `crates/daemon/src/snapshots/` (insta default location)
- Modify: `crates/daemon/src/main.rs`

- [ ] **Step 1: Write the failing test with snapshot**

```rust
// crates/daemon/src/prompt.rs
//! Render a chat-templated prompt for one of the supported templates.

#[derive(Debug, Clone, Copy)]
pub enum Template { Gemma }

pub fn render(template: Template, system: &str, user: &str) -> String {
    match template {
        Template::Gemma => render_gemma(system, user),
    }
}

/// Gemma 2/3 chat template — no system role, so we inject the system
/// instructions as the leading user turn, then the actual user content.
fn render_gemma(system: &str, user: &str) -> String {
    format!(
        "<start_of_turn>user\n{system}\n\n---\n\n{user}<end_of_turn>\n<start_of_turn>model\n",
        system = system.trim(),
        user   = user.trim()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gemma_snapshot() {
        let out = render(
            Template::Gemma,
            "Rewrite in different words. Same meaning. Same language.",
            "The quick brown fox jumps over the lazy dog.",
        );
        insta::assert_snapshot!(out);
    }
}
```

Add to `crates/daemon/src/main.rs`:

```rust
mod prompt;
```

- [ ] **Step 2: Run and accept the snapshot**

Run: `cargo test -p smarty-pants-daemon --lib prompt:: -- --include-ignored`
Expected: snapshot test fails the first time with `pending snapshot — review with cargo insta review`.

Run: `cargo insta accept`  (install with `cargo install cargo-insta` if missing)
Then re-run the test; it should pass.

- [ ] **Step 3: Commit**

```bash
git add crates/daemon/src/prompt.rs crates/daemon/src/snapshots/
git commit -m "daemon: prompt rendering with Gemma chat template"
```

---

## Task 10: `daemon::llm` — trait + Echo stub

**Files:**
- Create: `crates/daemon/src/llm.rs`
- Modify: `crates/daemon/src/main.rs`

- [ ] **Step 1: Write the failing test**

```rust
// crates/daemon/src/llm.rs
//! Generation backend trait + a deterministic stub for tests.

use async_trait::async_trait;

#[derive(Debug, Clone)]
pub struct GenerationParams {
    pub max_tokens:  u32,
    pub temperature: f32,
    pub top_p:       f32,
    pub seed:        u32,
}
impl Default for GenerationParams {
    fn default() -> Self {
        Self { max_tokens: 512, temperature: 0.7, top_p: 0.9, seed: 0 }
    }
}

#[async_trait]
pub trait Llm: Send + Sync + 'static {
    /// `prompt` is the fully chat-templated text. Implementations must
    /// return only the model's generated continuation, NOT echoing the prompt.
    async fn generate(&self, prompt: &str, params: &GenerationParams) -> anyhow::Result<String>;
}

// ── deterministic stub for tests ──────────────────────────────────────
pub struct EchoLlm;

#[async_trait]
impl Llm for EchoLlm {
    async fn generate(&self, prompt: &str, _: &GenerationParams) -> anyhow::Result<String> {
        // Extract the last user-turn content for a predictable transformation.
        // Tests only need a deterministic output, not a real paraphrase.
        let tail = prompt
            .rsplit("---\n\n")
            .next()
            .and_then(|s| s.strip_suffix("<end_of_turn>\n<start_of_turn>model\n"))
            .unwrap_or(prompt)
            .trim();
        Ok(format!("[paraphrased] {tail}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn echo_stub_returns_predictable_string() {
        let llm = EchoLlm;
        let prompt = "<start_of_turn>user\nrewrite this.\n\n---\n\nhello world<end_of_turn>\n<start_of_turn>model\n";
        let out = llm.generate(prompt, &GenerationParams::default()).await.unwrap();
        assert_eq!(out, "[paraphrased] hello world");
    }
}
```

Add to `crates/daemon/src/main.rs`:

```rust
mod llm;
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p smarty-pants-daemon --lib llm::`
Expected: 1 passed.

- [ ] **Step 3: Commit**

```bash
git add crates/daemon/
git commit -m "daemon: Llm trait + EchoLlm stub for tests"
```

---

## Task 11: `daemon::model_download` — fetch + SHA-256 verify

**Files:**
- Create: `crates/daemon/src/model_download.rs`
- Modify: `crates/daemon/src/main.rs`

- [ ] **Step 1: Write the failing tests**

```rust
// crates/daemon/src/model_download.rs
//! Fetch a model file to disk and verify its SHA-256.
//!
//! Phase 1 ships a single hardcoded model. The URL+hash live as
//! associated constants below; Phase 2 moves them into config.

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

pub struct ModelSpec {
    pub key:    &'static str,
    pub url:    &'static str,
    pub sha256: &'static str, // hex lowercase
    pub size:   u64,
}

pub const GEMMA_3_1B_IT_Q4_K_M: ModelSpec = ModelSpec {
    key:    "gemma-3-1b-it-q4_k_m",
    url:    "https://huggingface.co/unsloth/gemma-3-1b-it-GGUF/resolve/main/gemma-3-1b-it-Q4_K_M.gguf",
    // NOTE for the implementer: fetch the real SHA at implementation time
    // (curl -sL "<url>" | sha256sum) and paste it here. The string below
    // is a placeholder; the verify step will reject downloads until it's
    // replaced with the actual hash.
    sha256: "0000000000000000000000000000000000000000000000000000000000000000",
    size:   806_000_000,
};

pub async fn ensure_model(
    spec:     &ModelSpec,
    data_dir: &Path,
) -> anyhow::Result<PathBuf> {
    let dst = data_dir.join(format!("{}.gguf", spec.key));
    if dst.exists() && verify_sha256(&dst, spec.sha256).await.unwrap_or(false) {
        tracing::info!(path = %dst.display(), "model already present and verified");
        return Ok(dst);
    }
    download(&spec.url, &dst).await?;
    if !verify_sha256(&dst, spec.sha256).await? {
        let _ = tokio::fs::remove_file(&dst).await;
        anyhow::bail!("downloaded model sha256 does not match expected {}", spec.sha256);
    }
    tracing::info!(path = %dst.display(), "model downloaded and verified");
    Ok(dst)
}

async fn download(url: &str, dst: &Path) -> anyhow::Result<()> {
    if let Some(parent) = dst.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let tmp = dst.with_extension("gguf.partial");
    let resp = reqwest::get(url).await?.error_for_status()?;
    let mut stream = resp.bytes_stream();
    let mut file = tokio::fs::File::create(&tmp).await?;
    use futures::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).await?;
    }
    file.sync_all().await?;
    tokio::fs::rename(&tmp, dst).await?;
    Ok(())
}

async fn verify_sha256(path: &Path, expected_hex: &str) -> anyhow::Result<bool> {
    let bytes = tokio::fs::read(path).await?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let got = hex::encode(hasher.finalize());
    Ok(got.eq_ignore_ascii_case(expected_hex))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn verify_sha256_matches() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("x.bin");
        tokio::fs::write(&p, b"hello").await.unwrap();
        // sha256("hello") = 2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824
        assert!(verify_sha256(
            &p,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824",
        ).await.unwrap());
    }

    #[tokio::test]
    async fn verify_sha256_rejects_mismatch() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("x.bin");
        tokio::fs::write(&p, b"hello").await.unwrap();
        assert!(!verify_sha256(&p, "deadbeef").await.unwrap());
    }
}
```

Add to `crates/daemon/src/main.rs`:

```rust
mod model_download;
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p smarty-pants-daemon --lib model_download::`
Expected: 2 passed.

- [ ] **Step 3: Pin the real SHA-256**

Run from a shell with internet access:
```bash
curl -sL "https://huggingface.co/unsloth/gemma-3-1b-it-GGUF/resolve/main/gemma-3-1b-it-Q4_K_M.gguf" | sha256sum
```
Paste the 64-hex output into `GEMMA_3_1B_IT_Q4_K_M.sha256` replacing the zeros placeholder.

- [ ] **Step 4: Commit**

```bash
git add crates/daemon/
git commit -m "daemon: model download with resumable fetch and SHA-256 verify"
```

---

## Task 12: `daemon::llm::LlamaLlm` — real llama-cpp-2 implementation

**Files:**
- Modify: `crates/daemon/src/llm.rs` (append `LlamaLlm`)

- [ ] **Step 1: Append the real implementation**

```rust
// crates/daemon/src/llm.rs — append at end

use llama_cpp_2::{
    context::params::LlamaContextParams,
    llama_backend::LlamaBackend,
    llama_batch::LlamaBatch,
    model::{params::LlamaModelParams, AddBos, LlamaModel, Special},
    sampling::LlamaSampler,
};
use std::num::NonZeroU32;
use std::path::Path;
use std::sync::Arc;

pub struct LlamaLlm {
    backend: Arc<LlamaBackend>,
    model:   Arc<LlamaModel>,
    n_ctx:   u32,
    n_threads: i32,
}

impl LlamaLlm {
    pub fn load(
        backend:         Arc<LlamaBackend>,
        model_path:      &Path,
        n_ctx:           u32,
        n_threads:       u32,
        gpu_layers:      i32,
        gpu_main_device: u32,
    ) -> anyhow::Result<Self> {
        // Probe whether a GPU device is actually present. If the user
        // requested GPU offload (gpu_layers != 0) but none is detected,
        // warn and degrade to CPU rather than refusing to start.
        let gpu_available = gpu_devices_present();
        let effective_gpu_layers = match (gpu_layers, gpu_available) {
            (0, _) => {
                tracing::info!("gpu_layers = 0; using CPU only");
                0
            }
            (n, false) => {
                tracing::warn!(
                    requested = n,
                    "gpu_layers requested but no GPU device detected — falling back to CPU"
                );
                0
            }
            (n, true) => {
                let layers = if n < 0 { i32::MAX } else { n };
                tracing::info!(layers, gpu_main_device, "offloading to GPU");
                layers
            }
        };

        let mut params = LlamaModelParams::default();
        params = params.with_main_gpu(gpu_main_device as i32);
        params = params.with_n_gpu_layers(effective_gpu_layers as u32);

        let model = LlamaModel::load_from_file(&backend, model_path, &params)
            .map_err(|e| anyhow::anyhow!("load gguf: {e}"))?;
        tracing::info!(
            n_params = model.n_params(),
            n_layer  = model.n_layer(),
            n_ctx_train = model.n_ctx_train(),
            "model loaded"
        );
        let n_threads = if n_threads == 0 {
            (num_cpus_get_minus_one()) as i32
        } else {
            n_threads as i32
        };
        Ok(Self { backend, model: Arc::new(model), n_ctx, n_threads })
    }
}

fn num_cpus_get_minus_one() -> usize {
    std::thread::available_parallelism().map(|n| n.get().saturating_sub(1).max(1)).unwrap_or(1)
}

/// Heuristic GPU presence probe. With the `vulkan` feature, llama.cpp logs
/// detected devices during `LlamaBackend::init()`; if you want strict
/// detection, parse those logs. The conservative fallback below assumes a
/// GPU is present if either `/dev/dri/renderD128` exists (works on every
/// Linux GPU stack — Intel/AMD/NVIDIA — as long as a render node is created).
fn gpu_devices_present() -> bool {
    std::path::Path::new("/dev/dri/renderD128").exists()
}

#[async_trait]
impl Llm for LlamaLlm {
    async fn generate(&self, prompt: &str, params: &GenerationParams) -> anyhow::Result<String> {
        // llama-cpp-2 is sync; run on blocking pool
        let backend  = self.backend.clone();
        let model    = self.model.clone();
        let n_ctx    = self.n_ctx;
        let n_threads = self.n_threads;
        let prompt   = prompt.to_owned();
        let params   = params.clone();

        tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
            let ctx_params = LlamaContextParams::default()
                .with_n_ctx(NonZeroU32::new(n_ctx))
                .with_n_threads(n_threads)
                .with_n_threads_batch(n_threads);
            let mut ctx = model.new_context(&backend, ctx_params)
                .map_err(|e| anyhow::anyhow!("new_context: {e}"))?;

            let tokens = model.str_to_token(&prompt, AddBos::Always)
                .map_err(|e| anyhow::anyhow!("tokenize: {e}"))?;
            let mut batch = LlamaBatch::new(tokens.len().max(512), 1);
            for (i, t) in tokens.iter().enumerate() {
                batch.add(*t, i as i32, &[0], i == tokens.len() - 1)
                    .map_err(|e| anyhow::anyhow!("batch: {e}"))?;
            }
            ctx.decode(&mut batch).map_err(|e| anyhow::anyhow!("decode prompt: {e}"))?;

            let mut sampler = LlamaSampler::chain_simple([
                LlamaSampler::temp(params.temperature),
                LlamaSampler::top_p(params.top_p, 1),
                LlamaSampler::dist(if params.seed == 0 {
                    rand_seed()
                } else {
                    params.seed
                }),
            ]);

            let mut out = String::new();
            let mut n_cur = batch.n_tokens();
            let mut decoder = encoding_rs::UTF_8.new_decoder();
            for _ in 0..params.max_tokens {
                let token = sampler.sample(&ctx, batch.n_tokens() - 1);
                if model.is_eog_token(token) { break; }
                sampler.accept(token);
                let piece = model
                    .token_to_piece(token, &mut decoder, true, Some(Special::Tokenize))
                    .map_err(|e| anyhow::anyhow!("detokenize: {e}"))?;
                out.push_str(&piece);
                batch.clear();
                batch.add(token, n_cur, &[0], true)
                    .map_err(|e| anyhow::anyhow!("batch add: {e}"))?;
                n_cur += 1;
                ctx.decode(&mut batch).map_err(|e| anyhow::anyhow!("decode: {e}"))?;
            }
            Ok(out.trim().to_owned())
        })
        .await
        .map_err(|e| anyhow::anyhow!("join: {e}"))?
    }
}

fn rand_seed() -> u32 {
    // Use system time nanoseconds as a quick non-crypto seed
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(1)
}
```

Add to `crates/daemon/Cargo.toml`:

```toml
[dependencies]
# … existing …
encoding_rs = "0.8"
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p smarty-pants-daemon`
Expected: builds clean. (Vulkan headers required — see README, Task 22.)

- [ ] **Step 3: (Manual, optional) smoke-test against a real model**

This is gated behind `--features live-model`. Skip in default CI:

```bash
cargo test -p smarty-pants-daemon --features live-model --release \
    --test live_model_smoke -- --nocapture
```

A live test goes in `crates/daemon/tests/live_model_smoke.rs` if you want to author one — Phase 1 acceptance treats Task 21's E2E as sufficient.

- [ ] **Step 4: Commit**

```bash
git add crates/daemon/
git commit -m "daemon: LlamaLlm — real generation backed by llama-cpp-2 vulkan"
```

---

## Task 13: `daemon::pipeline` — single-flight orchestrator

**Files:**
- Create: `crates/daemon/src/pipeline.rs`
- Modify: `crates/daemon/src/main.rs`

- [ ] **Step 1: Write the failing tests**

```rust
// crates/daemon/src/pipeline.rs
//! Glue capture → generate → inject under a single-flight mutex.

use crate::{
    inject, llm::{GenerationParams, Llm}, prompt::{self, Template}, selection, wayland::Wayland,
};
use smarty_pants_core::{config::Config, protocol::{ErrorKind, Response}};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

pub struct Pipeline {
    wl:       Arc<dyn Wayland>,
    llm:      Arc<dyn Llm>,
    cfg:      Arc<Config>,
    inflight: Mutex<()>,
}

impl Pipeline {
    pub fn new(wl: Arc<dyn Wayland>, llm: Arc<dyn Llm>, cfg: Arc<Config>) -> Self {
        Self { wl, llm, cfg, inflight: Mutex::new(()) }
    }

    pub async fn run(&self, mode_name: &str) -> Response {
        // Single-flight: if we cannot grab the lock immediately, return Busy.
        let _guard = match self.inflight.try_lock() {
            Ok(g) => g,
            Err(_) => return Response::Busy,
        };
        let started = Instant::now();
        match self.run_inner(mode_name).await {
            Ok(Some(chars)) => Response::Ok {
                generated_chars: chars,
                ms: started.elapsed().as_millis() as u64,
            },
            Ok(None) => Response::Empty,
            Err((kind, msg)) => Response::Error { kind, message: msg },
        }
    }

    async fn run_inner(&self, mode_name: &str) -> Result<Option<usize>, (ErrorKind, String)> {
        let mode = self.cfg.modes.get(mode_name).ok_or_else(|| {
            (ErrorKind::Internal, format!("unknown mode: {mode_name}"))
        })?;
        let captured = selection::capture(
            self.wl.clone(),
            self.cfg.capture.prefer_primary,
            self.cfg.capture.ctrl_c_settle_ms,
            self.cfg.capture.max_chars,
        )
        .await
        .map_err(|e| (ErrorKind::Capture, e.to_string()))?;
        let Some(captured) = captured else { return Ok(None) };

        let prompt = prompt::render(Template::Gemma, &mode.system, &captured.text);
        let params = GenerationParams {
            max_tokens:  mode.max_tokens.unwrap_or(self.cfg.model.max_tokens),
            temperature: mode.temperature.unwrap_or(self.cfg.model.temperature),
            top_p:       mode.top_p.unwrap_or(self.cfg.model.top_p),
            seed:        self.cfg.model.seed,
        };
        let generated = self.llm.generate(&prompt, &params).await
            .map_err(|e| (ErrorKind::Inference, e.to_string()))?;

        inject::write(
            self.wl.clone(),
            &generated,
            self.cfg.inject.paste_settle_ms,
            self.cfg.inject.restore_clipboard,
        )
        .await
        .map_err(|e| (ErrorKind::Inject, e.to_string()))?;

        Ok(Some(generated.chars().count()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{llm::EchoLlm, wayland::mock::MockWayland};
    use smarty_pants_core::config::{Config, ModeCfg};

    fn cfg_with_mode() -> Arc<Config> {
        let mut cfg = Config::default();
        cfg.modes.insert("rewrite".into(), ModeCfg {
            system:      "Rewrite in different words. Same language.".into(),
            shortcut:    None,
            description: None,
            temperature: None,
            top_p:       None,
            max_tokens:  None,
        });
        Arc::new(cfg)
    }

    #[tokio::test]
    async fn happy_path_returns_ok_and_pastes() {
        let wl = Arc::new(MockWayland::new());
        wl.set_primary(Some("Hello world."));
        let pipe = Pipeline::new(wl.clone(), Arc::new(EchoLlm), cfg_with_mode());

        let resp = pipe.run("rewrite").await;
        match resp {
            Response::Ok { generated_chars, .. } => assert!(generated_chars > 0),
            other => panic!("expected Ok, got {other:?}"),
        }
        // EchoLlm output should now be on the regular clipboard
        let v = wl.read(crate::wayland::ClipboardKind::Regular).await.unwrap();
        assert_eq!(v.as_deref(), Some("[paraphrased] Hello world."));
    }

    #[tokio::test]
    async fn empty_selection_returns_empty() {
        let wl = Arc::new(MockWayland::new());
        let pipe = Pipeline::new(wl, Arc::new(EchoLlm), cfg_with_mode());
        assert!(matches!(pipe.run("rewrite").await, Response::Empty));
    }

    #[tokio::test]
    async fn unknown_mode_returns_internal_error() {
        let wl = Arc::new(MockWayland::new());
        wl.set_primary(Some("x"));
        let pipe = Pipeline::new(wl, Arc::new(EchoLlm), cfg_with_mode());
        let resp = pipe.run("nope").await;
        assert!(matches!(resp, Response::Error { kind: ErrorKind::Internal, .. }));
    }

    #[tokio::test]
    async fn concurrent_second_call_returns_busy() {
        use std::time::Duration;
        struct SlowLlm;
        #[async_trait::async_trait]
        impl Llm for SlowLlm {
            async fn generate(&self, _: &str, _: &GenerationParams) -> anyhow::Result<String> {
                tokio::time::sleep(Duration::from_millis(50)).await;
                Ok("slow".into())
            }
        }
        let wl = Arc::new(MockWayland::new());
        wl.set_primary(Some("x"));
        let pipe = Arc::new(Pipeline::new(wl, Arc::new(SlowLlm), cfg_with_mode()));
        let a = tokio::spawn({ let p = pipe.clone(); async move { p.run("rewrite").await } });
        tokio::time::sleep(Duration::from_millis(5)).await;
        let b = pipe.run("rewrite").await;
        let _ = a.await.unwrap();
        assert!(matches!(b, Response::Busy));
    }
}
```

Add to `crates/daemon/src/main.rs`:

```rust
mod pipeline;
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p smarty-pants-daemon --lib pipeline::`
Expected: 4 passed.

- [ ] **Step 3: Commit**

```bash
git add crates/daemon/
git commit -m "daemon: pipeline (single-flight capture→llm→inject)"
```

---

## Task 14: `daemon::server` — Unix socket accept loop

**Files:**
- Create: `crates/daemon/src/server.rs`
- Modify: `crates/daemon/src/main.rs`

- [ ] **Step 1: Write the failing test**

```rust
// crates/daemon/src/server.rs
//! Unix-socket server. One newline-delimited JSON request per connection.

use crate::pipeline::Pipeline;
use smarty_pants_core::protocol::{Request, Response};
use std::path::Path;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};

pub struct Server {
    listener: UnixListener,
    pipeline: Arc<Pipeline>,
}

impl Server {
    pub fn bind(socket_path: &Path, pipeline: Arc<Pipeline>) -> anyhow::Result<Self> {
        // remove stale socket
        let _ = std::fs::remove_file(socket_path);
        if let Some(parent) = socket_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let listener = UnixListener::bind(socket_path)?;
        Ok(Self { listener, pipeline })
    }

    pub async fn serve(self) -> anyhow::Result<()> {
        loop {
            let (stream, _) = self.listener.accept().await?;
            let pipeline = self.pipeline.clone();
            tokio::spawn(async move {
                if let Err(e) = handle(stream, pipeline).await {
                    tracing::warn!(error = %e, "client error");
                }
            });
        }
    }
}

async fn handle(stream: UnixStream, pipeline: Arc<Pipeline>) -> anyhow::Result<()> {
    let (read, mut write) = stream.into_split();
    let mut reader = BufReader::new(read);
    let mut line = String::new();
    reader.read_line(&mut line).await?;
    let req: Request = serde_json::from_str(line.trim())?;

    let resp: Response = match req {
        Request::Paraphrase { mode } => pipeline.run(&mode).await,
        Request::Status => Response::Status {
            healthy: true,
            model_loaded: true, // refined in Task 16 (main wiring)
            mode_count: pipeline_mode_count(&pipeline),
        },
        Request::Shutdown => Response::Ok { generated_chars: 0, ms: 0 },
    };

    let body = serde_json::to_string(&resp)?;
    write.write_all(body.as_bytes()).await?;
    write.write_all(b"\n").await?;
    write.shutdown().await?;
    Ok(())
}

fn pipeline_mode_count(_p: &Pipeline) -> usize { 1 /* Phase 1: hardcoded rewrite */ }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{llm::EchoLlm, wayland::mock::MockWayland};
    use smarty_pants_core::config::{Config, ModeCfg};
    use tempfile::TempDir;
    use tokio::io::AsyncReadExt;

    #[tokio::test]
    async fn round_trip_paraphrase() {
        let tmp = TempDir::new().unwrap();
        let sock = tmp.path().join("sp.sock");

        let wl = Arc::new(MockWayland::new());
        wl.set_primary(Some("hi"));
        let mut cfg = Config::default();
        cfg.modes.insert("rewrite".into(), ModeCfg {
            system: "rewrite".into(),
            shortcut: None, description: None,
            temperature: None, top_p: None, max_tokens: None,
        });
        let pipe = Arc::new(Pipeline::new(wl, Arc::new(EchoLlm), Arc::new(cfg)));
        let server = Server::bind(&sock, pipe).unwrap();
        let h = tokio::spawn(async move { let _ = server.serve().await; });

        // give it a moment to be listening
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;

        let mut client = UnixStream::connect(&sock).await.unwrap();
        client.write_all(b"{\"kind\":\"paraphrase\",\"mode\":\"rewrite\"}\n").await.unwrap();
        let mut buf = String::new();
        client.read_to_string(&mut buf).await.unwrap();
        let resp: Response = serde_json::from_str(buf.trim()).unwrap();
        assert!(matches!(resp, Response::Ok { .. }));

        h.abort();
    }
}
```

Add to `crates/daemon/src/main.rs`:

```rust
mod server;
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p smarty-pants-daemon --lib server::`
Expected: 1 passed.

- [ ] **Step 3: Commit**

```bash
git add crates/daemon/
git commit -m "daemon: server (Unix socket accept loop + per-conn handler)"
```

---

## Task 15: `daemon::shortcuts` — ashpd GlobalShortcuts integration

**Files:**
- Create: `crates/daemon/src/shortcuts.rs`
- Modify: `crates/daemon/src/main.rs`

- [ ] **Step 1: Write the falling tests for the dispatcher (the easy half)**

```rust
// crates/daemon/src/shortcuts.rs
//! XDG GlobalShortcuts portal integration.
//!
//! Two pieces:
//!   • `Dispatcher` — turns "shortcut id activated" events into pipeline runs.
//!     This is unit-testable without a portal.
//!   • `run_session` — async function that owns the ashpd session, binds the
//!     shortcuts, and forwards Activated events to a Dispatcher. Skipped at
//!     test time; exercised by Phase 1 manual smoke + the E2E task.

use crate::pipeline::Pipeline;
use smarty_pants_core::config::Config;
use std::sync::Arc;

pub struct Dispatcher {
    pipeline: Arc<Pipeline>,
}

impl Dispatcher {
    pub fn new(pipeline: Arc<Pipeline>) -> Self { Self { pipeline } }

    /// Called for every Activated event. `id` is the shortcut id, which we
    /// equate with the mode name.
    pub async fn handle_activation(&self, id: &str) {
        tracing::info!(shortcut_id = id, "portal activation");
        let resp = self.pipeline.run(id).await;
        tracing::info!(?resp, "portal pipeline result");
    }
}

/// Returns true if the portal is reachable AND BindShortcuts succeeds.
/// On false, callers should log info and serve socket-only.
pub async fn run_session(
    cfg:        &Config,
    dispatcher: Arc<Dispatcher>,
) -> anyhow::Result<()> {
    use ashpd::desktop::global_shortcuts::{GlobalShortcuts, NewShortcut};
    use futures::StreamExt;

    if !cfg.shortcuts.enabled {
        tracing::info!("shortcuts.enabled = false; portal session skipped");
        return Ok(());
    }

    let portal = match GlobalShortcuts::new().await {
        Ok(p) => p,
        Err(e) => {
            if cfg.shortcuts.require_portal {
                anyhow::bail!("require_portal=true and portal unavailable: {e}");
            }
            tracing::info!("GlobalShortcuts portal unavailable: {e} — socket-only");
            return Ok(());
        }
    };

    let session = portal.create_session(Default::default()).await?;
    let shortcuts: Vec<NewShortcut> = cfg.modes.iter().map(|(id, m)| {
        let desc = m.description.clone().unwrap_or_else(|| format!("Paraphrase: {id}"));
        let mut s = NewShortcut::new(id.clone(), desc);
        s = s.preferred_trigger(m.shortcut.as_deref());
        s
    }).collect();

    if shortcuts.is_empty() {
        tracing::warn!("no modes configured; nothing to bind");
        return Ok(());
    }

    portal
        .bind_shortcuts(&session, &shortcuts, None, Default::default())
        .await?;

    tracing::info!(count = shortcuts.len(), "portal shortcuts bound");

    let mut activations = portal.receive_activated().await?;
    while let Some(act) = activations.next().await {
        // ashpd's Activated exposes `shortcut_id()`. Newer revisions may
        // name this differently — adapt if compilation fails.
        let id = act.shortcut_id().to_owned();
        let d = dispatcher.clone();
        tokio::spawn(async move { d.handle_activation(&id).await });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{llm::EchoLlm, wayland::mock::MockWayland};
    use smarty_pants_core::config::{Config, ModeCfg};

    #[tokio::test]
    async fn dispatcher_invokes_pipeline_with_id_as_mode() {
        let wl = Arc::new(MockWayland::new());
        wl.set_primary(Some("hello"));
        let mut cfg = Config::default();
        cfg.modes.insert("rewrite".into(), ModeCfg {
            system: "rw".into(),
            shortcut: None, description: None,
            temperature: None, top_p: None, max_tokens: None,
        });
        let pipe = Arc::new(Pipeline::new(wl.clone(), Arc::new(EchoLlm), Arc::new(cfg)));
        let d = Dispatcher::new(pipe);
        d.handle_activation("rewrite").await;
        let v = wl.read(crate::wayland::ClipboardKind::Regular).await.unwrap();
        assert_eq!(v.as_deref(), Some("[paraphrased] hello"));
    }
}
```

Add to `crates/daemon/src/main.rs`:

```rust
mod shortcuts;
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p smarty-pants-daemon --lib shortcuts::`
Expected: 1 passed.

- [ ] **Step 3: Commit**

```bash
git add crates/daemon/
git commit -m "daemon: GlobalShortcuts portal session + dispatcher"
```

---

## Task 16: `daemon::main` — wiring + signal handling + tracing

**Files:**
- Modify: `crates/daemon/src/main.rs` (full rewrite)

- [ ] **Step 1: Write the new main**

```rust
// crates/daemon/src/main.rs
mod wayland;
mod selection;
mod inject;
mod prompt;
mod llm;
mod model_download;
mod pipeline;
mod server;
mod shortcuts;

use anyhow::Context;
use llm::{Llm, LlamaLlm};
use llama_cpp_2::llama_backend::LlamaBackend;
use pipeline::Pipeline;
use server::Server;
use shortcuts::{Dispatcher, run_session};
use smarty_pants_core::{config::Config, paths};
use std::path::PathBuf;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let config_path = paths::expand("$XDG_CONFIG_HOME/smarty-pants/config.toml");
    let cfg = if config_path.exists() {
        Config::from_path(&config_path).context("load config")?
    } else {
        tracing::info!("no config at {} — using defaults", config_path.display());
        let mut cfg = Config::default();
        // Phase 1: inject the single hardcoded rewrite mode if user has none
        if cfg.modes.is_empty() {
            cfg.modes.insert("rewrite".into(),
                smarty_pants_core::config::ModeCfg {
                    system: include_str!("../../../examples/rewrite_prompt.txt").to_owned(),
                    shortcut: Some("SUPER+SHIFT+P".to_owned()),
                    description: Some("Paraphrase: rewrite in different words".to_owned()),
                    temperature: None, top_p: None, max_tokens: None,
                });
        }
        cfg
    };
    let cfg = Arc::new(cfg);

    preflight_tools()?;

    let data_dir = paths::expand("$XDG_DATA_HOME/smarty-pants/models");
    tokio::fs::create_dir_all(&data_dir).await?;
    let model_path = model_download::ensure_model(
        &model_download::GEMMA_3_1B_IT_Q4_K_M, &data_dir,
    ).await.context("ensure model")?;

    // ── load LLM ──
    let backend = Arc::new(LlamaBackend::init().context("llama backend init")?);
    let llm: Arc<dyn Llm> = Arc::new(LlamaLlm::load(
        backend,
        &model_path,
        cfg.model.context_size,
        cfg.model.threads,
        cfg.model.gpu_layers,
        cfg.model.gpu_main_device,
    ).context("load LLM")?);

    // ── Wayland + pipeline ──
    let wl = Arc::new(wayland::real::RealWayland::new());
    let pipeline = Arc::new(Pipeline::new(wl, llm, cfg.clone()));

    // ── socket server ──
    let socket_path = paths::expand(&cfg.daemon.socket_path);
    let server = Server::bind(&socket_path, pipeline.clone()).context("bind socket")?;
    let server_task = tokio::spawn(server.serve());

    // ── portal shortcuts (best-effort) ──
    let dispatcher = Arc::new(Dispatcher::new(pipeline.clone()));
    let cfg_for_shortcuts = cfg.clone();
    let shortcuts_task = tokio::spawn(async move {
        if let Err(e) = run_session(&cfg_for_shortcuts, dispatcher).await {
            tracing::error!(error = %e, "shortcuts session ended in error");
        }
    });

    // ── shutdown on SIGTERM/SIGINT ──
    tokio::select! {
        _ = tokio::signal::ctrl_c() => tracing::info!("SIGINT received"),
        _ = wait_for_sigterm() => tracing::info!("SIGTERM received"),
        r = server_task => tracing::error!("server task ended: {r:?}"),
    }

    shortcuts_task.abort();
    let _ = std::fs::remove_file(&socket_path);
    Ok(())
}

fn init_tracing() {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,smarty_pants=info"));
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_target(true))
        .init();
}

async fn wait_for_sigterm() {
    use tokio::signal::unix::{signal, SignalKind};
    if let Ok(mut s) = signal(SignalKind::terminate()) {
        s.recv().await;
    } else {
        // can't listen; block forever
        std::future::pending::<()>().await;
    }
}

fn preflight_tools() -> anyhow::Result<()> {
    for tool in ["wtype", "wl-copy", "wl-paste"] {
        which::which(tool).map_err(|_| anyhow::anyhow!(
            "required tool missing: `{tool}`. Install `wtype` and `wl-clipboard` first."
        ))?;
    }
    Ok(())
}
```

Add to `crates/daemon/Cargo.toml`:

```toml
[dependencies]
# … existing …
which = "6"
```

Create `examples/rewrite_prompt.txt`:

```
You are a paraphrasing assistant. Rewrite the user's text so it has
the same meaning but different wording and sentence structure. Rules:
- Preserve meaning exactly. Do not add, remove, or invent facts.
- Reply in the SAME LANGUAGE as the input.
- Output ONLY the paraphrased text. No preamble, no quotes, no notes.
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build -p smarty-pants-daemon`
Expected: builds. (Will fail at runtime until the SHA-256 placeholder is replaced — Task 11 step 3.)

- [ ] **Step 3: Commit**

```bash
git add crates/daemon/src/main.rs crates/daemon/Cargo.toml examples/rewrite_prompt.txt
git commit -m "daemon: main wiring (config, model load, server, portal, signals)"
```

---

## Task 17: `cli::main` + clap router

**Files:**
- Modify: `crates/cli/src/main.rs` (full rewrite)

- [ ] **Step 1: Write the router**

```rust
// crates/cli/src/main.rs
mod trigger;
mod status;
mod daemon_cmd;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version, about = "smarty-pants — local AI paraphraser for Wayland")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Paraphrase the current text selection using a configured mode.
    Trigger {
        #[arg(long, default_value = "rewrite")]
        mode: String,
    },
    /// Show daemon health.
    Status,
    /// Manage the daemon process.
    Daemon {
        #[command(subcommand)]
        sub: DaemonSub,
    },
}

#[derive(Subcommand)]
enum DaemonSub {
    /// Start the daemon as a detached child process.
    Start,
    /// Send shutdown to the running daemon via the socket.
    Stop,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Trigger { mode } => trigger::run(&mode).await,
        Cmd::Status            => status::run().await,
        Cmd::Daemon { sub: DaemonSub::Start } => daemon_cmd::start().await,
        Cmd::Daemon { sub: DaemonSub::Stop  } => daemon_cmd::stop().await,
    }
}
```

- [ ] **Step 2: Build (will fail until Tasks 18–20 create the submodules)**

Run: `cargo check -p smarty-pants-cli`
Expected: `error[E0583]: file not found for module `trigger`` — that's fine, next tasks fill these in.

- [ ] **Step 3: No commit yet** — wait until Tasks 18-20 finish so cli compiles.

---

## Task 18: `cli::trigger`

**Files:**
- Create: `crates/cli/src/trigger.rs`

- [ ] **Step 1: Write the module**

```rust
// crates/cli/src/trigger.rs
use smarty_pants_core::{paths, protocol::{Request, Response}};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

pub async fn run(mode: &str) -> anyhow::Result<()> {
    let socket = paths::expand("$XDG_RUNTIME_DIR/smarty-pants.sock");
    let mut stream = UnixStream::connect(&socket).await.map_err(|e| {
        anyhow::anyhow!(
            "cannot connect to {}: {e}. Is the daemon running? Try `smarty-pants daemon start`.",
            socket.display()
        )
    })?;
    let req = Request::Paraphrase { mode: mode.to_owned() };
    let body = serde_json::to_string(&req)?;
    stream.write_all(body.as_bytes()).await?;
    stream.write_all(b"\n").await?;

    let mut buf = String::new();
    stream.read_to_string(&mut buf).await?;
    let resp: Response = serde_json::from_str(buf.trim())?;
    match resp {
        Response::Ok { generated_chars, ms } => {
            eprintln!("ok — paraphrased {generated_chars} chars in {ms} ms");
            Ok(())
        }
        Response::Empty        => { eprintln!("no text selected"); std::process::exit(3) }
        Response::Busy         => { eprintln!("daemon busy"); std::process::exit(4) }
        Response::ModelLoading => { eprintln!("model loading"); std::process::exit(5) }
        Response::Status { .. } => unreachable!("trigger sent paraphrase, got Status"),
        Response::Error { kind, message } => {
            eprintln!("error ({kind:?}): {message}");
            std::process::exit(1)
        }
    }
}
```

- [ ] **Step 2: Build**

Run: `cargo check -p smarty-pants-cli`
Expected: still missing `status` and `daemon_cmd` — next tasks.

- [ ] **Step 3: No commit yet** — see Task 20.

---

## Task 19: `cli::status`

**Files:**
- Create: `crates/cli/src/status.rs`

- [ ] **Step 1: Write the module**

```rust
// crates/cli/src/status.rs
use smarty_pants_core::{paths, protocol::{Request, Response}};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

pub async fn run() -> anyhow::Result<()> {
    let socket = paths::expand("$XDG_RUNTIME_DIR/smarty-pants.sock");
    match UnixStream::connect(&socket).await {
        Err(_) => {
            println!("daemon: not running");
            std::process::exit(2);
        }
        Ok(mut stream) => {
            let body = serde_json::to_string(&Request::Status)?;
            stream.write_all(body.as_bytes()).await?;
            stream.write_all(b"\n").await?;
            let mut buf = String::new();
            stream.read_to_string(&mut buf).await?;
            let resp: Response = serde_json::from_str(buf.trim())?;
            if let Response::Status { healthy, model_loaded, mode_count } = resp {
                println!("daemon: running ({}, model={}, modes={mode_count})",
                    if healthy { "healthy" } else { "unhealthy" },
                    if model_loaded { "loaded" } else { "loading" });
            } else {
                println!("daemon: unexpected response {resp:?}");
                std::process::exit(2);
            }
            Ok(())
        }
    }
}
```

- [ ] **Step 2: No commit yet** — see Task 20.

---

## Task 20: `cli::daemon_cmd` — start/stop

**Files:**
- Create: `crates/cli/src/daemon_cmd.rs`

- [ ] **Step 1: Write the module**

```rust
// crates/cli/src/daemon_cmd.rs
use smarty_pants_core::paths;
use tokio::process::Command;

pub async fn start() -> anyhow::Result<()> {
    // Spawn the daemon binary detached. Look for it next to ourselves first
    // (cargo install layout), then fall back to PATH.
    let bin = locate_daemon_binary()?;
    let mut cmd = Command::new(&bin);
    cmd.kill_on_drop(false);
    // Detach: redirect std{in,out,err}
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());
    let child = cmd.spawn()
        .map_err(|e| anyhow::anyhow!("spawn {}: {e}", bin.display()))?;
    println!("daemon started (pid {})", child.id().unwrap_or(0));
    Ok(())
}

pub async fn stop() -> anyhow::Result<()> {
    use smarty_pants_core::protocol::Request;
    use tokio::io::AsyncWriteExt;
    use tokio::net::UnixStream;
    let socket = paths::expand("$XDG_RUNTIME_DIR/smarty-pants.sock");
    let Ok(mut stream) = UnixStream::connect(&socket).await else {
        println!("daemon: not running");
        return Ok(());
    };
    let body = serde_json::to_string(&Request::Shutdown)?;
    stream.write_all(body.as_bytes()).await?;
    stream.write_all(b"\n").await?;
    println!("daemon stop requested");
    Ok(())
}

fn locate_daemon_binary() -> anyhow::Result<std::path::PathBuf> {
    if let Ok(self_path) = std::env::current_exe() {
        if let Some(dir) = self_path.parent() {
            let candidate = dir.join("smarty-pants-daemon");
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }
    which::which("smarty-pants-daemon")
        .map_err(|_| anyhow::anyhow!("smarty-pants-daemon not found on PATH"))
}
```

Add to `crates/cli/Cargo.toml`:

```toml
[dependencies]
# … existing …
which = "6"
```

NOTE on shutdown: the daemon's `Request::Shutdown` currently returns `Ok` but does not actually exit the process — wiring shutdown into `main` is a Phase 2 follow-up (or fix here if it's quick). For Phase 1, `stop` issuing a Shutdown is enough to surface the request to the daemon; in practice users `pkill smarty-pants-daemon` or rely on the systemd unit (Phase 2).

- [ ] **Step 2: Build the full CLI**

Run: `cargo build -p smarty-pants-cli`
Expected: builds clean.

- [ ] **Step 3: Commit (the whole CLI in one commit)**

```bash
git add crates/cli/
git commit -m "cli: clap router + trigger/status/daemon subcommands"
```

---

## Task 21: workspace-level E2E test (daemon + CLI on a temp socket)

**Files:**
- Create: `tests/e2e_socket_path.rs`
- Modify: `Cargo.toml` (declare a workspace test target — see step 1)

This test uses the EchoLlm stub via a small in-tree test binary that wires the daemon with a mock Wayland + EchoLlm + a temp socket. Cleanest way: expose a `daemon::testing::run_with_stubs` helper from the daemon crate, then call it from the test.

- [ ] **Step 1: Add a `pub` testing entry-point in the daemon library**

Convert `crates/daemon` to also export a library by adding to `crates/daemon/Cargo.toml`:

```toml
[lib]
name = "smarty_pants_daemon"
path = "src/lib.rs"
```

Create `crates/daemon/src/lib.rs`:

```rust
//! Library facade so workspace-level tests can spin up the daemon with stubs.

pub mod wayland;
pub mod selection;
pub mod inject;
pub mod prompt;
pub mod llm;
pub mod pipeline;
pub mod server;
pub mod shortcuts;

pub mod testing {
    use crate::{llm::EchoLlm, pipeline::Pipeline, server::Server, wayland::mock::MockWayland};
    use smarty_pants_core::config::{Config, ModeCfg};
    use std::path::Path;
    use std::sync::Arc;

    /// Spawn a daemon with EchoLlm + MockWayland on the given socket.
    /// Returns the abort handle and a clone of the mock Wayland for the
    /// test to drive.
    pub async fn run_with_stubs(socket: &Path, primary_text: &str) -> (tokio::task::JoinHandle<()>, Arc<MockWayland>) {
        let wl = Arc::new(MockWayland::new());
        wl.set_primary(Some(primary_text));
        let mut cfg = Config::default();
        cfg.modes.insert("rewrite".into(), ModeCfg {
            system: "rewrite".into(),
            shortcut: None, description: None,
            temperature: None, top_p: None, max_tokens: None,
        });
        let pipe = Arc::new(Pipeline::new(wl.clone(), Arc::new(EchoLlm), Arc::new(cfg)));
        let server = Server::bind(socket, pipe).expect("bind");
        let handle = tokio::spawn(async move { let _ = server.serve().await; });
        (handle, wl)
    }
}
```

Modify `crates/daemon/src/main.rs`: remove the duplicated `mod` declarations and use the library crate's modules instead. `model_download.rs` stays binary-only because it carries the hardcoded Phase 1 model constants — moving it into the lib would couple every consumer to those constants.

```rust
// crates/daemon/src/main.rs — replace the top portion
mod model_download;

use anyhow::Context;
use llama_cpp_2::llama_backend::LlamaBackend;
use smarty_pants_core::{config::Config, paths};
use smarty_pants_daemon::{
    llm::{Llm, LlamaLlm},
    pipeline::Pipeline,
    server::Server,
    shortcuts::{run_session, Dispatcher},
    wayland,
};
use std::sync::Arc;

// … rest of main() body unchanged …
```

The rest of `main.rs` body — `init_tracing`, `wait_for_sigterm`, `preflight_tools`, and the `#[tokio::main] async fn main()` itself — stays exactly as written in Task 16.

- [ ] **Step 2: Write the E2E test**

```rust
// tests/e2e_socket_path.rs
use smarty_pants_core::protocol::{Request, Response};
use smarty_pants_daemon::testing::run_with_stubs;
use std::time::Duration;
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

#[tokio::test]
async fn happy_path_via_socket_uses_echo_llm_and_writes_clipboard() {
    let tmp = TempDir::new().unwrap();
    let sock = tmp.path().join("sp.sock");

    let (server_handle, wl) = run_with_stubs(&sock, "Hello, friend.").await;
    tokio::time::sleep(Duration::from_millis(30)).await;

    let mut client = UnixStream::connect(&sock).await.unwrap();
    let req = serde_json::to_string(&Request::Paraphrase { mode: "rewrite".into() }).unwrap();
    client.write_all(req.as_bytes()).await.unwrap();
    client.write_all(b"\n").await.unwrap();
    let mut buf = String::new();
    client.read_to_string(&mut buf).await.unwrap();

    let resp: Response = serde_json::from_str(buf.trim()).unwrap();
    assert!(matches!(resp, Response::Ok { .. }));

    // Mock Wayland should have the EchoLlm output on the regular clipboard
    let v = wl.read(smarty_pants_daemon::wayland::ClipboardKind::Regular).await.unwrap();
    assert_eq!(v.as_deref(), Some("[paraphrased] Hello, friend."));

    server_handle.abort();
}
```

Add to `crates/daemon/Cargo.toml`:

```toml
[dev-dependencies]
# … existing insta, tempfile …
smarty-pants-core = { path = "../core" }
```

The test lives in the workspace root `tests/` dir and is run by the daemon's package by adding to `crates/daemon/Cargo.toml`:

```toml
[[test]]
name = "e2e_socket_path"
path = "../../tests/e2e_socket_path.rs"
```

- [ ] **Step 3: Run it**

Run: `cargo test -p smarty-pants-daemon --test e2e_socket_path`
Expected: 1 passed.

- [ ] **Step 4: Commit**

```bash
git add tests/ crates/daemon/
git commit -m "tests: workspace E2E socket happy-path with EchoLlm + MockWayland"
```

---

## Task 22: README + build-deps docs + example config

**Files:**
- Modify: `README.md` (full rewrite)
- Create: `examples/config.toml`
- Create: `docs/INSTALL.md` (optional but recommended)

- [ ] **Step 1: Rewrite README**

```markdown
# smarty-pants

Local AI paraphraser for Wayland. Highlight text in any app, press a hotkey, get it rewritten in place — no cloud, no telemetry.

## Status

Phase 1 (MVP). Supports Hyprland (XDG GlobalShortcuts portal) and niri / Sway (compositor-config hotkey + CLI trigger). Single `rewrite` mode. Gemma 3 1B Q4 model auto-downloaded on first run.

## Build

You need:

- Rust ≥ 1.75 (stable)
- `cmake`, `clang`, `pkg-config`
- Vulkan loader + headers: `vulkan-loader`, `vulkan-headers`, `glslang`
- `wtype` and `wl-clipboard` at runtime

On openSUSE Tumbleweed:

    sudo zypper install cmake clang pkg-config vulkan-headers libvulkan1 glslang-devel wtype wl-clipboard

Then:

    cargo install --path crates/cli --locked
    cargo install --path crates/daemon --locked

## Configure

Optional. Defaults work for the `rewrite` mode. To customize, copy `examples/config.toml` to `~/.config/smarty-pants/config.toml`.

## Run

    smarty-pants daemon start

On first start the daemon downloads the Gemma 3 1B Q4 model (~800 MB) to `~/.local/share/smarty-pants/models/` and verifies SHA-256.

### Hyprland

The daemon registers a portal shortcut named `rewrite` at startup. Open Hyprland's shortcut settings UI and bind it to whatever key combo you like (suggested default: `SUPER+SHIFT+P`).

### niri / Sway

Add to your compositor config:

```kdl
# ~/.config/niri/config.kdl
binds { Super+Shift+P { spawn "smarty-pants" "trigger" "--mode" "rewrite"; } }
```

```
# ~/.config/sway/config
bindsym $mod+Shift+p exec smarty-pants trigger --mode rewrite
```

Reload your compositor. Highlight text, press the hotkey.

## Status check

    smarty-pants status

## Stop

    smarty-pants daemon stop
    # or kill -TERM $(pgrep smarty-pants-daemon)

## License

Apache-2.0.
```

- [ ] **Step 2: Write `examples/config.toml`**

```toml
# Copy to ~/.config/smarty-pants/config.toml and edit.
# Every section is optional; defaults match Phase 1 behavior.

[daemon]
log_level = "info"

[shortcuts]
enabled        = true
require_portal = false

[model]
gpu_layers = -1     # -1 = offload all if a Vulkan GPU is present, 0 = CPU

[modes.rewrite]
shortcut    = "SUPER+SHIFT+P"
description = "Paraphrase: rewrite in different words"
system      = """
You are a paraphrasing assistant. Rewrite the user's text so it has
the same meaning but different wording and sentence structure. Rules:
- Preserve meaning exactly. Do not add, remove, or invent facts.
- Reply in the SAME LANGUAGE as the input.
- Output ONLY the paraphrased text. No preamble, no quotes, no notes.
"""
```

- [ ] **Step 3: Verify everything still builds**

Run: `cargo build --workspace`
Expected: clean build.

- [ ] **Step 4: Run the full test suite**

Run: `cargo test --workspace`
Expected: all unit + integration tests pass.

- [ ] **Step 5: Commit**

```bash
git add README.md examples/
git commit -m "docs: README, install instructions, example config"
```

---

## Phase 1 acceptance checklist

After Task 22, verify the acceptance criteria from spec Section 9:

- [ ] Fresh checkout, `cargo install --path crates/cli && cargo install --path crates/daemon` succeeds on a clean openSUSE Tumbleweed (or your distro of choice).
- [ ] On Hyprland: `smarty-pants daemon start`, open shortcut settings, assign a key combo to the `rewrite` shortcut, highlight a sentence, press the combo, sentence is rewritten in place within ~3 s on CPU / ≤1 s on Vulkan GPU (after warm).
- [ ] On niri or Sway: same setup but with a `binds`/`bindsym` line + `smarty-pants trigger --mode rewrite`. Same latency targets.
- [ ] 50 invocations in a row do not crash the daemon.
- [ ] `cargo test --workspace` is green.
- [ ] `cargo clippy --workspace -- -D warnings` is green.
- [ ] `cargo fmt --all --check` is green.
