use smarty_pants_core::{paths, protocol::{Request, Response}};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

pub async fn run() -> anyhow::Result<()> {
    let socket = paths::expand("$XDG_RUNTIME_DIR/smarty-pants.sock");
    match UnixStream::connect(&socket).await {
        Err(_) => {
            println!("daemon: not running");
            std::process::exit(2);
        }
        Ok(mut stream) => {
            let body = serde_json::to_string(&Request::Status)?;
            stream.write_all(body.as_bytes()).await?;
            stream.write_all(b"\n").await?;
            let mut buf = String::new();
            stream.read_to_string(&mut buf).await?;
            let resp: Response = serde_json::from_str(buf.trim())?;
            if let Response::Status { healthy, model_loaded, mode_count } = resp {
                println!("daemon: running ({}, model={}, modes={mode_count})",
                    if healthy { "healthy" } else { "unhealthy" },
                    if model_loaded { "loaded" } else { "loading" });
            } else {
                println!("daemon: unexpected response {resp:?}");
                std::process::exit(2);
            }
            Ok(())
        }
    }
}
