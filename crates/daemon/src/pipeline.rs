//! Glue capture → generate → inject under a single-flight mutex.

use crate::{
    inject, llm::{GenerationParams, Llm}, prompt::{self, Template}, selection, wayland::Wayland,
};
use smarty_pants_core::{config::Config, protocol::{ErrorKind, Response}};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;



/// Single-line preview of a string for log output. Replaces newlines with
/// spaces and caps length so logs stay readable.
fn preview(s: &str) -> String {
    const MAX: usize = 200;
    let one_line: String = s.chars().map(|c| if c == '\n' { ' ' } else { c }).collect();
    if one_line.chars().count() <= MAX {
        one_line
    } else {
        let head: String = one_line.chars().take(MAX).collect();
        format!("{head}…")
    }
}

pub struct Pipeline {
    wl:       Arc<dyn Wayland>,
    llm:      Arc<dyn Llm>,
    cfg:      Arc<Config>,
    template: Template,
    inflight: Mutex<()>,
}

impl Pipeline {
    pub fn new(
        wl:       Arc<dyn Wayland>,
        llm:      Arc<dyn Llm>,
        cfg:      Arc<Config>,
        template: Template,
    ) -> Self {
        Self { wl, llm, cfg, template, inflight: Mutex::new(()) }
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
            Err((kind, msg)) => Response::Error { error_kind: kind, message: msg },
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
        tracing::info!(
            chars = captured.text.chars().count(),
            preview = %preview(&captured.text),
            "captured selection"
        );

        let prompt = prompt::render(self.template, &mode.system, &captured.text);
        let params = GenerationParams {
            max_tokens:  mode.max_tokens.unwrap_or(self.cfg.model.max_tokens),
            temperature: mode.temperature.unwrap_or(self.cfg.model.temperature),
            top_p:       mode.top_p.unwrap_or(self.cfg.model.top_p),
            seed:        self.cfg.model.seed,
        };
        let generated = self.llm.generate(&prompt, &params).await
            .map_err(|e| (ErrorKind::Inference, e.to_string()))?;
        tracing::info!(
            chars = generated.chars().count(),
            preview = %preview(&generated),
            "llm generated"
        );

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
        let pipe = Pipeline::new(wl.clone(), Arc::new(EchoLlm), cfg_with_mode(), Template::Gemma);

        let resp = pipe.run("rewrite").await;
        match resp {
            Response::Ok { generated_chars, .. } => assert!(generated_chars > 0),
            other => panic!("expected Ok, got {other:?}"),
        }
        // EchoLlm output should now be on the regular clipboard.
        let v = wl.read(crate::wayland::ClipboardKind::Regular).await.unwrap();
        assert_eq!(v.as_deref(), Some("[paraphrased] Hello world."));
    }

    #[tokio::test]
    async fn empty_selection_returns_empty() {
        let wl = Arc::new(MockWayland::new());
        let pipe = Pipeline::new(wl, Arc::new(EchoLlm), cfg_with_mode(), Template::Gemma);
        assert!(matches!(pipe.run("rewrite").await, Response::Empty));
    }

    #[tokio::test]
    async fn unknown_mode_returns_internal_error() {
        let wl = Arc::new(MockWayland::new());
        wl.set_primary(Some("x"));
        let pipe = Pipeline::new(wl, Arc::new(EchoLlm), cfg_with_mode(), Template::Gemma);
        let resp = pipe.run("nope").await;
        assert!(matches!(resp, Response::Error { error_kind: ErrorKind::Internal, .. }));
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
        let pipe = Arc::new(Pipeline::new(wl, Arc::new(SlowLlm), cfg_with_mode(), Template::Gemma));
        let a = tokio::spawn({ let p = pipe.clone(); async move { p.run("rewrite").await } });
        tokio::time::sleep(Duration::from_millis(5)).await;
        let b = pipe.run("rewrite").await;
        let _ = a.await.unwrap();
        assert!(matches!(b, Response::Busy));
    }
}
