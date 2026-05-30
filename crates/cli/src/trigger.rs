use smarty_pants_core::{paths, protocol::{Request, Response}};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

pub async fn run(mode: &str) -> anyhow::Result<()> {
    let socket = paths::expand("$XDG_RUNTIME_DIR/smarty-pants.sock");
    let mut stream = UnixStream::connect(&socket).await.map_err(|e| {
        anyhow::anyhow!(
            "cannot connect to {}: {e}. Is the daemon running? Try `smarty-pants daemon start`.",
            socket.display()
        )
    })?;
    let req = Request::Paraphrase { mode: mode.to_owned() };
    let body = serde_json::to_string(&req)?;
    stream.write_all(body.as_bytes()).await?;
    stream.write_all(b"\n").await?;

    let mut buf = String::new();
    stream.read_to_string(&mut buf).await?;
    let resp: Response = serde_json::from_str(buf.trim())?;
    match resp {
        Response::Ok { generated_chars, ms } => {
            eprintln!("ok — paraphrased {generated_chars} chars in {ms} ms");
            Ok(())
        }
        Response::Empty        => { eprintln!("no text selected"); std::process::exit(3) }
        Response::Busy         => { eprintln!("daemon busy"); std::process::exit(4) }
        Response::ModelLoading => { eprintln!("model loading"); std::process::exit(5) }
        Response::Status { .. } => unreachable!("trigger sent paraphrase, got Status"),
        Response::Error { error_kind, message } => {
            eprintln!("error ({error_kind:?}): {message}");
            std::process::exit(1)
        }
    }
}
