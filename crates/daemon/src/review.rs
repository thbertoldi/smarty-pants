//! Explicit review of both texts. Contents travel over stdin, never process arguments.

use tokio::{io::AsyncWriteExt, process::Command};

pub async fn show(original: &str, generated: &str) -> anyhow::Result<bool> {
    let mut child = Command::new("zenity")
        .args([
            "--text-info",
            "--title=Review rewrite — smarty-pants",
            "--width=760",
            "--height=600",
            "--ok-label=Copy rewrite",
            "--cancel-label=Discard",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| {
            anyhow::anyhow!("Install zenity to review rewrites, or select Copy only in the tray")
        })?;
    let body = format!("ORIGINAL\n\n{original}\n\n────────────────────────────────\nREWRITE\n\n{generated}\n\n────────────────────────────────\nCheck meaning, names and facts. Copy rewrite puts it on your clipboard; paste it where you want.");
    let mut stdin = child.stdin.take().unwrap();
    // If the dialog is dismissed while writing, still reap the child and honor Cancel.
    let written = stdin.write_all(body.as_bytes()).await;
    drop(stdin);
    let status = child.wait().await?;
    if status.code() == Some(1) {
        return Ok(false);
    }
    anyhow::ensure!(
        status.success(),
        "review dialog failed; selection was not replaced"
    );
    written?;
    Ok(true)
}
