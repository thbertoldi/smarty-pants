//! Fetch a model file to disk and verify its SHA-256.
//!
//! Presets are selected by config; user-supplied GGUF files need no download.

use crate::prompt::Template;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub struct ModelSpec {
    pub key: &'static str,
    pub url: &'static str,
    pub sha256: &'static str, // hex lowercase
    #[allow(dead_code)] // Phase 2: progress reporting
    pub size: u64,
    /// Chat template the GGUF expects. Different model families need
    /// different turn delimiters; passing the wrong one produces garbage.
    pub chat_template: Template,
    pub label: &'static str,
}

// Official Qwen GGUF revisions and LFS SHA-256 values checked 2026-09-06.
pub const QWEN_2_5_1_5B_IT_Q4_K_M: ModelSpec = ModelSpec {
    key: "qwen-2.5-1.5b-instruct-q4_k_m",
    url: "https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct-GGUF/resolve/91cad51170dc346986eccefdc2dd33a9da36ead9/qwen2.5-1.5b-instruct-q4_k_m.gguf",
    sha256: "6a1a2eb6d15622bf3c96857206351ba97e1af16c30d7a74ee38970e434e9407e",
    size: 1_117_320_736,
    chat_template: Template::ChatML,
    label: "Qwen 2.5 1.5B — 1.1 GB (default)",
};

pub const QWEN_2_5_3B_IT_Q4_K_M: ModelSpec = ModelSpec {
    key: "qwen-2.5-3b-instruct-q4_k_m",
    url: "https://huggingface.co/Qwen/Qwen2.5-3B-Instruct-GGUF/resolve/7dabda4d13d513e3e842b20f0d435c732f172cbe/qwen2.5-3b-instruct-q4_k_m.gguf",
    sha256: "626b4a6678b86442240e33df819e00132d3ba7dddfe1cdc4fbb18e0a9615c62d",
    size: 2_104_932_768,
    chat_template: Template::ChatML,
    label: "Qwen 2.5 3B — 2.1 GB",
};

#[allow(dead_code)] // kept for reference / future fallback
pub const GEMMA_3_1B_IT_Q4_K_M: ModelSpec = ModelSpec {
    key: "gemma-3-1b-it-q4_k_m",
    url: "https://huggingface.co/unsloth/gemma-3-1b-it-GGUF/resolve/f0b45be0aac41bd6a100a4b5734cad5f67255bfb/gemma-3-1b-it-Q4_K_M.gguf",
    // Pinned 2026-05-31 from a fresh download of the URL above.
    sha256: "8270790f3ab69fdfe860b7b64008d9a19986d8df7e407bb018184caa08798ebd",
    size: 806_058_272,
    chat_template: Template::Gemma,
    label: "Gemma 3 1B — 0.8 GB",
};

pub const QWEN_2_5_7B_IT_Q4_K_M: ModelSpec = ModelSpec {
    key:           "qwen-2.5-7b-instruct-q4_k_m",
    url:           "https://huggingface.co/bartowski/Qwen2.5-7B-Instruct-GGUF/resolve/8911e8a47f92bac19d6f5c64a2e2095bd2f7d031/Qwen2.5-7B-Instruct-Q4_K_M.gguf",
    // Pinned 2026-05-31 from a fresh download of the URL above.
    sha256:        "65b8fcd92af6b4fefa935c625d1ac27ea29dcb6ee14589c55a8f115ceaaa1423",
    size:          4_683_074_240,
    chat_template: Template::ChatML,
    label:         "Qwen 2.5 7B — 4.7 GB",
};

pub const PRESETS: &[ModelSpec] = &[
    QWEN_2_5_1_5B_IT_Q4_K_M,
    QWEN_2_5_3B_IT_Q4_K_M,
    QWEN_2_5_7B_IT_Q4_K_M,
    GEMMA_3_1B_IT_Q4_K_M,
];

pub fn find(key: &str) -> anyhow::Result<&'static ModelSpec> {
    PRESETS.iter().find(|spec| spec.key == key)
        .ok_or_else(|| anyhow::anyhow!("unknown model preset: {key}; choose a preset or set model.path and model.chat_template"))
}

pub async fn ensure_model(spec: &ModelSpec, data_dir: &Path) -> anyhow::Result<PathBuf> {
    let dst = data_dir.join(format!("{}.gguf", spec.key));
    if dst.exists() && verify_sha256(&dst, spec.sha256).await.unwrap_or(false) {
        tracing::info!(path = %dst.display(), "model already present and verified");
        return Ok(dst);
    }
    tracing::info!(
        model = spec.key,
        bytes = spec.size,
        "downloading model on first use"
    );
    download(spec, &dst).await?;
    tracing::info!(path = %dst.display(), "model downloaded and verified");
    Ok(dst)
}

async fn download(spec: &ModelSpec, dst: &Path) -> anyhow::Result<()> {
    if let Some(parent) = dst.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let tmp = dst.with_extension("gguf.partial");
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(15))
        .read_timeout(std::time::Duration::from_secs(60))
        .build()?;
    let resp = client.get(spec.url).send().await?.error_for_status()?;
    let mut stream = resp.bytes_stream();
    let mut file = tokio::fs::File::create(&tmp).await?;
    let mut hasher = Sha256::new();
    use futures::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).await?;
        hasher.update(&chunk);
    }
    file.sync_all().await?;
    drop(file);
    if !hex::encode(hasher.finalize()).eq_ignore_ascii_case(spec.sha256) {
        let _ = tokio::fs::remove_file(&tmp).await;
        anyhow::bail!(
            "downloaded model sha256 does not match expected {}",
            spec.sha256
        );
    }
    tokio::fs::rename(&tmp, dst).await?;
    Ok(())
}

async fn verify_sha256(path: &Path, expected_hex: &str) -> anyhow::Result<bool> {
    let mut file = tokio::fs::File::open(path).await?;
    let mut hasher = Sha256::new();
    let mut bytes = vec![0; 64 * 1024];
    loop {
        let n = file.read(&mut bytes).await?;
        if n == 0 {
            break;
        }
        hasher.update(&bytes[..n]);
    }
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
        )
        .await
        .unwrap());
    }

    #[tokio::test]
    async fn verify_sha256_rejects_mismatch() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("x.bin");
        tokio::fs::write(&p, b"hello").await.unwrap();
        assert!(!verify_sha256(&p, "deadbeef").await.unwrap());
    }
}
