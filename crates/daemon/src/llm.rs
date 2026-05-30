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
