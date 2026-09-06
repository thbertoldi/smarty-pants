//! Native StatusNotifierItem/DBusMenu tray, without a GTK runtime in the daemon.

use crate::{model_download::PRESETS, settings::Settings};
use ksni::{menu::*, TrayMethods};
use smarty_pants_core::{
    config::{Config, Provider},
    config_file,
    protocol::{Response, SettingChange},
};
use std::sync::Arc;
use tokio::{process::Command, sync::mpsc};
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
enum Action {
    Change(SettingChange),
    Reload,
    OpenConfig,
    ApiConnection(Option<Provider>),
    ApiKey,
    Unload,
    Quit,
}

struct WritingTray {
    cfg: Arc<Config>,
    summary: String,
    error: Option<String>,
    working: bool,
    sender: mpsc::Sender<Action>,
}

impl WritingTray {
    fn send(&mut self, action: Action) {
        if self.sender.try_send(action).is_err() {
            self.error = Some("Another settings action is pending".into());
        }
    }

    fn item(&self, label: &str, action: Action, enabled: bool) -> MenuItem<Self> {
        StandardItem {
            label: label.into(),
            enabled: enabled && !self.working,
            activate: Box::new(move |tray: &mut Self| tray.send(action.clone())),
            ..Default::default()
        }
        .into()
    }

    fn check(
        &self,
        label: &str,
        checked: bool,
        action: SettingChange,
        enabled: bool,
    ) -> MenuItem<Self> {
        CheckmarkItem {
            label: label.into(),
            checked,
            enabled: enabled && !self.working,
            activate: Box::new(move |tray: &mut Self| tray.send(Action::Change(action.clone()))),
            ..Default::default()
        }
        .into()
    }
}

impl ksni::Tray for WritingTray {
    fn id(&self) -> String {
        "computer.smarty-pants".into()
    }
    fn title(&self) -> String {
        format!("smarty-pants — {}", self.summary)
    }
    fn icon_name(&self) -> String {
        "accessories-text-editor".into()
    }
    fn status(&self) -> ksni::Status {
        if self.error.is_some() {
            ksni::Status::NeedsAttention
        } else {
            ksni::Status::Active
        }
    }
    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            title: "smarty-pants".into(),
            description: self.summary.clone(),
            ..Default::default()
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let cfg = &self.cfg;
        let local = cfg.inference.provider == Provider::Local;
        let providers = [
            Provider::Local,
            Provider::Deepseek,
            Provider::OpenaiCompatible,
        ];
        let provider_menu = RadioGroup {
            selected: providers
                .iter()
                .position(|p| *p == cfg.inference.provider)
                .unwrap_or(0),
            options: [
                "Local (on this device)",
                "DeepSeek (cloud)",
                "Compatible API (configured server)",
            ]
            .iter()
            .enumerate()
            .map(|(i, label)| RadioItem {
                label: (*label).into(),
                enabled: !self.working && (i != 0 || cfg!(feature = "local")),
                ..Default::default()
            })
            .collect(),
            select: Box::new(move |tray: &mut Self, index| {
                if let Some(provider) = providers.get(index) {
                    if *provider == Provider::OpenaiCompatible
                        && tray.cfg.api.model.trim().is_empty()
                    {
                        tray.send(Action::ApiConnection(Some(*provider)));
                    } else {
                        tray.send(Action::Change(SettingChange::Provider {
                            provider: *provider,
                        }));
                    }
                }
            }),
        };
        let mut models: Vec<MenuItem<Self>> = PRESETS
            .iter()
            .map(|spec| {
                self.check(
                    spec.label,
                    cfg.model.path.is_none() && cfg.model.name == spec.key,
                    SettingChange::LocalModel {
                        name: spec.key.into(),
                    },
                    local && cfg!(feature = "local"),
                )
            })
            .collect();
        if cfg.model.path.is_some() {
            models.push(
                StandardItem {
                    label: "Custom GGUF (from config)".into(),
                    enabled: false,
                    ..Default::default()
                }
                .into(),
            );
        }
        let mut menu = vec![
            StandardItem {
                label: self.summary.replace('_', "__"),
                enabled: false,
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            self.check(
                "Paused",
                cfg.daemon.paused,
                SettingChange::Paused {
                    paused: !cfg.daemon.paused,
                },
                true,
            ),
            SubMenu {
                label: "Provider".into(),
                submenu: vec![provider_menu.into()],
                ..Default::default()
            }
            .into(),
            SubMenu {
                label: "Local model (download size)".into(),
                enabled: local,
                submenu: models,
                ..Default::default()
            }
            .into(),
            self.item(
                "API connection…",
                Action::ApiConnection(if local {
                    None
                } else {
                    Some(cfg.inference.provider)
                }),
                true,
            ),
            self.item("Set API key…", Action::ApiKey, !local),
            SubMenu {
                label: "Resource use".into(),
                enabled: local,
                submenu: vec![
                    self.check(
                        "Use GPU when available",
                        cfg.model.gpu_layers != 0,
                        SettingChange::Gpu {
                            enabled: cfg.model.gpu_layers == 0,
                        },
                        local,
                    ),
                    self.check(
                        "Keep model loaded",
                        cfg.model.idle_unload_seconds == 0,
                        SettingChange::KeepLoaded {
                            enabled: cfg.model.idle_unload_seconds != 0,
                        },
                        local,
                    ),
                    self.item("Unload model now", Action::Unload, local),
                ],
                ..Default::default()
            }
            .into(),
            self.check(
                "Restore previous clipboard",
                cfg.inject.restore_clipboard,
                SettingChange::RestoreClipboard {
                    enabled: !cfg.inject.restore_clipboard,
                },
                true,
            ),
            MenuItem::Separator,
            self.item("Edit configuration…", Action::OpenConfig, true),
            self.item("Reload configuration", Action::Reload, true),
        ];
        if let Some(error) = &self.error {
            menu.push(
                StandardItem {
                    label: format!("Error: {}", error.replace('_', "__")),
                    enabled: false,
                    ..Default::default()
                }
                .into(),
            );
        }
        menu.push(MenuItem::Separator);
        menu.push(
            StandardItem {
                label: "Quit smarty-pants".into(),
                activate: Box::new(|tray: &mut Self| tray.send(Action::Quit)),
                ..Default::default()
            }
            .into(),
        );
        menu
    }
}

pub async fn run(settings: Arc<Settings>, shutdown: CancellationToken) -> anyhow::Result<()> {
    let (sender, mut actions) = mpsc::channel(8);
    let tray = WritingTray {
        cfg: settings.pipeline.config().await,
        summary: "Ready".into(),
        error: None,
        working: false,
        sender,
    };
    // Register even when the panel starts after the daemon (e.g. Waybar login).
    let handle = tray.assume_sni_available(true).spawn().await?;
    let mut changes = settings.subscribe();
    let mut tick = tokio::time::interval(std::time::Duration::from_secs(2));
    let mut jobs = tokio::task::JoinSet::new();
    let mut action_error = None;
    loop {
        tokio::select! {
            _ = shutdown.cancelled() => break,
            Some(action) = actions.recv() => {
                if matches!(action, Action::Quit) { shutdown.cancel(); break; }
                if jobs.is_empty() {
                    let settings = settings.clone();
                    jobs.spawn(async move { perform(action, settings).await });
                    action_error = None;
                }
            }
            Some(result) = jobs.join_next(), if !jobs.is_empty() => {
                action_error = match result {
                    Ok(Ok(())) => None,
                    Ok(Err(e)) => Some(e.to_string()),
                    Err(_) => Some("Settings action failed".into()),
                };
                if let Some(error) = &action_error { tracing::warn!(%error, "tray action failed"); }
            }
            _ = tick.tick() => {}
            result = changes.changed() => { if result.is_err() { break; } }
        }
        let cfg = settings.pipeline.config().await;
        let (summary, generation_error) = summary(settings.pipeline.status().await);
        let error = action_error.clone().or(generation_error);
        if handle
            .update(|tray| {
                tray.cfg = cfg;
                tray.summary = summary;
                tray.error = error;
                tray.working = !jobs.is_empty();
            })
            .await
            .is_none()
        {
            break;
        }
    }
    jobs.abort_all();
    handle.shutdown().await;
    Ok(())
}

fn summary(status: Response) -> (String, Option<String>) {
    let Response::Status {
        provider,
        model,
        model_loaded,
        paused,
        busy,
        last_error,
        ..
    } = status
    else {
        return ("Status unavailable".into(), None);
    };
    let state = if paused {
        "Paused"
    } else if busy && !model_loaded && provider == "local" {
        "Loading local model…"
    } else if busy {
        "Rewriting…"
    } else if provider != "local" {
        "API configured"
    } else if model_loaded {
        "Ready"
    } else {
        "Ready (loads on first rewrite)"
    };
    (format!("{state} · {provider} · {model}"), last_error)
}

async fn perform(action: Action, settings: Arc<Settings>) -> anyhow::Result<()> {
    match action {
        Action::Change(change) => settings.change(change).await,
        Action::Reload => settings.reload().await,
        Action::Unload => settings.pipeline.unload().await,
        Action::OpenConfig => {
            settings.ensure_file().await?;
            let status = Command::new("xdg-open")
                .arg(settings.path())
                .status()
                .await?;
            anyhow::ensure!(
                status.success(),
                "could not open config file; edit it with your preferred text editor"
            );
            Ok(())
        }
        Action::ApiConnection(provider) => edit_api_connection(settings, provider).await,
        Action::ApiKey => set_api_key(settings).await,
        Action::Quit => Ok(()),
    }
}

async fn dialog(args: &[&str]) -> anyhow::Result<Option<String>> {
    which::which("zenity").map_err(|_| {
        anyhow::anyhow!("Install zenity for settings dialogs, or use Edit configuration")
    })?;
    let output = Command::new("zenity")
        .args(args)
        .kill_on_drop(true)
        .output()
        .await?;
    if output.status.code() == Some(1) {
        return Ok(None);
    } // Cancel is not an error.
    anyhow::ensure!(output.status.success(), "could not open settings dialog");
    Ok(Some(
        String::from_utf8(output.stdout)?
            .trim_end_matches('\n')
            .to_owned(),
    ))
}

async fn edit_api_connection(
    settings: Arc<Settings>,
    provider: Option<Provider>,
) -> anyhow::Result<()> {
    let cfg = settings.pipeline.config().await;
    let provider = match provider {
        Some(provider) => provider,
        None => {
            let Some(choice) = dialog(&[
                "--list",
                "--radiolist",
                "--title=smarty-pants API provider",
                "--column=Select",
                "--column=Provider",
                "TRUE",
                "DeepSeek",
                "FALSE",
                "Compatible API",
            ])
            .await?
            else {
                return Ok(());
            };
            if choice == "DeepSeek" {
                Provider::Deepseek
            } else {
                Provider::OpenaiCompatible
            }
        }
    };
    let api = match provider {
        Provider::Deepseek => (&cfg.deepseek).into(),
        Provider::OpenaiCompatible => cfg.api.clone(),
        Provider::Local => anyhow::bail!("select an API provider first"),
    };
    let text = format!("Leave fields blank to keep their current values.\n\nBase URL: {}\nModel: {}\nKey environment variable: {}\n\nSaving selects this provider. Selected text is sent to it when you rewrite. A saved key file takes precedence over an environment key.",
        api.base_url, api.model, api.api_key_env.as_deref().unwrap_or("none"));
    let Some(values) = dialog(&[
        "--forms",
        "--title=smarty-pants API connection",
        "--no-markup",
        "--text",
        &text,
        "--separator=\t",
        "--add-entry=Base URL",
        "--add-entry=Model",
        "--add-entry=API key environment variable",
    ])
    .await?
    else {
        return Ok(());
    };
    let parts: Vec<_> = values.split('\t').map(str::trim).collect();
    anyhow::ensure!(parts.len() == 3, "invalid form values");
    settings
        .change(SettingChange::ApiConnection {
            provider,
            base_url: if parts[0].is_empty() {
                api.base_url
            } else {
                parts[0].into()
            },
            model: if parts[1].is_empty() {
                api.model
            } else {
                parts[1].into()
            },
            api_key_env: if parts[2].is_empty() {
                api.api_key_env
            } else {
                Some(parts[2].into())
            },
            activate: true,
        })
        .await
}

async fn set_api_key(settings: Arc<Settings>) -> anyhow::Result<()> {
    let provider = settings.pipeline.config().await.inference.provider;
    anyhow::ensure!(provider != Provider::Local, "select an API provider first");
    let Some(key) = dialog(&[
        "--password",
        "--title=smarty-pants API key",
        "--text=API key (saved in a private file on this device)",
    ])
    .await?
    else {
        return Ok(());
    };
    anyhow::ensure!(!key.trim().is_empty(), "API key cannot be empty");
    let dir = settings.path().parent().unwrap().join("keys");
    // A fresh file makes the config pointer switch atomic. Failed validation
    // cannot overwrite the currently active credential.
    let path = config_file::new_private_file(&dir, key.trim().as_bytes())?;
    if let Err(error) = settings
        .change(SettingChange::ApiKeyFile {
            provider,
            path: path.to_string_lossy().into_owned(),
        })
        .await
    {
        let _ = std::fs::remove_file(path);
        return Err(error);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ksni::Tray;

    #[tokio::test]
    async fn menu_actions_request_changes_without_optimistic_state() {
        let (sender, mut actions) = mpsc::channel(8);
        let mut tray = WritingTray {
            cfg: Arc::new(Config::default()),
            summary: "Ready".into(),
            error: None,
            working: false,
            sender,
        };
        let mut menu = tray.menu();
        let MenuItem::Checkmark(pause) = &mut menu[2] else {
            panic!("expected pause item");
        };
        (pause.activate)(&mut tray);
        assert!(matches!(
            actions.recv().await,
            Some(Action::Change(SettingChange::Paused { paused: true }))
        ));
        assert!(!tray.cfg.daemon.paused);
    }
}
