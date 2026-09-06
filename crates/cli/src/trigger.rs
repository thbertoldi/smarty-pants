use smarty_pants_core::protocol::{Request, Response};

pub async fn run(mode: &str) -> anyhow::Result<()> {
    let resp = crate::client::send(Request::Paraphrase {
        mode: mode.to_owned(),
    })
    .await?;
    match resp {
        Response::Ok {
            generated_chars,
            ms,
        } => {
            eprintln!("ok — paraphrased {generated_chars} chars in {ms} ms");
            Ok(())
        }
        Response::Copied {
            generated_chars,
            ms,
            reason,
        } => {
            eprintln!("copied {generated_chars} chars in {ms} ms — {reason}");
            Ok(())
        }
        Response::Cancelled => {
            eprintln!("rewrite discarded");
            Ok(())
        }
        Response::Empty => {
            eprintln!("no new selection captured; select text that exposes PRIMARY, or ensure Copy changes the clipboard");
            std::process::exit(3)
        }
        Response::Paused => {
            eprintln!("daemon paused; use `smarty-pants resume`");
            std::process::exit(6)
        }
        Response::Busy => {
            eprintln!("daemon busy");
            std::process::exit(4)
        }
        Response::ModelLoading => {
            eprintln!("model loading");
            std::process::exit(5)
        }
        Response::Status { .. } => unreachable!("trigger sent paraphrase, got Status"),
        Response::Error {
            error_kind,
            message,
        } => {
            eprintln!("error ({error_kind:?}): {message}");
            std::process::exit(1)
        }
    }
}
