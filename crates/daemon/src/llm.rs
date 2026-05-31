//! Generation backend trait + a deterministic stub for tests.
//!
//! The real `LlamaLlm` backend (Task 12) lives in the same file behind the
//! `vulkan` (or `cuda`/`rocm`) feature; for Phase 1 unit tests we use
//! `EchoLlm` so we never need to load a model.

use async_trait::async_trait;

#[derive(Debug, Clone)]
pub struct GenerationParams {
    pub max_tokens:  u32,
    pub temperature: f32,
    pub top_p:       f32,
    pub seed:        u32,
}
impl Default for GenerationParams {
    fn default() -> Self {
        Self { max_tokens: 512, temperature: 0.7, top_p: 0.9, seed: 0 }
    }
}

#[async_trait]
pub trait Llm: Send + Sync + 'static {
    /// `prompt` is the fully chat-templated text. Implementations must
    /// return only the model's generated continuation, NOT echoing the prompt.
    async fn generate(&self, prompt: &str, params: &GenerationParams) -> anyhow::Result<String>;
}

// ── deterministic stub for tests ──────────────────────────────────────
pub struct EchoLlm;

#[async_trait]
impl Llm for EchoLlm {
    async fn generate(&self, prompt: &str, _: &GenerationParams) -> anyhow::Result<String> {
        // Extract the last user-turn content for a predictable transformation.
        // Tests only need a deterministic output, not a real paraphrase.
        let tail = prompt
            .rsplit("---\n\n")
            .next()
            .and_then(|s| s.strip_suffix("<end_of_turn>\n<start_of_turn>model\n"))
            .unwrap_or(prompt)
            .trim();
        Ok(format!("[paraphrased] {tail}"))
    }
}

// ── real llama-cpp-2 backed implementation ─────────────────────────────

use llama_cpp_2::{
    context::params::LlamaContextParams,
    llama_backend::LlamaBackend,
    llama_batch::LlamaBatch,
    model::{params::LlamaModelParams, AddBos, LlamaModel},
    sampling::LlamaSampler,
};
use std::num::NonZeroU32;
use std::path::Path;
use std::sync::Arc;

pub struct LlamaLlm {
    backend:   Arc<LlamaBackend>,
    model:     Arc<LlamaModel>,
    n_ctx:     u32,
    n_threads: i32,
}

impl LlamaLlm {
    pub fn load(
        backend:         Arc<LlamaBackend>,
        model_path:      &Path,
        n_ctx:           u32,
        n_threads:       u32,
        gpu_layers:      i32,
        gpu_main_device: u32,
    ) -> anyhow::Result<Self> {
        // Probe whether a GPU device is actually present. If the user
        // requested GPU offload (gpu_layers != 0) but none is detected,
        // warn and degrade to CPU rather than refusing to start.
        let gpu_available = gpu_devices_present();
        let effective_gpu_layers = match (gpu_layers, gpu_available) {
            (0, _) => {
                tracing::info!("gpu_layers = 0; using CPU only");
                0
            }
            (n, false) => {
                tracing::warn!(
                    requested = n,
                    "gpu_layers requested but no GPU device detected — falling back to CPU"
                );
                0
            }
            (n, true) => {
                let layers = if n < 0 { i32::MAX } else { n };
                tracing::info!(layers, gpu_main_device, "offloading to GPU");
                layers
            }
        };

        let mut params = LlamaModelParams::default();
        params = params.with_main_gpu(gpu_main_device as i32);
        // u32 cast: i32::MAX is fine; negative was already coerced above.
        params = params.with_n_gpu_layers(effective_gpu_layers as u32);

        let model = LlamaModel::load_from_file(&backend, model_path, &params)
            .map_err(|e| anyhow::anyhow!("load gguf: {e}"))?;
        tracing::info!(
            n_params    = model.n_params(),
            n_layer     = model.n_layer(),
            n_ctx_train = model.n_ctx_train(),
            "model loaded"
        );
        let n_threads = if n_threads == 0 {
            num_cpus_get_minus_one() as i32
        } else {
            n_threads as i32
        };
        Ok(Self { backend, model: Arc::new(model), n_ctx, n_threads })
    }
}

fn num_cpus_get_minus_one() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get().saturating_sub(1).max(1))
        .unwrap_or(1)
}

/// Heuristic GPU presence probe — `/dev/dri/renderD128` exists if any modern
/// Linux GPU driver loaded a render node (Intel/AMD/NVIDIA). Cheaper than
/// asking llama.cpp to enumerate devices before model load.
fn gpu_devices_present() -> bool {
    std::path::Path::new("/dev/dri/renderD128").exists()
}

#[async_trait]
impl Llm for LlamaLlm {
    async fn generate(&self, prompt: &str, params: &GenerationParams) -> anyhow::Result<String> {
        // llama-cpp-2 is sync; run on blocking pool.
        let backend   = self.backend.clone();
        let model     = self.model.clone();
        let n_ctx     = self.n_ctx;
        let n_threads = self.n_threads;
        let prompt    = prompt.to_owned();
        let params    = params.clone();

        tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
            let ctx_params = LlamaContextParams::default()
                .with_n_ctx(NonZeroU32::new(n_ctx))
                .with_n_threads(n_threads)
                .with_n_threads_batch(n_threads);
            let mut ctx = model.new_context(&backend, ctx_params)
                .map_err(|e| anyhow::anyhow!("new_context: {e}"))?;

            let tokens = model.str_to_token(&prompt, AddBos::Always)
                .map_err(|e| anyhow::anyhow!("tokenize: {e}"))?;
            let mut batch = LlamaBatch::new(tokens.len().max(512), 1);
            for (i, t) in tokens.iter().enumerate() {
                batch.add(*t, i as i32, &[0], i == tokens.len() - 1)
                    .map_err(|e| anyhow::anyhow!("batch: {e}"))?;
            }
            ctx.decode(&mut batch).map_err(|e| anyhow::anyhow!("decode prompt: {e}"))?;

            let mut sampler = LlamaSampler::chain_simple([
                LlamaSampler::temp(params.temperature),
                LlamaSampler::top_p(params.top_p, 1),
                LlamaSampler::dist(if params.seed == 0 {
                    rand_seed()
                } else {
                    params.seed
                }),
            ]);

            let mut out     = String::new();
            let mut n_cur   = batch.n_tokens();
            let mut decoder = encoding_rs::UTF_8.new_decoder();
            for _ in 0..params.max_tokens {
                let token = sampler.sample(&ctx, batch.n_tokens() - 1);
                if model.is_eog_token(token) { break; }
                sampler.accept(token);
                let piece = model
                    .token_to_piece(token, &mut decoder, true, None)
                    .map_err(|e| anyhow::anyhow!("detokenize: {e}"))?;
                out.push_str(&piece);
                batch.clear();
                batch.add(token, n_cur, &[0], true)
                    .map_err(|e| anyhow::anyhow!("batch add: {e}"))?;
                n_cur += 1;
                ctx.decode(&mut batch).map_err(|e| anyhow::anyhow!("decode: {e}"))?;
            }
            Ok(out.trim().to_owned())
        })
        .await
        .map_err(|e| anyhow::anyhow!("join: {e}"))?
    }
}

fn rand_seed() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn echo_stub_returns_predictable_string() {
        let llm = EchoLlm;
        let prompt = "<start_of_turn>user\nrewrite this.\n\n---\n\nhello world<end_of_turn>\n<start_of_turn>model\n";
        let out = llm.generate(prompt, &GenerationParams::default()).await.unwrap();
        assert_eq!(out, "[paraphrased] hello world");
    }
}
