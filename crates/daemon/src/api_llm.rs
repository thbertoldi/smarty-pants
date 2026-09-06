//! Direct chat-completions transport for DeepSeek and compatible servers.

use crate::{
    llm::{GenerationParams, Llm},
    prompt::Prompt,
};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;
use smarty_pants_core::{
    config::{ApiCfg, Provider},
    paths,
};
use std::time::Duration;

pub struct ApiLlm {
    cfg: ApiCfg,
    provider: Provider,
    client: reqwest::Client,
    endpoint: String,
}

impl ApiLlm {
    pub fn new(cfg: ApiCfg, provider: Provider) -> anyhow::Result<Self> {
        let base = cfg.base_url.trim_end_matches('/');
        let endpoint = if base.ends_with("/chat/completions") {
            base.to_owned()
        } else {
            format!("{base}/chat/completions")
        };
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(cfg.timeout_seconds))
            .connect_timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Self {
            cfg,
            provider,
            client,
            endpoint,
        })
    }

    fn api_key(&self) -> anyhow::Result<Option<String>> {
        if let Some(path) = &self.cfg.api_key_file {
            let metadata = std::fs::metadata(paths::expand(path))
                .map_err(|_| anyhow::anyhow!("cannot read API key file; check api_key_file"))?;
            anyhow::ensure!(
                metadata.is_file() && metadata.len() <= 16 * 1024,
                "API key file must be a regular file of at most 16 KiB"
            );
            let value = std::fs::read_to_string(paths::expand(path))
                .map_err(|_| anyhow::anyhow!("cannot read API key file; check api_key_file"))?;
            anyhow::ensure!(!value.trim().is_empty(), "API key file is empty");
            return Ok(Some(value.trim().to_owned()));
        }
        if let Some(name) = &self.cfg.api_key_env {
            let value = std::env::var(name).ok().filter(|v| !v.trim().is_empty())
                .ok_or_else(|| anyhow::anyhow!("API key environment variable {name} is missing; set it in the daemon environment or configure api_key_file"))?;
            return Ok(Some(value.trim().to_owned()));
        }
        anyhow::ensure!(
            self.provider != Provider::Deepseek,
            "DeepSeek requires api_key_env or api_key_file"
        );
        Ok(None)
    }

    fn body(&self, prompt: &Prompt, params: &GenerationParams) -> serde_json::Value {
        let mut body = json!({
            "model": self.cfg.model,
            "messages": [
                {"role": "system", "content": prompt.system.trim()},
                {"role": "user", "content": prompt.user_message()},
            ],
            "temperature": params.temperature,
            "top_p": params.top_p,
            "max_tokens": params.max_tokens,
            "stream": false,
        });
        if self.provider == Provider::Deepseek {
            // Writing transforms need the answer, without a reasoning pass.
            body["thinking"] = json!({"type": "disabled"});
        }
        body
    }
}

#[async_trait]
impl Llm for ApiLlm {
    async fn generate(&self, prompt: &Prompt, params: &GenerationParams) -> anyhow::Result<String> {
        let mut request = self
            .client
            .post(&self.endpoint)
            .json(&self.body(prompt, params));
        if let Some(key) = self.api_key()? {
            let mut header = reqwest::header::HeaderValue::from_str(&format!("Bearer {key}"))
                .map_err(|_| anyhow::anyhow!("API key contains invalid header characters"))?;
            header.set_sensitive(true);
            request = request.header(reqwest::header::AUTHORIZATION, header);
        }
        let mut response = request.send().await.map_err(transport_error)?;
        let status = response.status();
        if !status.is_success() {
            // Provider bodies can echo input or credentials. Do not log/return them.
            let hint = match status.as_u16() {
                401 | 403 => "check the API key and account permissions",
                402 => "check the account balance",
                404 => "check the base URL and model name",
                429 => "rate limit reached; try again later",
                500..=599 => "provider unavailable; try again later",
                _ => "check the API configuration",
            };
            anyhow::bail!("API returned HTTP {}: {hint}", status.as_u16());
        }
        const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(transport_error)? {
            anyhow::ensure!(
                bytes.len() + chunk.len() <= MAX_RESPONSE_BYTES,
                "API response is too large"
            );
            bytes.extend_from_slice(&chunk);
        }
        parse_completion(&bytes)
    }

    fn is_loaded(&self) -> bool {
        false
    } // No local model is resident.
}

fn transport_error(error: reqwest::Error) -> anyhow::Error {
    if error.is_timeout() {
        anyhow::anyhow!("API request timed out; your text was not replaced")
    } else {
        anyhow::anyhow!("API connection failed; check the endpoint and network")
    }
}

#[derive(Deserialize)]
struct Completion {
    choices: Vec<Choice>,
}
#[derive(Deserialize)]
struct Choice {
    finish_reason: String,
    message: Message,
}
#[derive(Deserialize)]
struct Message {
    content: Option<String>,
    #[serde(default)]
    refusal: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<serde_json::Value>>,
}

fn parse_completion(bytes: &[u8]) -> anyhow::Result<String> {
    let completion: Completion = serde_json::from_slice(bytes)
        .map_err(|_| anyhow::anyhow!("API returned an invalid chat completion"))?;
    let choice = completion
        .choices
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("API returned no completion"))?;
    if choice.finish_reason == "length" {
        anyhow::bail!("API output was truncated; increase model.max_tokens or select less text");
    }
    anyhow::ensure!(
        choice.finish_reason == "stop",
        "API did not finish a text response; your text was not replaced"
    );
    anyhow::ensure!(
        choice.message.refusal.as_deref().unwrap_or("").is_empty()
            && choice.message.tool_calls.as_ref().is_none_or(Vec::is_empty),
        "API declined to return a rewrite"
    );
    let content = choice.message.content.unwrap_or_default();
    anyhow::ensure!(!content.trim().is_empty(), "API returned empty text");
    Ok(content.trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn accepts_explicit_null_tool_calls_as_text_only() {
        let response = r#"{"choices":[{"finish_reason":"stop","message":{"content":"Olá!","tool_calls":null}}]}"#;
        assert_eq!(parse_completion(response.as_bytes()).unwrap(), "Olá!");
    }

    #[test]
    fn rejects_incomplete_or_non_text_answers() {
        for reply in [
            json!({"choices": []}),
            json!({"choices": [{"finish_reason": "length", "message": {"content": "partial"}}]}),
            json!({"choices": [{"finish_reason": "content_filter", "message": {"content": "no"}}]}),
            json!({"choices": [{"finish_reason": "stop", "message": {"content": null, "reasoning_content": "private thoughts"}}]}),
            json!({"choices": [{"finish_reason": "stop", "message": {"content": " ", "refusal": "declined"}}]}),
        ] {
            assert!(parse_completion(&serde_json::to_vec(&reply).unwrap()).is_err());
        }
        assert!(parse_completion(b"not json").is_err());
    }

    async fn mock_api(status: &str, response: &str) -> (String, tokio::task::JoinHandle<String>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/v1/", listener.local_addr().unwrap());
        let response = format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{response}", response.len());
        let task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            loop {
                let mut buf = [0; 4096];
                let n = socket.read(&mut buf).await.unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&buf[..n]);
                if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&bytes[..end]).to_lowercase();
                    let len: usize = headers
                        .lines()
                        .find_map(|line| line.strip_prefix("content-length: "))
                        .unwrap()
                        .parse()
                        .unwrap();
                    if bytes.len() >= end + 4 + len {
                        break;
                    }
                }
            }
            socket.write_all(response.as_bytes()).await.unwrap();
            String::from_utf8(bytes).unwrap()
        });
        (url, task)
    }

    #[tokio::test]
    async fn deepseek_posts_roles_auth_and_non_thinking_mode() {
        let (url, request) = mock_api("200 OK", r#"{"choices":[{"finish_reason":"stop","message":{"content":"Olá, mundo!","reasoning_content":"do not paste this"}}]}"#).await;
        let dir = tempfile::tempdir().unwrap();
        let key = dir.path().join("api.key");
        std::fs::write(&key, "test-secret\n").unwrap();
        let llm = ApiLlm::new(
            ApiCfg {
                base_url: url,
                model: "deepseek-v4-flash".into(),
                api_key_file: Some(key.to_string_lossy().into()),
                ..ApiCfg::default()
            },
            Provider::Deepseek,
        )
        .unwrap();
        let output = llm
            .generate(
                &Prompt::new("Rewrite faithfully.", "Olá mundo", Some("Portuguese")),
                &GenerationParams::default(),
            )
            .await
            .unwrap();
        assert_eq!(output, "Olá, mundo!");
        let request = request.await.unwrap();
        assert!(request.starts_with("POST /v1/chat/completions HTTP/1.1"));
        assert!(request
            .to_lowercase()
            .contains("authorization: bearer test-secret"));
        let body: serde_json::Value =
            serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
        assert_eq!(body["thinking"]["type"], "disabled");
        assert_eq!(body["messages"][0]["role"], "system");
        assert!(body["messages"][1]["content"]
            .as_str()
            .unwrap()
            .contains("Portuguese"));
        assert!(!body.to_string().contains("<|im_start|>"));
    }

    #[tokio::test]
    async fn compatible_api_omits_auth_and_provider_specific_fields() {
        let (url, request) = mock_api("429 Too Many Requests", "secret echoed by server").await;
        let llm = ApiLlm::new(
            ApiCfg {
                base_url: url,
                model: "small-editor".into(),
                ..ApiCfg::default()
            },
            Provider::OpenaiCompatible,
        )
        .unwrap();
        let error = llm
            .generate(
                &Prompt::new("rewrite", "input", None),
                &GenerationParams::default(),
            )
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("429"));
        assert!(!error.contains("secret"));
        let request = request.await.unwrap();
        assert!(!request.to_lowercase().contains("authorization:"));
        assert!(!request.contains("thinking"));
    }

    #[tokio::test]
    async fn missing_credentials_fail_before_any_network_call() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = ApiCfg {
            base_url: "http://127.0.0.1:1".into(),
            model: "editor".into(),
            api_key_file: Some(dir.path().join("missing.key").to_string_lossy().into()),
            ..ApiCfg::default()
        };
        let llm = ApiLlm::new(cfg, Provider::Deepseek).unwrap();
        let error = llm
            .generate(
                &Prompt::new("rewrite", "text", None),
                &GenerationParams::default(),
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("API key file"));
    }

    #[tokio::test]
    async fn request_timeout_is_bounded() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let cfg = ApiCfg {
            base_url: format!("http://{}", listener.local_addr().unwrap()),
            model: "editor".into(),
            timeout_seconds: 1,
            ..ApiCfg::default()
        };
        let llm = ApiLlm::new(cfg, Provider::OpenaiCompatible).unwrap();
        let error = tokio::time::timeout(
            Duration::from_secs(3),
            llm.generate(
                &Prompt::new("rewrite", "text", None),
                &GenerationParams::default(),
            ),
        )
        .await
        .unwrap()
        .unwrap_err();
        assert!(error.to_string().contains("timed out"));
    }
}
