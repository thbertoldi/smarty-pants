//! Construct only the selected provider. Local weights are loaded on demand.

use crate::{api_llm::ApiLlm, llm::Llm};
use smarty_pants_core::config::{Config, Provider};
use std::sync::Arc;

pub fn create(cfg: &Config) -> anyhow::Result<Arc<dyn Llm>> {
    cfg.validate()?;
    match cfg.inference.provider {
        Provider::Local => {
            #[cfg(feature = "local")]
            {
                Ok(Arc::new(local::LazyLocal::new(cfg.model.clone())?))
            }
            #[cfg(not(feature = "local"))]
            {
                anyhow::bail!("this build has no local inference; configure an API provider or rebuild with --features local")
            }
        }
        provider => Ok(Arc::new(ApiLlm::new(
            cfg.api_config().expect("API provider"),
            provider,
        )?)),
    }
}

#[cfg(feature = "local")]
mod local {
    use crate::{
        llm::{GenerationParams, Llm},
        local_llm::LlamaLlm,
        model_download,
        prompt::Prompt,
    };
    use async_trait::async_trait;
    use llama_cpp_2::llama_backend::LlamaBackend;
    use smarty_pants_core::{config::ModelCfg, paths};
    use std::{
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
        time::{Duration, Instant},
    };
    use tokio::sync::{Mutex, OnceCell};

    // llama.cpp permits one backend initialization per process. Keep the small
    // backend handle; the model weights themselves are released when idle.
    static BACKEND: std::sync::Mutex<Option<Arc<LlamaBackend>>> = std::sync::Mutex::new(None);

    fn backend() -> anyhow::Result<Arc<LlamaBackend>> {
        let mut backend = BACKEND
            .lock()
            .map_err(|_| anyhow::anyhow!("backend lock poisoned"))?;
        if backend.is_none() {
            *backend = Some(Arc::new(LlamaBackend::init()?));
        }
        Ok(backend.as_ref().unwrap().clone())
    }

    struct Loaded {
        llm: LlamaLlm,
        last_used: Instant,
    }

    pub struct LazyLocal {
        cfg: ModelCfg,
        loaded: Mutex<Option<Loaded>>,
        resident: AtomicBool,
        verified_path: OnceCell<std::path::PathBuf>,
    }

    impl LazyLocal {
        pub fn new(cfg: ModelCfg) -> anyhow::Result<Self> {
            if let Some(path) = &cfg.path {
                anyhow::ensure!(
                    paths::expand(path).is_file(),
                    "model.path is not an existing GGUF file"
                );
            } else {
                model_download::find(&cfg.name)?;
            }
            Ok(Self {
                cfg,
                loaded: Mutex::new(None),
                resident: AtomicBool::new(false),
                verified_path: OnceCell::new(),
            })
        }

        async fn load(&self) -> anyhow::Result<LlamaLlm> {
            // Verify a preset once per configured backend, rather than hashing
            // gigabytes again after every idle unload.
            let path = self
                .verified_path
                .get_or_try_init(|| async {
                    if let Some(path) = &self.cfg.path {
                        Ok(paths::expand(path))
                    } else {
                        let spec = model_download::find(&self.cfg.name)?;
                        let dir = paths::expand("$XDG_DATA_HOME/smarty-pants/models");
                        model_download::ensure_model(spec, &dir).await
                    }
                })
                .await?
                .clone();
            let template = if self.cfg.path.is_some() {
                self.cfg.chat_template
            } else {
                model_download::find(&self.cfg.name)?.chat_template
            };
            let cfg = self.cfg.clone();
            tokio::task::spawn_blocking(move || {
                LlamaLlm::load(
                    backend()?,
                    &path,
                    cfg.context_size,
                    cfg.threads,
                    cfg.gpu_layers,
                    cfg.gpu_main_device,
                    template,
                )
            })
            .await?
        }
    }

    #[async_trait]
    impl Llm for LazyLocal {
        async fn generate(
            &self,
            prompt: &Prompt,
            params: &GenerationParams,
        ) -> anyhow::Result<String> {
            let mut slot = self.loaded.lock().await;
            if slot.is_none() {
                *slot = Some(Loaded {
                    llm: self.load().await?,
                    last_used: Instant::now(),
                });
                self.resident.store(true, Ordering::Release);
            }
            let loaded = slot.as_mut().unwrap();
            let result = loaded.llm.generate(prompt, params).await;
            loaded.last_used = Instant::now();
            result
        }

        fn is_loaded(&self) -> bool {
            self.resident.load(Ordering::Acquire)
        }

        async fn unload(&self) {
            let mut slot = self.loaded.lock().await;
            let old = slot.take();
            self.resident.store(false, Ordering::Release);
            // GPU/model destructors can be expensive; keep them off the reactor.
            let _ = tokio::task::spawn_blocking(move || drop(old)).await;
        }

        async fn unload_if_idle(&self) {
            if self.cfg.idle_unload_seconds == 0 {
                return;
            }
            let Ok(mut slot) = self.loaded.try_lock() else {
                return;
            };
            if slot.as_ref().is_some_and(|l| {
                l.last_used.elapsed() >= Duration::from_secs(self.cfg.idle_unload_seconds)
            }) {
                let old = slot.take();
                self.resident.store(false, Ordering::Release);
                let _ = tokio::task::spawn_blocking(move || drop(old)).await;
                tracing::info!("unloaded idle local model");
            }
        }
    }
}
