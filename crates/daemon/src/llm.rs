//! Provider-independent generation contract and a deterministic test backend.

use crate::prompt::Prompt;
use async_trait::async_trait;

#[derive(Debug, Clone)]
pub struct GenerationParams {
    pub max_tokens: u32,
    pub temperature: f32,
    pub top_p: f32,
    pub seed: u32,
}
impl Default for GenerationParams {
    fn default() -> Self {
        Self {
            max_tokens: 512,
            temperature: 0.2,
            top_p: 1.0,
            seed: 0,
        }
    }
}

#[async_trait]
pub trait Llm: Send + Sync + 'static {
    /// Return only a complete rewritten text. Errors must never be pasted.
    async fn generate(&self, prompt: &Prompt, params: &GenerationParams) -> anyhow::Result<String>;

    fn is_loaded(&self) -> bool {
        true
    }
    async fn unload(&self) {}
    async fn unload_if_idle(&self) {}
}

pub struct EchoLlm;

#[async_trait]
impl Llm for EchoLlm {
    async fn generate(&self, prompt: &Prompt, _: &GenerationParams) -> anyhow::Result<String> {
        Ok(format!("[paraphrased] {}", prompt.text.trim()))
    }
}
