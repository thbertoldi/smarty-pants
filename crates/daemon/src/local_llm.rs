//! Embedded llama.cpp inference. Compiled only with the `local` feature.

use crate::llm::{GenerationParams, Llm};
use async_trait::async_trait;
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
    template: crate::prompt::Template,
    use_gpu: bool,
    backend: Arc<LlamaBackend>,
    model: Arc<LlamaModel>,
    n_ctx: u32,
    n_threads: i32,
}

impl LlamaLlm {
    pub fn load(
        backend: Arc<LlamaBackend>,
        model_path: &Path,
        n_ctx: u32,
        n_threads: u32,
        gpu_layers: i32,
        gpu_main_device: u32,
        template: crate::prompt::Template,
    ) -> anyhow::Result<Self> {
        // Probe whether a GPU device is actually present. If the user
        // requested GPU offload (gpu_layers != 0) but none is detected,
        // warn and degrade to CPU rather than refusing to start.
        let gpu_available = backend.supports_gpu_offload() && gpu_devices_present();
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
            n_params = model.n_params(),
            n_layer = model.n_layer(),
            n_ctx_train = model.n_ctx_train(),
            "model loaded"
        );
        let n_threads = if n_threads == 0 {
            num_cpus_get_minus_one() as i32
        } else {
            n_threads as i32
        };
        Ok(Self {
            backend,
            model: Arc::new(model),
            n_ctx,
            n_threads,
            template,
            use_gpu: effective_gpu_layers != 0,
        })
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
    async fn generate(
        &self,
        prompt: &crate::prompt::Prompt,
        params: &GenerationParams,
    ) -> anyhow::Result<String> {
        // llama-cpp-2 is sync; run on blocking pool.
        let backend = self.backend.clone();
        let model = self.model.clone();
        let n_ctx = self.n_ctx;
        let n_threads = self.n_threads;
        let prompt = prompt.render(self.template);
        let params = params.clone();
        let use_gpu = self.use_gpu;

        tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
            let ctx_params = LlamaContextParams::default()
                .with_n_ctx(NonZeroU32::new(n_ctx))
                .with_offload_kqv(use_gpu)
                .with_op_offload(use_gpu)
                .with_n_threads(n_threads)
                .with_n_threads_batch(n_threads);
            let mut ctx = model.new_context(&backend, ctx_params)
                .map_err(|e| anyhow::anyhow!("new_context: {e}"))?;

            let tokens = model.str_to_token(&prompt, AddBos::Always)
                .map_err(|e| anyhow::anyhow!("tokenize: {e}"))?;
            anyhow::ensure!(!tokens.is_empty() && tokens.len() < ctx.n_ctx() as usize,
                "selected text exceeds the model context; select less text or increase model.context_size");
            let batch_size = ctx.n_batch() as usize;
            let mut batch = LlamaBatch::new(batch_size, 1);
            for (chunk_index, chunk) in tokens.chunks(batch_size).enumerate() {
                batch.clear();
                for (i, t) in chunk.iter().enumerate() {
                    let position = chunk_index * batch_size + i;
                    batch.add(*t, position as i32, &[0], position == tokens.len() - 1)
                        .map_err(|e| anyhow::anyhow!("batch: {e}"))?;
                }
                ctx.decode(&mut batch).map_err(|e| anyhow::anyhow!("decode prompt: {e}"))?;
            }

            let mut sampler = if params.temperature == 0.0 {
                LlamaSampler::greedy()
            } else { LlamaSampler::chain_simple([
                LlamaSampler::temp(params.temperature),
                LlamaSampler::top_p(params.top_p, 1),
                LlamaSampler::dist(if params.seed == 0 {
                    rand_seed()
                } else {
                    params.seed
                }),
            ]) };

            let mut out     = String::new();
            let mut n_cur   = tokens.len() as i32;
            let mut decoder = encoding_rs::UTF_8.new_decoder();
            for generated in 0..=params.max_tokens {
                let token = sampler.sample(&ctx, batch.n_tokens() - 1);
                if model.is_eog_token(token) {
                    anyhow::ensure!(!out.trim().is_empty(), "local model returned empty text");
                    return Ok(out.trim().to_owned());
                }
                anyhow::ensure!(generated < params.max_tokens,
                    "local output was truncated; increase model.max_tokens or select less text");
                anyhow::ensure!((n_cur as u32) < ctx.n_ctx(),
                    "local output exceeded the context; select less text or increase model.context_size");
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
            unreachable!("generation returns on end-of-text or token limit")
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
