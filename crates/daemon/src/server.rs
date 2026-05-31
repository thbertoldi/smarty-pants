//! Unix-socket server. One newline-delimited JSON request per connection.

use crate::pipeline::Pipeline;
use smarty_pants_core::protocol::{Request, Response};
use std::path::Path;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};

pub struct Server {
    listener: UnixListener,
    pipeline: Arc<Pipeline>,
}

impl Server {
    pub fn bind(socket_path: &Path, pipeline: Arc<Pipeline>) -> anyhow::Result<Self> {
        // Remove stale socket from a prior crashed daemon.
        let _ = std::fs::remove_file(socket_path);
        if let Some(parent) = socket_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let listener = UnixListener::bind(socket_path)?;
        Ok(Self { listener, pipeline })
    }

    pub async fn serve(self) -> anyhow::Result<()> {
        loop {
            let (stream, _) = self.listener.accept().await?;
            let pipeline = self.pipeline.clone();
            tokio::spawn(async move {
                if let Err(e) = handle(stream, pipeline).await {
                    tracing::warn!(error = %e, "client error");
                }
            });
        }
    }
}

async fn handle(stream: UnixStream, pipeline: Arc<Pipeline>) -> anyhow::Result<()> {
    let (read, mut write) = stream.into_split();
    let mut reader = BufReader::new(read);
    let mut line = String::new();
    reader.read_line(&mut line).await?;
    let req: Request = serde_json::from_str(line.trim())?;

    let resp: Response = match req {
        Request::Paraphrase { mode } => pipeline.run(&mode).await,
        Request::Status => Response::Status {
            healthy:      true,
            model_loaded: true, // Phase 1 always reports loaded once main wires it
            mode_count:   pipeline_mode_count(&pipeline),
        },
        Request::Shutdown => Response::Ok { generated_chars: 0, ms: 0 },
    };

    let body = serde_json::to_string(&resp)?;
    write.write_all(body.as_bytes()).await?;
    write.write_all(b"\n").await?;
    write.shutdown().await?;
    Ok(())
}

// Phase 1 has one hardcoded mode. T16 will pass real count via Pipeline.
fn pipeline_mode_count(_p: &Pipeline) -> usize { 1 }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{llm::EchoLlm, prompt::Template, wayland::mock::MockWayland};
    use smarty_pants_core::config::{Config, ModeCfg};
    use tempfile::TempDir;
    use tokio::io::AsyncReadExt;

    #[tokio::test]
    async fn round_trip_paraphrase() {
        let tmp = TempDir::new().unwrap();
        let sock = tmp.path().join("sp.sock");

        let wl = Arc::new(MockWayland::new());
        wl.set_primary(Some("hi"));
        let mut cfg = Config::default();
        cfg.modes.insert("rewrite".into(), ModeCfg {
            system: "rewrite".into(),
            shortcut: None, description: None,
            temperature: None, top_p: None, max_tokens: None,
        });
        let pipe = Arc::new(Pipeline::new(wl, Arc::new(EchoLlm), Arc::new(cfg), Template::Gemma));
        let server = Server::bind(&sock, pipe).unwrap();
        let h = tokio::spawn(async move { let _ = server.serve().await; });

        // Give it a moment to start listening.
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;

        let mut client = UnixStream::connect(&sock).await.unwrap();
        client.write_all(b"{\"kind\":\"paraphrase\",\"mode\":\"rewrite\"}\n").await.unwrap();
        let mut buf = String::new();
        client.read_to_string(&mut buf).await.unwrap();
        let resp: Response = serde_json::from_str(buf.trim()).unwrap();
        assert!(matches!(resp, Response::Ok { .. }));

        h.abort();
    }
}
