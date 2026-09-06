use tokio::process::Command;

pub async fn start() -> anyhow::Result<()> {
    use smarty_pants_core::{
        paths,
        protocol::{Request, Response},
    };
    if matches!(
        crate::client::send(Request::Status).await,
        Ok(Response::Status { healthy: true, .. })
    ) {
        println!("daemon already running");
        return Ok(());
    }
    // Spawn the daemon binary detached. Look for it next to ourselves first
    // (cargo install layout), then fall back to PATH.
    let bin = locate_daemon_binary()?;
    let log_path = paths::expand("$XDG_STATE_HOME/smarty-pants/daemon.log");
    std::fs::create_dir_all(log_path.parent().unwrap())?;
    // Keep detached startup failures inspectable without printing config or credentials.
    use std::os::unix::fs::OpenOptionsExt;
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(&log_path)?;
    let mut cmd = Command::new(&bin);
    cmd.kill_on_drop(false);
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(log.try_clone()?);
    cmd.stderr(log);
    let mut child = cmd
        .spawn()
        .map_err(|e| anyhow::anyhow!("spawn {}: {e}", bin.display()))?;
    for _ in 0..50 {
        if let Some(status) = child.try_wait()? {
            anyhow::bail!("daemon exited ({status}); see {}", log_path.display());
        }
        if matches!(
            tokio::time::timeout(
                std::time::Duration::from_millis(200),
                crate::client::send(Request::Status)
            )
            .await,
            Ok(Ok(Response::Status { healthy: true, .. }))
        ) {
            println!(
                "daemon ready (pid {}); log: {}",
                child.id().unwrap_or(0),
                log_path.display()
            );
            return Ok(());
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    // Don't leave an untracked child behind after reporting a failed start.
    let _ = child.kill().await;
    anyhow::bail!("daemon did not become ready; see {}", log_path.display())
}

pub async fn stop() -> anyhow::Result<()> {
    crate::client::control(smarty_pants_core::protocol::Request::Shutdown).await
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
