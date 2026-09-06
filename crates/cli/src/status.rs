use smarty_pants_core::protocol::{Request, Response};

pub async fn run() -> anyhow::Result<()> {
    match crate::client::send(Request::Status).await? {
        Response::Status {
            healthy,
            model_loaded,
            mode_count,
            provider,
            model,
            paused,
            busy,
            last_error,
            last_result,
        } => {
            println!("daemon: {}", if healthy { "running" } else { "unhealthy" });
            println!("provider: {provider}\nmodel: {model}");
            if provider == "local" {
                println!(
                    "local model: {}",
                    if model_loaded {
                        "loaded"
                    } else {
                        "loads on next rewrite"
                    }
                );
            }
            println!("paused: {paused}\nbusy: {busy}\nmodes: {mode_count}");
            if let Some(error) = last_error {
                println!("last error: {error}");
            }
            if let Some(result) = last_result {
                println!("last result: {result}");
            }
            Ok(())
        }
        response => anyhow::bail!("unexpected response: {response:?}"),
    }
}
