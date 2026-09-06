mod client;
mod daemon_cmd;
mod status;
mod trigger;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version, about = "smarty-pants — AI writing assistant for Wayland")]
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
    /// Pause rewriting (saved across restarts).
    Pause,
    /// Resume rewriting.
    Resume,
    /// Release the local model's memory; it loads again on the next rewrite.
    Unload,
    /// Edit or apply settings without restarting the daemon.
    Config {
        #[command(subcommand)]
        sub: ConfigSub,
    },
    /// Manage the daemon process.
    Daemon {
        #[command(subcommand)]
        sub: DaemonSub,
    },
}

#[derive(Subcommand)]
enum ConfigSub {
    /// Print the configuration file location.
    Path,
    /// Open configuration in the desktop's default editor.
    Edit,
    /// Validate and apply the file's settings to the running daemon.
    Reload,
    /// Select local inference, DeepSeek, or a compatible API.
    Provider {
        #[arg(value_enum)]
        provider: ProviderArg,
    },
    /// Choose a built-in local model preset (downloaded on first use).
    Model { name: String },
}

#[derive(Clone, clap::ValueEnum)]
enum ProviderArg {
    Local,
    Deepseek,
    OpenaiCompatible,
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
        Cmd::Status => status::run().await,
        Cmd::Pause => {
            client::control(smarty_pants_core::protocol::Request::SetPaused { paused: true }).await
        }
        Cmd::Resume => {
            client::control(smarty_pants_core::protocol::Request::SetPaused { paused: false }).await
        }
        Cmd::Unload => client::control(smarty_pants_core::protocol::Request::UnloadModel).await,
        Cmd::Config { sub } => configure(sub).await,
        Cmd::Daemon {
            sub: DaemonSub::Start,
        } => daemon_cmd::start().await,
        Cmd::Daemon {
            sub: DaemonSub::Stop,
        } => daemon_cmd::stop().await,
    }
}

async fn configure(sub: ConfigSub) -> anyhow::Result<()> {
    use smarty_pants_core::{
        config::{Config, Provider},
        config_file,
        protocol::{Request, SettingChange},
    };
    match sub {
        ConfigSub::Path => {
            println!("{}", client::config_path().display());
            Ok(())
        }
        ConfigSub::Edit => {
            let path = client::config_path();
            if !path.exists() {
                config_file::initialize(&path, &Config::load(&path)?)?;
            }
            let status = tokio::process::Command::new("xdg-open")
                .arg(path)
                .status()
                .await?;
            anyhow::ensure!(status.success(), "could not open the configuration file");
            Ok(())
        }
        ConfigSub::Reload => client::control(Request::Reload).await,
        ConfigSub::Provider { provider } => {
            client::control(Request::Configure {
                change: SettingChange::Provider {
                    provider: match provider {
                        ProviderArg::Local => Provider::Local,
                        ProviderArg::Deepseek => Provider::Deepseek,
                        ProviderArg::OpenaiCompatible => Provider::OpenaiCompatible,
                    },
                },
            })
            .await
        }
        ConfigSub::Model { name } => {
            client::control(Request::Configure {
                change: SettingChange::LocalModel { name },
            })
            .await
        }
    }
}
