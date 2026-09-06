//! Transactional settings changes shared by tray and Unix-socket clients.

use crate::{backend, pipeline::Pipeline};
use smarty_pants_core::{
    config::{Config, Provider},
    config_file::ConfigFile,
    protocol::SettingChange,
};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::sync::watch;

pub struct Settings {
    pub pipeline: Arc<Pipeline>,
    path: PathBuf,
    changed: watch::Sender<Arc<Config>>,
}

impl Settings {
    pub async fn new(pipeline: Arc<Pipeline>, path: PathBuf) -> Self {
        let (changed, _) = watch::channel(pipeline.config().await);
        Self {
            pipeline,
            path,
            changed,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn subscribe(&self) -> watch::Receiver<Arc<Config>> {
        self.changed.subscribe()
    }

    pub async fn reload(&self) -> anyhow::Result<()> {
        self.apply(None).await
    }
    pub async fn change(&self, change: SettingChange) -> anyhow::Result<()> {
        self.apply(Some(change)).await
    }

    async fn apply(&self, change: Option<SettingChange>) -> anyhow::Result<()> {
        let _guard = self.pipeline.lock_settings()?;
        let mut file = ConfigFile::load(&self.path)?;
        if let Some(change) = &change {
            match change {
                SettingChange::Provider { provider } => {
                    file.set("inference", "provider", provider.as_str())
                }
                SettingChange::LocalModel { name } => {
                    crate::model_download::find(name)?;
                    file.set("model", "name", name.as_str());
                    file.remove("model", "path");
                }
                SettingChange::Gpu { enabled } => {
                    file.set("model", "gpu_layers", if *enabled { -1 } else { 0 })
                }
                SettingChange::KeepLoaded { enabled } => file.set(
                    "model",
                    "idle_unload_seconds",
                    if *enabled { 0 } else { 300 },
                ),
                SettingChange::RestoreClipboard { enabled } => {
                    file.set("inject", "restore_clipboard", *enabled)
                }
                SettingChange::Delivery { delivery } => {
                    file.set("inject", "delivery", delivery.as_str())
                }
                SettingChange::ApiModel { provider, model } => {
                    file.set(api_section(*provider)?, "model", model.trim())
                }
                SettingChange::Paused { paused } => file.set("daemon", "paused", *paused),
                SettingChange::ApiConnection {
                    provider,
                    base_url,
                    model,
                    api_key_env,
                    activate,
                } => {
                    let section = api_section(*provider)?;
                    file.set(section, "base_url", base_url.as_str());
                    file.set(section, "model", model.as_str());
                    if let Some(name) = api_key_env {
                        file.set(section, "api_key_env", name.as_str());
                    } else {
                        file.remove(section, "api_key_env");
                    }
                    if *activate {
                        file.set("inference", "provider", provider.as_str());
                    }
                }
                SettingChange::ApiKeyFile { provider, path } => {
                    file.set(api_section(*provider)?, "api_key_file", path.as_str())
                }
            }
        }
        let cfg = file.config()?;
        if let Some(
            SettingChange::ApiConnection { provider, .. }
            | SettingChange::ApiModel { provider, .. },
        ) = &change
        {
            let mut validation = cfg.clone();
            validation.inference.provider = *provider;
            validation.validate()?;
        }
        let old = self.pipeline.config().await;
        anyhow::ensure!(
            cfg.daemon.socket_path == old.daemon.socket_path,
            "socket_path changes require a daemon restart"
        );
        anyhow::ensure!(
            cfg.daemon.log_level == old.daemon.log_level,
            "log_level changes require a daemon restart"
        );
        anyhow::ensure!(
            cfg.tray == old.tray,
            "tray.enabled changes require a daemon restart"
        );
        let provider_changed = cfg.inference != old.inference;
        let backend_changed = provider_changed
            || match cfg.inference.provider {
                Provider::Local => cfg.model != old.model,
                _ => cfg.api_config() != old.api_config(),
            };
        let llm = if backend_changed {
            Some(backend::create(&cfg)?)
        } else {
            None
        };
        // Only after all validation succeeds do we atomically replace the file.
        if change.is_some() {
            file.save(&self.path)?;
        }
        let cfg = Arc::new(cfg);
        self.pipeline.replace(cfg.clone(), llm).await;
        self.changed.send_replace(cfg);
        Ok(())
    }

    pub async fn ensure_file(&self) -> anyhow::Result<()> {
        smarty_pants_core::config_file::initialize(
            &self.path,
            self.pipeline.config().await.as_ref(),
        )
    }
}

fn api_section(provider: Provider) -> anyhow::Result<&'static str> {
    match provider {
        Provider::Deepseek => Ok("deepseek"),
        Provider::OpenaiCompatible => Ok("api"),
        Provider::Local => anyhow::bail!("select an API provider first"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{llm::EchoLlm, wayland::mock::MockWayland};

    #[tokio::test]
    async fn changing_api_model_preserves_connection_and_key_and_rejects_empty_id() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "[inference]\nprovider='deepseek'\n[deepseek]\nbase_url='https://api.deepseek.com'\napi_key_file='/tmp/test.key'\n# keep this comment\n").unwrap();
        let pipeline = Arc::new(Pipeline::new(
            Arc::new(MockWayland::new()),
            Arc::new(EchoLlm),
            Arc::new(Config::load(&path).unwrap()),
        ));
        let settings = Settings::new(pipeline.clone(), path.clone()).await;
        settings
            .change(SettingChange::ApiModel {
                provider: Provider::Deepseek,
                model: "deepseek-v4-pro".into(),
            })
            .await
            .unwrap();
        let cfg = pipeline.config().await;
        assert_eq!(cfg.deepseek.model, "deepseek-v4-pro");
        assert_eq!(cfg.deepseek.base_url, "https://api.deepseek.com");
        assert_eq!(cfg.deepseek.api_key_file.as_deref(), Some("/tmp/test.key"));
        let saved = std::fs::read_to_string(&path).unwrap();
        assert!(saved.contains("# keep this comment"));
        assert!(settings
            .change(SettingChange::ApiModel {
                provider: Provider::Deepseek,
                model: "  ".into()
            })
            .await
            .is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), saved);
        assert_eq!(pipeline.config().await.deepseek.model, "deepseek-v4-pro");
    }

    #[tokio::test]
    async fn invalid_reload_keeps_working_configuration() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut cfg = Config::default();
        cfg.add_builtin_modes();
        let pipeline = Arc::new(Pipeline::new(
            Arc::new(MockWayland::new()),
            Arc::new(EchoLlm),
            Arc::new(cfg),
        ));
        let settings = Settings::new(pipeline.clone(), path.clone()).await;
        std::fs::write(&path, "[model]\nname = 'does-not-exist'\n").unwrap();
        assert!(settings.reload().await.is_err());
        assert_eq!(
            pipeline.config().await.model.name,
            Config::default().model.name
        );
    }

    #[tokio::test]
    async fn pause_is_persisted_and_takes_effect_without_replacing_backend() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let pipeline = Arc::new(Pipeline::new(
            Arc::new(MockWayland::new()),
            Arc::new(EchoLlm),
            Arc::new(Config::default()),
        ));
        let settings = Settings::new(pipeline.clone(), path.clone()).await;
        settings
            .change(SettingChange::Paused { paused: true })
            .await
            .unwrap();
        assert!(Config::load(&path).unwrap().daemon.paused);
        assert_eq!(
            pipeline.run("rewrite").await,
            smarty_pants_core::protocol::Response::Paused
        );
    }
}
