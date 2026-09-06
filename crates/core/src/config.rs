//! TOML configuration shared by the daemon, tray and CLI.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Default)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub daemon: DaemonCfg,
    pub shortcuts: ShortcutsCfg,
    pub model: ModelCfg,
    pub inference: InferenceCfg,
    pub deepseek: DeepseekCfg,
    pub api: ApiCfg,
    pub tray: TrayCfg,
    pub writing: WritingCfg,
    pub capture: CaptureCfg,
    pub inject: InjectCfg,
    pub modes: BTreeMap<String, ModeCfg>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct DaemonCfg {
    pub socket_path: String,
    pub log_level: String,
    pub busy_response: String,
    pub paused: bool,
}
impl Default for DaemonCfg {
    fn default() -> Self {
        Self {
            socket_path: "$XDG_RUNTIME_DIR/smarty-pants.sock".into(),
            log_level: "info".into(),
            busy_response: "reject".into(),
            paused: false,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct ShortcutsCfg {
    pub enabled: bool,
    pub require_portal: bool,
    pub app_id: String,
}
impl Default for ShortcutsCfg {
    fn default() -> Self {
        Self {
            enabled: true,
            require_portal: false,
            app_id: "computer.smarty-pants".into(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct ModelCfg {
    pub name: String,
    /// An existing GGUF file. When set, no preset is downloaded.
    pub path: Option<String>,
    pub chat_template: ChatTemplate,
    /// Zero keeps the local model resident indefinitely.
    pub idle_unload_seconds: u64,
    pub context_size: u32,
    pub threads: u32,
    pub gpu_layers: i32, // -1 = all layers (if GPU present), 0 = CPU only, >0 = explicit count
    pub gpu_main_device: u32,
    pub seed: u32,
    pub max_tokens: u32,
    pub temperature: f32,
    pub top_p: f32,
}
impl Default for ModelCfg {
    fn default() -> Self {
        Self {
            name: "qwen-2.5-1.5b-instruct-q4_k_m".into(),
            path: None,
            chat_template: ChatTemplate::ChatML,
            idle_unload_seconds: 300,
            context_size: 4096,
            threads: 0,
            gpu_layers: -1,
            gpu_main_device: 0,
            seed: 0,
            max_tokens: 512,
            temperature: 0.2,
            top_p: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    #[default]
    Local,
    Deepseek,
    OpenaiCompatible,
}

impl Provider {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Deepseek => "deepseek",
            Self::OpenaiCompatible => "openai_compatible",
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct InferenceCfg {
    pub provider: Provider,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
pub enum ChatTemplate {
    #[serde(rename = "gemma")]
    Gemma,
    #[default]
    #[serde(rename = "chatml")]
    ChatML,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct ApiCfg {
    pub base_url: String,
    pub model: String,
    /// Credentials stay outside the TOML file and are never included in status.
    pub api_key_env: Option<String>,
    pub api_key_file: Option<String>,
    pub timeout_seconds: u64,
}

impl Default for ApiCfg {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:8080/v1".into(),
            model: String::new(),
            api_key_env: None,
            api_key_file: None,
            timeout_seconds: 60,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct DeepseekCfg {
    pub base_url: String,
    pub model: String,
    pub api_key_env: Option<String>,
    pub api_key_file: Option<String>,
    pub timeout_seconds: u64,
}

impl Default for DeepseekCfg {
    fn default() -> Self {
        Self {
            base_url: "https://api.deepseek.com".into(),
            model: "deepseek-v4-flash".into(),
            api_key_env: Some("DEEPSEEK_API_KEY".into()),
            api_key_file: None,
            timeout_seconds: 60,
        }
    }
}

impl From<&DeepseekCfg> for ApiCfg {
    fn from(cfg: &DeepseekCfg) -> Self {
        Self {
            base_url: cfg.base_url.clone(),
            model: cfg.model.clone(),
            api_key_env: cfg.api_key_env.clone(),
            api_key_file: cfg.api_key_file.clone(),
            timeout_seconds: cfg.timeout_seconds,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct TrayCfg {
    pub enabled: bool,
}

impl Default for TrayCfg {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct WritingCfg {
    /// Reject changed/omitted numbers, URLs and code before replacing a selection.
    pub preserve_literals: bool,
}

impl Default for WritingCfg {
    fn default() -> Self {
        Self {
            preserve_literals: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct CaptureCfg {
    pub prefer_primary: bool,
    pub ctrl_c_settle_ms: u64,
    pub max_chars: usize,
}
impl Default for CaptureCfg {
    fn default() -> Self {
        Self {
            prefer_primary: true,
            ctrl_c_settle_ms: 40,
            max_chars: 8000,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct InjectCfg {
    pub delivery: Delivery,
    pub restore_clipboard: bool,
    pub paste_settle_ms: u64,
}
impl Default for InjectCfg {
    fn default() -> Self {
        // restore_clipboard: false by default because terminal-hosted TUIs
        // (Claude Code, helix, lazygit, ...) buffer the bracketed-paste
        // sequence and read the system clipboard with extra latency. If we
        // restore the prior clipboard 80ms after firing Ctrl+Shift+V, the
        // TUI sometimes reads the prior content instead of the paraphrase.
        // Leaving the paraphrase on the clipboard is also a reasonable UX:
        // it matches what a manual paste would have done.
        //
        // paste_settle_ms: 200ms is a generous-but-not-annoying window for
        // any focused app to consume the clipboard before we touch it again.
        Self {
            delivery: Delivery::Paste,
            restore_clipboard: false,
            paste_settle_ms: 200,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Delivery {
    #[default]
    Paste,
    Copy,
    Review,
}

impl Delivery {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Paste => "paste",
            Self::Copy => "copy",
            Self::Review => "review",
        }
    }
}

/// A paraphrase mode definition. The `system` prompt is required; all other
/// fields are optional and (when present) override the `[model]` defaults
/// for this mode. `ModeCfg` intentionally lacks struct-level
/// `#[serde(default)]` — modes must be fully specified, not partially
/// inherited, so unknown modes are loud errors instead of silent empties.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ModeCfg {
    pub system: String,
    #[serde(default)]
    pub shortcut: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub top_p: Option<f32>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
}

impl Config {
    pub fn from_path(path: &std::path::Path) -> anyhow::Result<Self> {
        let raw = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("read {}: {e}", path.display()))?;
        let cfg: Self =
            toml::from_str(&raw).map_err(|e| anyhow::anyhow!("parse {}: {e}", path.display()))?;
        cfg.validate()?;
        Ok(cfg)
    }

    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        let mut cfg = match std::fs::read_to_string(path) {
            Ok(raw) => toml::from_str::<Self>(&raw)
                .map_err(|e| anyhow::anyhow!("parse {}: {e}", path.display()))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(e) => return Err(e.into()),
        };
        cfg.add_builtin_modes();
        cfg.validate()?;
        Ok(cfg)
    }

    pub fn add_builtin_modes(&mut self) {
        for (name, shortcut, description, system) in [
            (
                "rewrite",
                "SUPER+R",
                "Improve: grammar and fluency",
                include_str!("../../../examples/prompts/rewrite.txt"),
            ),
            (
                "linkedin",
                "SUPER+SHIFT+L",
                "Improve: LinkedIn voice",
                include_str!("../../../examples/prompts/linkedin.txt"),
            ),
            (
                "academic",
                "SUPER+A",
                "Improve: academic voice",
                include_str!("../../../examples/prompts/academic.txt"),
            ),
            (
                "condense",
                "SUPER+K",
                "Improve: condense for fewer tokens",
                include_str!("../../../examples/prompts/condense.txt"),
            ),
        ] {
            self.modes.entry(name.into()).or_insert_with(|| ModeCfg {
                system: system.into(),
                shortcut: Some(shortcut.into()),
                description: Some(description.into()),
                temperature: None,
                top_p: None,
                max_tokens: None,
            });
        }
    }

    pub fn api_config(&self) -> Option<ApiCfg> {
        match self.inference.provider {
            Provider::Local => None,
            Provider::Deepseek => Some((&self.deepseek).into()),
            Provider::OpenaiCompatible => Some(self.api.clone()),
        }
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.model.context_size >= 512,
            "model.context_size must be at least 512"
        );
        anyhow::ensure!(
            self.model.gpu_layers >= -1,
            "model.gpu_layers must be -1, 0, or positive"
        );
        anyhow::ensure!(
            self.model.threads <= i32::MAX as u32,
            "model.threads is too large"
        );
        anyhow::ensure!(
            self.model.gpu_main_device <= i32::MAX as u32,
            "model.gpu_main_device is too large"
        );
        validate_sampling(
            self.model.temperature,
            self.model.top_p,
            self.model.max_tokens,
        )?;
        for (name, mode) in &self.modes {
            anyhow::ensure!(
                !mode.system.trim().is_empty(),
                "mode {name} has an empty system prompt"
            );
            validate_sampling(
                mode.temperature.unwrap_or(self.model.temperature),
                mode.top_p.unwrap_or(self.model.top_p),
                mode.max_tokens.unwrap_or(self.model.max_tokens),
            )?;
        }
        if let Some(api) = self.api_config() {
            anyhow::ensure!(!api.model.trim().is_empty(), "API model must be configured");
            anyhow::ensure!(
                (1..=600).contains(&api.timeout_seconds),
                "API timeout_seconds must be between 1 and 600"
            );
            let url = url::Url::parse(&api.base_url)
                .map_err(|_| anyhow::anyhow!("invalid API base_url"))?;
            anyhow::ensure!(
                url.host_str().is_some() && matches!(url.scheme(), "http" | "https"),
                "API base_url must be an HTTP(S) URL"
            );
            anyhow::ensure!(
                url.username().is_empty()
                    && url.password().is_none()
                    && url.query().is_none()
                    && url.fragment().is_none(),
                "API base_url must not contain credentials, a query, or a fragment"
            );
            let loopback = match url.host() {
                Some(url::Host::Domain(host)) => host == "localhost",
                Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
                Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
                None => false,
            };
            anyhow::ensure!(
                url.scheme() == "https" || loopback,
                "remote APIs require HTTPS; HTTP is supported for loopback servers"
            );
            if let Some(key) = &api.api_key_env {
                anyhow::ensure!(
                    !key.is_empty() && key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'),
                    "api_key_env must be an environment variable name"
                );
            }
        }
        Ok(())
    }
}

fn validate_sampling(temperature: f32, top_p: f32, max_tokens: u32) -> anyhow::Result<()> {
    anyhow::ensure!(
        temperature.is_finite() && (0.0..=2.0).contains(&temperature),
        "temperature must be between 0 and 2"
    );
    anyhow::ensure!(
        top_p.is_finite() && top_p > 0.0 && top_p <= 1.0,
        "top_p must be greater than 0 and at most 1"
    );
    anyhow::ensure!(max_tokens > 0, "max_tokens must be positive");
    Ok(())
}
