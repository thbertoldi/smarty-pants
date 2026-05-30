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

/// Owns the portal session for the lifetime of the daemon.
///
/// Returns `Ok(())` after gracefully handling the no-portal case (logs and
/// exits without error). Returns `Err(_)` only when `cfg.shortcuts.require_portal`
/// is true and the portal is unavailable.
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
        NewShortcut::new(id.clone(), desc)
            .preferred_trigger(m.shortcut.as_deref())
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
        // ashpd's Activated exposes a `shortcut_id()` accessor. If the actual
        // method name in your ashpd version differs (e.g., `.id()` or a public
        // field), adapt the call here and report the adaptation.
        let id = act.shortcut_id().to_owned();
        let d = dispatcher.clone();
        tokio::spawn(async move { d.handle_activation(&id).await });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{llm::EchoLlm, wayland::{mock::MockWayland, Wayland}};
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
