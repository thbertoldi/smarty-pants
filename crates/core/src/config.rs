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
    pub socket_path:   String,
    pub log_level:     String,
    pub busy_response: String,
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
    pub gpu_layers:      i32,  // -1 = all layers (if GPU present), 0 = CPU only, >0 = explicit count
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

/// A paraphrase mode definition. The `system` prompt is required; all other
/// fields are optional and (when present) override the `[model]` defaults
/// for this mode. `ModeCfg` intentionally lacks struct-level
/// `#[serde(default)]` — modes must be fully specified, not partially
/// inherited, so unknown modes are loud errors instead of silent empties.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModeCfg {
    pub system:      String,
    #[serde(default)]
    pub shortcut:    Option<String>,
    #[serde(default)]
    pub description: Option<String>,
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
        toml::from_str(&raw)
            .map_err(|e| anyhow::anyhow!("parse {}: {e}", path.display()))
    }
}
