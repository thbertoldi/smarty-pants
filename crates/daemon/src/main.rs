//! Daemon lifecycle: settings, IPC, shortcuts and tray start before model load.

use anyhow::Context;
use smarty_pants_core::{config::Config, paths};
use smarty_pants_daemon::{
    backend,
    pipeline::Pipeline,
    server::Server,
    settings::Settings,
    shortcuts::{run_session, Dispatcher},
    wayland,
};
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config_path = paths::expand("$XDG_CONFIG_HOME/smarty-pants/config.toml");
    let cfg = Arc::new(Config::load(&config_path).context("load config")?);
    init_tracing(&cfg.daemon.log_level);
    preflight_tools()?;

    let llm = backend::create(&cfg)?;
    let wl = Arc::new(wayland::real::RealWayland::new());
    let pipeline = Arc::new(Pipeline::new(wl, llm, cfg.clone()));
    let settings = Arc::new(Settings::new(pipeline.clone(), config_path).await);
    let socket_path = paths::expand(&cfg.daemon.socket_path);
    let server = Server::bind(&socket_path, pipeline.clone())?.with_settings(settings.clone());
    let shutdown = server.shutdown_token();
    let mut server_task = tokio::spawn(server.serve());
    let mut shortcuts_task = start_shortcuts(cfg.clone(), pipeline.clone());
    #[cfg(feature = "tray")]
    let tray_task = if cfg.tray.enabled {
        let settings = settings.clone();
        let shutdown = shutdown.clone();
        Some(tokio::spawn(async move {
            if let Err(error) = smarty_pants_daemon::tray::run(settings, shutdown).await {
                tracing::warn!(%error, "tray unavailable; CLI and shortcuts remain available");
            }
        }))
    } else {
        None
    };

    tracing::info!(provider = cfg.inference.provider.as_str(), "daemon ready");
    let mut changes = settings.subscribe();
    let mut current = cfg;
    let mut idle_tick = tokio::time::interval(std::time::Duration::from_secs(15));
    let result = loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => break Ok(()),
            _ = wait_for_sigterm() => break Ok(()),
            result = &mut server_task => break result.context("socket server task").and_then(|result| result),
            _ = shutdown.cancelled() => break Ok(()),
            _ = idle_tick.tick() => pipeline.unload_if_idle().await,
            result = changes.changed() => {
                if result.is_err() { break Ok(()); }
                let updated = changes.borrow_and_update().clone();
                if updated.shortcuts != current.shortcuts || shortcut_definitions(&updated) != shortcut_definitions(&current) {
                    shortcuts_task.abort();
                    shortcuts_task = start_shortcuts(updated.clone(), pipeline.clone());
                }
                current = updated;
            }
        }
    };
    shutdown.cancel();
    shortcuts_task.abort();
    server_task.abort();
    #[cfg(feature = "tray")]
    if let Some(task) = tray_task {
        let _ = task.await;
    }
    let _ = std::fs::remove_file(&socket_path);
    result
}

fn start_shortcuts(cfg: Arc<Config>, pipeline: Arc<Pipeline>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        if let Err(error) = run_session(&cfg, Arc::new(Dispatcher::new(pipeline))).await {
            tracing::error!(%error, "shortcuts session ended in error");
        }
    })
}

fn shortcut_definitions(cfg: &Config) -> Vec<(&str, Option<&str>, Option<&str>)> {
    cfg.modes
        .iter()
        .map(|(name, mode)| {
            (
                name.as_str(),
                mode.shortcut.as_deref(),
                mode.description.as_deref(),
            )
        })
        .collect()
}

fn init_tracing(level: &str) {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));
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
        // Can't listen for some reason; block forever and let SIGINT win.
        std::future::pending::<()>().await;
    }
}

fn preflight_tools() -> anyhow::Result<()> {
    for tool in ["wtype", "wl-copy", "wl-paste"] {
        which::which(tool).map_err(|_| {
            anyhow::anyhow!(
                "required tool missing: `{tool}`. Install `wtype` and `wl-clipboard` first."
            )
        })?;
    }
    Ok(())
}
