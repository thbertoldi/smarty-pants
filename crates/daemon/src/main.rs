// Daemon entry point. All real logic lives in the library crate
// (`smarty_pants_daemon::*`); this file wires it together, owns the
// long-lived async runtime, and handles signals.

use anyhow::Context;
use llama_cpp_2::llama_backend::LlamaBackend;
use smarty_pants_core::{config::Config, paths};
use smarty_pants_daemon::{
    llm::{Llm, LlamaLlm},
    model_download,
    pipeline::Pipeline,
    server::Server,
    shortcuts::{run_session, Dispatcher},
    wayland,
};
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    // ── config ──
    let config_path = paths::expand("$XDG_CONFIG_HOME/smarty-pants/config.toml");
    let mut cfg = if config_path.exists() {
        Config::from_path(&config_path).context("load config")?
    } else {
        tracing::info!(
            "no config at {} — using defaults",
            config_path.display()
        );
        Config::default()
    };

    // Inject the three built-in modes for any name the user's config
    // doesn't already define. Each mode registers as a separate portal
    // shortcut; the user binds them in hyprland.conf via the `global`
    // dispatcher (e.g. `bind = SUPER, R, global, surface-transient:rewrite`).
    type ModeDefaults = (&'static str, &'static str, &'static str, &'static str);
    const DEFAULT_MODES: &[ModeDefaults] = &[
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
    ];
    for (name, shortcut, description, system) in DEFAULT_MODES {
        if !cfg.modes.contains_key(*name) {
            cfg.modes.insert(
                (*name).into(),
                smarty_pants_core::config::ModeCfg {
                    system: (*system).to_owned(),
                    shortcut: Some((*shortcut).to_owned()),
                    description: Some((*description).to_owned()),
                    temperature: None,
                    top_p: None,
                    max_tokens: None,
                },
            );
        }
    }
    let cfg = Arc::new(cfg);

    // ── runtime tool preflight ──
    preflight_tools()?;

    // ── ensure model is on disk ──
    let data_dir = paths::expand("$XDG_DATA_HOME/smarty-pants/models");
    tokio::fs::create_dir_all(&data_dir).await?;
    let model_spec = &model_download::QWEN_2_5_7B_IT_Q4_K_M;
    let model_path = model_download::ensure_model(model_spec, &data_dir)
        .await
        .context("ensure model")?;

    // ── load LLM ──
    let backend = Arc::new(LlamaBackend::init().context("llama backend init")?);
    let llm: Arc<dyn Llm> = Arc::new(
        LlamaLlm::load(
            backend,
            &model_path,
            cfg.model.context_size,
            cfg.model.threads,
            cfg.model.gpu_layers,
            cfg.model.gpu_main_device,
        )
        .context("load LLM")?,
    );

    // ── Wayland + pipeline ──
    let wl = Arc::new(wayland::real::RealWayland::new());
    let pipeline = Arc::new(Pipeline::new(wl, llm, cfg.clone(), model_spec.chat_template));

    // ── Unix-socket server ──
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

    // ── wait for SIGTERM / SIGINT / server crash ──
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
