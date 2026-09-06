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
fn applies_defaults_when_minimal_toml() {
    let cfg: Config = toml::from_str("").expect("parse defaults");
    assert_eq!(cfg.daemon.log_level, "info");
    assert_eq!(cfg.model.context_size, 4096);
    assert_eq!(cfg.capture.ctrl_c_settle_ms, 40);
}

#[test]
fn deepseek_partial_overrides_keep_provider_defaults() {
    let cfg: Config = toml::from_str(
        "[inference]\nprovider = 'deepseek'\n[deepseek]\nmodel = 'deepseek-v4-pro'\n",
    )
    .unwrap();
    cfg.validate().unwrap();
    assert_eq!(cfg.deepseek.base_url, "https://api.deepseek.com");
    assert_eq!(
        cfg.deepseek.api_key_env.as_deref(),
        Some("DEEPSEEK_API_KEY")
    );
    let empty: Config = toml::from_str("").unwrap();
    assert_eq!(empty, Config::default());
}

#[test]
fn validates_remote_transport_and_sampling_before_applying_config() {
    for invalid in [
        "[inference]\nprovider = 'typo'",
        "[model]\ntemperature = nan",
        "[model]\nmax_tokens = 0",
        "[modes.custom]\nsystem = ''",
        "[inference]\nprovider = 'openai_compatible'\n[api]\nbase_url = 'http://example.com/v1'\nmodel = 'editor'",
        "[inference]\nprovider = 'deepseek'\n[deepseek]\nbase_url = 'https://secret@example.com/v1'",
        "[inference]\nprovider = 'deepseek'\n[deepseek]\ntimeout_seconds = 0",
    ] {
        let result = toml::from_str::<Config>(invalid).map_err(anyhow::Error::from).and_then(|cfg| cfg.validate());
        assert!(result.is_err(), "accepted {invalid}");
    }
    let cfg: Config = toml::from_str("[inference]\nprovider = 'openai_compatible'\n[api]\nbase_url = 'http://[::1]:8080/v1'\nmodel = 'editor'").unwrap();
    cfg.validate().unwrap();
}

#[test]
fn shipped_example_loads_and_retains_all_builtin_modes() {
    let cfg = Config::load(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/config.toml"),
    )
    .unwrap();
    assert_eq!(cfg.modes.len(), 4);
    assert!(cfg.modes.contains_key("condense"));
}

#[test]
fn parse_error_includes_file_path() {
    use std::io::Write;
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("bad.toml");
    let mut f = std::fs::File::create(&p).unwrap();
    f.write_all(b"[daemon]\nlog_level = 123\n").unwrap(); // wrong type
    let err = Config::from_path(&p).unwrap_err();
    let msg = format!("{err:#}");
    assert!(
        msg.contains("bad.toml"),
        "expected file path in error, got: {msg}"
    );
}
