//! Fetch a model file to disk and verify its SHA-256.
//!
//! Phase 1 ships a single hardcoded model. The URL+hash live as
//! associated constants below; Phase 2 moves them into config.

use crate::prompt::Template;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

pub struct ModelSpec {
    pub key:           &'static str,
    pub url:           &'static str,
    pub sha256:        &'static str, // hex lowercase
    #[allow(dead_code)] // Phase 2: progress reporting
    pub size:          u64,
    /// Chat template the GGUF expects. Different model families need
    /// different turn delimiters; passing the wrong one produces garbage.
    pub chat_template: Template,
}

#[allow(dead_code)] // kept for reference / future fallback
pub const GEMMA_3_1B_IT_Q4_K_M: ModelSpec = ModelSpec {
    key:           "gemma-3-1b-it-q4_k_m",
    url:           "https://huggingface.co/unsloth/gemma-3-1b-it-GGUF/resolve/main/gemma-3-1b-it-Q4_K_M.gguf",
    // Pinned 2026-05-31 from a fresh download of the URL above.
    sha256:        "8270790f3ab69fdfe860b7b64008d9a19986d8df7e407bb018184caa08798ebd",
    size:          806_058_272,
    chat_template: Template::Gemma,
};

pub const QWEN_2_5_7B_IT_Q4_K_M: ModelSpec = ModelSpec {
    key:           "qwen-2.5-7b-instruct-q4_k_m",
    url:           "https://huggingface.co/bartowski/Qwen2.5-7B-Instruct-GGUF/resolve/main/Qwen2.5-7B-Instruct-Q4_K_M.gguf",
    // Pinned 2026-05-31 from a fresh download of the URL above.
    sha256:        "65b8fcd92af6b4fefa935c625d1ac27ea29dcb6ee14589c55a8f115ceaaa1423",
    size:          4_683_074_240,
    chat_template: Template::ChatML,
};

pub async fn ensure_model(
    spec:     &ModelSpec,
    data_dir: &Path,
) -> anyhow::Result<PathBuf> {
    let dst = data_dir.join(format!("{}.gguf", spec.key));
    if dst.exists() && verify_sha256(&dst, spec.sha256).await.unwrap_or(false) {
        tracing::info!(path = %dst.display(), "model already present and verified");
        return Ok(dst);
    }
    download(spec.url, &dst).await?;
    if !verify_sha256(&dst, spec.sha256).await? {
        let _ = tokio::fs::remove_file(&dst).await;
        anyhow::bail!("downloaded model sha256 does not match expected {}", spec.sha256);
    }
    tracing::info!(path = %dst.display(), "model downloaded and verified");
    Ok(dst)
}

async fn download(url: &str, dst: &Path) -> anyhow::Result<()> {
    if let Some(parent) = dst.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let tmp = dst.with_extension("gguf.partial");
    let resp = reqwest::get(url).await?.error_for_status()?;
    let mut stream = resp.bytes_stream();
    let mut file = tokio::fs::File::create(&tmp).await?;
    use futures::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).await?;
    }
    file.sync_all().await?;
    tokio::fs::rename(&tmp, dst).await?;
    Ok(())
}

async fn verify_sha256(path: &Path, expected_hex: &str) -> anyhow::Result<bool> {
    let bytes = tokio::fs::read(path).await?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let got = hex::encode(hasher.finalize());
    Ok(got.eq_ignore_ascii_case(expected_hex))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn verify_sha256_matches() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("x.bin");
        tokio::fs::write(&p, b"hello").await.unwrap();
        // sha256("hello") = 2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824
        assert!(verify_sha256(
            &p,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824",
        ).await.unwrap());
    }

    #[tokio::test]
    async fn verify_sha256_rejects_mismatch() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("x.bin");
        tokio::fs::write(&p, b"hello").await.unwrap();
        assert!(!verify_sha256(&p, "deadbeef").await.unwrap());
    }
}
