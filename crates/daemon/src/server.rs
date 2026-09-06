//! One newline-delimited JSON request per Unix-socket connection.

use crate::{pipeline::Pipeline, settings::Settings};
use smarty_pants_core::protocol::{ErrorKind, Request, Response, SettingChange};
use std::{path::Path, sync::Arc};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio_util::sync::CancellationToken;

pub struct Server {
    listener: UnixListener,
    pipeline: Arc<Pipeline>,
    settings: Option<Arc<Settings>>,
    shutdown: CancellationToken,
}

impl Server {
    pub fn bind(socket_path: &Path, pipeline: Arc<Pipeline>) -> anyhow::Result<Self> {
        use std::os::unix::fs::PermissionsExt;
        if socket_path.exists() {
            // Do not disconnect a running daemon by unlinking its socket.
            match std::os::unix::net::UnixStream::connect(socket_path) {
                Ok(_) => {
                    anyhow::bail!("a daemon is already listening on {}", socket_path.display())
                }
                Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
                    std::fs::remove_file(socket_path)?;
                }
                Err(e) => return Err(e.into()),
            }
        }
        if let Some(parent) = socket_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let listener = UnixListener::bind(socket_path)?;
        std::fs::set_permissions(socket_path, std::fs::Permissions::from_mode(0o600))?;
        Ok(Self {
            listener,
            pipeline,
            settings: None,
            shutdown: CancellationToken::new(),
        })
    }

    pub fn with_settings(mut self, settings: Arc<Settings>) -> Self {
        self.settings = Some(settings);
        self
    }
    pub fn shutdown_token(&self) -> CancellationToken {
        self.shutdown.clone()
    }

    pub async fn serve(self) -> anyhow::Result<()> {
        let mut clients = tokio::task::JoinSet::new();
        loop {
            tokio::select! {
                _ = self.shutdown.cancelled() => break,
                client = self.listener.accept() => {
                    let (stream, _) = client?;
                    let pipeline = self.pipeline.clone();
                    let settings = self.settings.clone();
                    let shutdown = self.shutdown.clone();
                    clients.spawn(async move {
                        if let Err(e) = handle(stream, pipeline, settings, shutdown).await {
                            tracing::warn!(error = %e, "client error");
                        }
                    });
                }
                Some(_) = clients.join_next(), if !clients.is_empty() => {}
            }
        }
        Ok(())
    }
}

async fn handle(
    stream: UnixStream,
    pipeline: Arc<Pipeline>,
    settings: Option<Arc<Settings>>,
    shutdown: CancellationToken,
) -> anyhow::Result<()> {
    let (read, mut write) = stream.into_split();
    let mut reader = BufReader::new(read).take(64 * 1024);
    let mut line = String::new();
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        reader.read_line(&mut line),
    )
    .await??;
    if line.is_empty() {
        return Ok(());
    }
    anyhow::ensure!(line.ends_with('\n'), "request too large or incomplete");
    let req: Request = serde_json::from_str(line.trim())?;
    let should_shutdown = matches!(req, Request::Shutdown);
    let resp = match req {
        Request::Paraphrase { mode } => pipeline.run(&mode).await,
        Request::Status => pipeline.status().await,
        Request::Shutdown => success(),
        Request::UnloadModel => result_response(pipeline.unload().await),
        request => {
            let result = if let Some(settings) = settings {
                match request {
                    Request::Reload => settings.reload().await,
                    Request::SetPaused { paused } => {
                        settings.change(SettingChange::Paused { paused }).await
                    }
                    Request::Configure { change } => settings.change(change).await,
                    _ => unreachable!(),
                }
            } else {
                Err(anyhow::anyhow!("settings are unavailable"))
            };
            result_response(result)
        }
    };
    let body = serde_json::to_string(&resp)?;
    write.write_all(body.as_bytes()).await?;
    write.write_all(b"\n").await?;
    write.shutdown().await?;
    if should_shutdown {
        shutdown.cancel();
    }
    Ok(())
}

fn success() -> Response {
    Response::Ok {
        generated_chars: 0,
        ms: 0,
    }
}
fn result_response(result: anyhow::Result<()>) -> Response {
    match result {
        Ok(()) => success(),
        Err(e) => Response::Error {
            error_kind: ErrorKind::Configuration,
            message: e.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{llm::EchoLlm, wayland::mock::MockWayland};
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
        cfg.modes.insert(
            "rewrite".into(),
            ModeCfg {
                system: "rewrite".into(),
                shortcut: None,
                description: None,
                temperature: None,
                top_p: None,
                max_tokens: None,
            },
        );
        let pipe = Arc::new(Pipeline::new(wl, Arc::new(EchoLlm), Arc::new(cfg)));
        let server = Server::bind(&sock, pipe).unwrap();
        let h = tokio::spawn(async move {
            let _ = server.serve().await;
        });

        // Give it a moment to start listening.
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;

        let mut client = UnixStream::connect(&sock).await.unwrap();
        client
            .write_all(b"{\"kind\":\"paraphrase\",\"mode\":\"rewrite\"}\n")
            .await
            .unwrap();
        let mut buf = String::new();
        client.read_to_string(&mut buf).await.unwrap();
        let resp: Response = serde_json::from_str(buf.trim()).unwrap();
        assert!(matches!(resp, Response::Copied { .. }));

        h.abort();
    }
}
