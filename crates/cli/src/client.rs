use smarty_pants_core::{
    config::Config,
    paths,
    protocol::{Request, Response},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::UnixStream,
};

pub fn config_path() -> std::path::PathBuf {
    paths::expand("$XDG_CONFIG_HOME/smarty-pants/config.toml")
}

pub async fn send(request: Request) -> anyhow::Result<Response> {
    // Status/reload must still reach the daemon when unrelated settings are
    // invalid. A socket override also permits recovery from malformed TOML.
    let configured = std::fs::read_to_string(config_path())
        .ok()
        .and_then(|raw| toml::from_str::<toml::Value>(&raw).ok())
        .and_then(|value| {
            value
                .get("daemon")?
                .get("socket_path")?
                .as_str()
                .map(str::to_owned)
        });
    let socket = paths::expand(
        &std::env::var("SMARTY_PANTS_SOCKET")
            .ok()
            .or(configured)
            .unwrap_or_else(|| Config::default().daemon.socket_path),
    );
    let mut stream = UnixStream::connect(&socket).await.map_err(|e| {
        anyhow::anyhow!(
            "cannot connect to {}: {e}. Start it with `smarty-pants daemon start`.",
            socket.display()
        )
    })?;
    let body = serde_json::to_string(&request)?;
    stream.write_all(body.as_bytes()).await?;
    stream.write_all(b"\n").await?;
    let mut response = String::new();
    // The first local rewrite can download weights. Control calls should be quick.
    let timeout = if matches!(request, Request::Paraphrase { .. }) {
        1800
    } else {
        15
    };
    tokio::time::timeout(
        std::time::Duration::from_secs(timeout),
        BufReader::new(stream).read_line(&mut response),
    )
    .await??;
    Ok(serde_json::from_str(response.trim())?)
}

pub async fn control(request: Request) -> anyhow::Result<()> {
    match send(request).await? {
        Response::Ok { .. } => {
            println!("done");
            Ok(())
        }
        Response::Error { message, .. } => anyhow::bail!("{message}"),
        response => anyhow::bail!("unexpected response: {response:?}"),
    }
}
