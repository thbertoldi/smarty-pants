use smarty_pants_core::paths;
use tokio::process::Command;

pub async fn start() -> anyhow::Result<()> {
    // Spawn the daemon binary detached. Look for it next to ourselves first
    // (cargo install layout), then fall back to PATH.
    let bin = locate_daemon_binary()?;
    let mut cmd = Command::new(&bin);
    cmd.kill_on_drop(false);
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());
    let child = cmd.spawn()
        .map_err(|e| anyhow::anyhow!("spawn {}: {e}", bin.display()))?;
    println!("daemon started (pid {})", child.id().unwrap_or(0));
    Ok(())
}

pub async fn stop() -> anyhow::Result<()> {
    use smarty_pants_core::protocol::Request;
    use tokio::io::AsyncWriteExt;
    use tokio::net::UnixStream;
    let socket = paths::expand("$XDG_RUNTIME_DIR/smarty-pants.sock");
    let Ok(mut stream) = UnixStream::connect(&socket).await else {
        println!("daemon: not running");
        return Ok(());
    };
    let body = serde_json::to_string(&Request::Shutdown)?;
    stream.write_all(body.as_bytes()).await?;
    stream.write_all(b"\n").await?;
    println!("daemon stop requested");
    Ok(())
}

fn locate_daemon_binary() -> anyhow::Result<std::path::PathBuf> {
    if let Ok(self_path) = std::env::current_exe() {
        if let Some(dir) = self_path.parent() {
            let candidate = dir.join("smarty-pants-daemon");
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }
    which::which("smarty-pants-daemon")
        .map_err(|_| anyhow::anyhow!("smarty-pants-daemon not found on PATH"))
}
