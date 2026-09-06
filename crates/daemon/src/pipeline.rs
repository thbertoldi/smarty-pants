//! Capture → generate → inject, with one operation at a time.

use crate::{
    inject, language,
    llm::{GenerationParams, Llm},
    prompt::Prompt,
    selection,
    wayland::Wayland,
};
use smarty_pants_core::{
    config::Config,
    protocol::{ErrorKind, Response},
};
use std::{sync::Arc, time::Instant};
use tokio::sync::{Mutex, MutexGuard, RwLock};

struct Runtime {
    cfg: Arc<Config>,
    llm: Arc<dyn Llm>,
}

pub struct Pipeline {
    wl: Arc<dyn Wayland>,
    runtime: RwLock<Runtime>,
    inflight: Mutex<()>,
    last_error: RwLock<Option<String>>,
}

impl Pipeline {
    pub fn new(wl: Arc<dyn Wayland>, llm: Arc<dyn Llm>, cfg: Arc<Config>) -> Self {
        Self {
            wl,
            runtime: RwLock::new(Runtime { cfg, llm }),
            inflight: Mutex::new(()),
            last_error: RwLock::new(None),
        }
    }

    pub async fn config(&self) -> Arc<Config> {
        self.runtime.read().await.cfg.clone()
    }

    /// Settings and unload operations share the same gate as rewrites.
    pub fn lock_settings(&self) -> anyhow::Result<MutexGuard<'_, ()>> {
        self.inflight
            .try_lock()
            .map_err(|_| anyhow::anyhow!("a rewrite is in progress; try again when it finishes"))
    }

    /// Caller holds lock_settings while validating/persisting the change.
    pub async fn replace(&self, cfg: Arc<Config>, llm: Option<Arc<dyn Llm>>) {
        let old = {
            let mut runtime = self.runtime.write().await;
            runtime.cfg = cfg;
            llm.map(|llm| std::mem::replace(&mut runtime.llm, llm))
        };
        if let Some(old) = old {
            old.unload().await;
        }
        *self.last_error.write().await = None;
    }

    pub async fn unload(&self) -> anyhow::Result<()> {
        let _guard = self.lock_settings()?;
        self.runtime.read().await.llm.clone().unload().await;
        Ok(())
    }

    pub async fn unload_if_idle(&self) {
        let Ok(_guard) = self.lock_settings() else {
            return;
        };
        self.runtime.read().await.llm.clone().unload_if_idle().await;
    }

    pub async fn status(&self) -> Response {
        let runtime = self.runtime.read().await;
        let cfg = &runtime.cfg;
        let model = cfg.api_config().map(|api| api.model).unwrap_or_else(|| {
            cfg.model
                .path
                .clone()
                .unwrap_or_else(|| cfg.model.name.clone())
        });
        Response::Status {
            healthy: true,
            model_loaded: runtime.llm.is_loaded(),
            mode_count: cfg.modes.len(),
            provider: cfg.inference.provider.as_str().into(),
            model,
            paused: cfg.daemon.paused,
            busy: self.inflight.try_lock().is_err(),
            last_error: self.last_error.read().await.clone(),
        }
    }

    pub async fn run(&self, mode_name: &str) -> Response {
        let _guard = match self.inflight.try_lock() {
            Ok(g) => g,
            Err(_) => return Response::Busy,
        };
        let (cfg, llm) = {
            let runtime = self.runtime.read().await;
            (runtime.cfg.clone(), runtime.llm.clone())
        };
        if cfg.daemon.paused {
            return Response::Paused;
        }
        let started = Instant::now();
        let result = self.run_inner(mode_name, &cfg, llm).await;
        *self.last_error.write().await = result.as_ref().err().map(|(_, message)| message.clone());
        match result {
            Ok(Some(chars)) => Response::Ok {
                generated_chars: chars,
                ms: started.elapsed().as_millis() as u64,
            },
            Ok(None) => Response::Empty,
            Err((kind, message)) => Response::Error {
                error_kind: kind,
                message,
            },
        }
    }

    async fn run_inner(
        &self,
        mode_name: &str,
        cfg: &Config,
        llm: Arc<dyn Llm>,
    ) -> Result<Option<usize>, (ErrorKind, String)> {
        let mode = cfg
            .modes
            .get(mode_name)
            .ok_or_else(|| (ErrorKind::Internal, format!("unknown mode: {mode_name}")))?;
        let captured = selection::capture(
            self.wl.clone(),
            cfg.capture.prefer_primary,
            cfg.capture.ctrl_c_settle_ms,
            cfg.capture.max_chars,
        )
        .await
        .map_err(|e| (ErrorKind::Capture, e.to_string()))?;
        let Some(captured) = captured else {
            return Ok(None);
        };
        // Never write selected text or generated text to the journal.
        tracing::info!(chars = captured.text.chars().count(), "captured selection");
        let prompt = Prompt::new(
            &mode.system,
            &captured.text,
            language::detect(&captured.text),
        );
        let params = GenerationParams {
            max_tokens: mode.max_tokens.unwrap_or(cfg.model.max_tokens),
            temperature: mode.temperature.unwrap_or(cfg.model.temperature),
            top_p: mode.top_p.unwrap_or(cfg.model.top_p),
            seed: cfg.model.seed,
        };
        let generated = llm
            .generate(&prompt, &params)
            .await
            .map_err(|e| (ErrorKind::Inference, e.to_string()))?;
        if generated.trim().is_empty() {
            return Err((
                ErrorKind::Inference,
                "model returned empty text; selection was not replaced".into(),
            ));
        }
        if cfg.writing.preserve_literals {
            crate::writing::validate(&captured.text, &generated)
                .map_err(|e| (ErrorKind::Inference, e.to_string()))?;
        }
        tracing::info!(chars = generated.chars().count(), "llm generated");
        inject::write(
            self.wl.clone(),
            &generated,
            cfg.inject.paste_settle_ms,
            cfg.inject.restore_clipboard,
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
        cfg.modes.insert(
            "rewrite".into(),
            ModeCfg {
                system: "Rewrite in different words. Same language.".into(),
                shortcut: None,
                description: None,
                temperature: None,
                top_p: None,
                max_tokens: None,
            },
        );
        Arc::new(cfg)
    }

    #[tokio::test]
    async fn failed_or_unfaithful_generation_never_pastes_or_changes_clipboard() {
        struct Answer(&'static str);
        #[async_trait::async_trait]
        impl Llm for Answer {
            async fn generate(&self, _: &Prompt, _: &GenerationParams) -> anyhow::Result<String> {
                if self.0 == "error" {
                    anyhow::bail!("API output was truncated");
                }
                Ok(self.0.into())
            }
        }
        for answer in ["error", "", "We tested samples.", "We tested 25 samples."] {
            let wl = Arc::new(MockWayland::new());
            wl.set_primary(Some("We tested 24 samples."));
            wl.set_regular(Some("original clipboard"));
            let pipe = Pipeline::new(wl.clone(), Arc::new(Answer(answer)), cfg_with_mode());
            assert!(matches!(
                pipe.run("rewrite").await,
                Response::Error {
                    error_kind: ErrorKind::Inference,
                    ..
                }
            ));
            assert!(wl.combos().is_empty());
            assert_eq!(
                wl.read(crate::wayland::ClipboardKind::Regular)
                    .await
                    .unwrap()
                    .as_deref(),
                Some("original clipboard")
            );
        }
    }

    #[tokio::test]
    async fn changing_backend_releases_previous_models_memory() {
        use std::sync::atomic::{AtomicBool, Ordering};
        struct Resident(Arc<AtomicBool>);
        #[async_trait::async_trait]
        impl Llm for Resident {
            async fn generate(&self, _: &Prompt, _: &GenerationParams) -> anyhow::Result<String> {
                unreachable!()
            }
            async fn unload(&self) {
                self.0.store(false, Ordering::SeqCst);
            }
        }
        let resident = Arc::new(AtomicBool::new(true));
        let pipe = Pipeline::new(
            Arc::new(MockWayland::new()),
            Arc::new(Resident(resident.clone())),
            cfg_with_mode(),
        );
        let _guard = pipe.lock_settings().unwrap();
        pipe.replace(cfg_with_mode(), Some(Arc::new(EchoLlm))).await;
        assert!(!resident.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn happy_path_returns_ok_and_pastes() {
        let wl = Arc::new(MockWayland::new());
        wl.set_primary(Some("Hello world."));
        let pipe = Pipeline::new(wl.clone(), Arc::new(EchoLlm), cfg_with_mode());

        let resp = pipe.run("rewrite").await;
        match resp {
            Response::Ok {
                generated_chars, ..
            } => assert!(generated_chars > 0),
            other => panic!("expected Ok, got {other:?}"),
        }
        // EchoLlm output should now be on the regular clipboard.
        let v = wl
            .read(crate::wayland::ClipboardKind::Regular)
            .await
            .unwrap();
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
        assert!(matches!(
            resp,
            Response::Error {
                error_kind: ErrorKind::Internal,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn concurrent_second_call_returns_busy() {
        use std::time::Duration;
        struct SlowLlm;
        #[async_trait::async_trait]
        impl Llm for SlowLlm {
            async fn generate(
                &self,
                _: &crate::prompt::Prompt,
                _: &GenerationParams,
            ) -> anyhow::Result<String> {
                tokio::time::sleep(Duration::from_millis(50)).await;
                Ok("slow".into())
            }
        }
        let wl = Arc::new(MockWayland::new());
        wl.set_primary(Some("x"));
        let pipe = Arc::new(Pipeline::new(wl, Arc::new(SlowLlm), cfg_with_mode()));
        let a = tokio::spawn({
            let p = pipe.clone();
            async move { p.run("rewrite").await }
        });
        tokio::time::sleep(Duration::from_millis(5)).await;
        let b = pipe.run("rewrite").await;
        let _ = a.await.unwrap();
        assert!(matches!(b, Response::Busy));
    }
}
