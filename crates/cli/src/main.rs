mod trigger;
mod status;
mod daemon_cmd;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version, about = "smarty-pants — local AI paraphraser for Wayland")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Paraphrase the current text selection using a configured mode.
    Trigger {
        #[arg(long, default_value = "rewrite")]
        mode: String,
    },
    /// Show daemon health.
    Status,
    /// Manage the daemon process.
    Daemon {
        #[command(subcommand)]
        sub: DaemonSub,
    },
}

#[derive(Subcommand)]
enum DaemonSub {
    /// Start the daemon as a detached child process.
    Start,
    /// Send shutdown to the running daemon via the socket.
    Stop,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Trigger { mode } => trigger::run(&mode).await,
        Cmd::Status            => status::run().await,
        Cmd::Daemon { sub: DaemonSub::Start } => daemon_cmd::start().await,
        Cmd::Daemon { sub: DaemonSub::Stop  } => daemon_cmd::stop().await,
    }
}
