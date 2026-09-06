//! End-to-end: spawn the daemon with stubs, drive it over a real socket,
//! verify the round-trip behavior the CLI relies on.

use smarty_pants_core::protocol::{Request, Response};
use smarty_pants_daemon::testing::run_with_stubs;
use smarty_pants_daemon::wayland::{ClipboardKind, Wayland};
use std::time::Duration;
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

#[tokio::test]
async fn happy_path_via_socket_uses_echo_llm_and_writes_clipboard() {
    let tmp = TempDir::new().unwrap();
    let sock = tmp.path().join("sp.sock");

    let (server_handle, wl) = run_with_stubs(&sock, "Hello, friend.").await;
    tokio::time::sleep(Duration::from_millis(30)).await;

    let mut client = UnixStream::connect(&sock).await.unwrap();
    let req = serde_json::to_string(&Request::Paraphrase {
        mode: "rewrite".into(),
    })
    .unwrap();
    client.write_all(req.as_bytes()).await.unwrap();
    client.write_all(b"\n").await.unwrap();
    let mut buf = String::new();
    client.read_to_string(&mut buf).await.unwrap();

    let resp: Response = serde_json::from_str(buf.trim()).unwrap();
    assert!(matches!(resp, Response::Ok { .. }), "got {resp:?}");

    // The EchoLlm output should now be on the regular clipboard via inject.
    let v = wl.read(ClipboardKind::Regular).await.unwrap();
    assert_eq!(v.as_deref(), Some("[paraphrased] Hello, friend."));

    server_handle.abort();
}

#[tokio::test]
async fn empty_selection_returns_empty_response() {
    let tmp = TempDir::new().unwrap();
    let sock = tmp.path().join("sp.sock");

    let (server_handle, _wl) = run_with_stubs(&sock, "").await;
    tokio::time::sleep(Duration::from_millis(30)).await;

    let mut client = UnixStream::connect(&sock).await.unwrap();
    let req = serde_json::to_string(&Request::Paraphrase {
        mode: "rewrite".into(),
    })
    .unwrap();
    client.write_all(req.as_bytes()).await.unwrap();
    client.write_all(b"\n").await.unwrap();
    let mut buf = String::new();
    client.read_to_string(&mut buf).await.unwrap();

    let resp: Response = serde_json::from_str(buf.trim()).unwrap();
    assert!(matches!(resp, Response::Empty), "got {resp:?}");

    server_handle.abort();
}

async fn request(socket: &std::path::Path, request: Request) -> Response {
    let mut client = UnixStream::connect(socket).await.unwrap();
    client
        .write_all(format!("{}\n", serde_json::to_string(&request).unwrap()).as_bytes())
        .await
        .unwrap();
    let mut buf = String::new();
    client.read_to_string(&mut buf).await.unwrap();
    serde_json::from_str(buf.trim()).unwrap()
}

#[tokio::test]
async fn status_reports_modes_pause_persists_and_shutdown_exits_server() {
    use smarty_pants_core::config::Config;
    use smarty_pants_daemon::{
        llm::EchoLlm, pipeline::Pipeline, server::Server, settings::Settings,
        wayland::mock::MockWayland,
    };
    use std::sync::Arc;
    let tmp = TempDir::new().unwrap();
    let sock = tmp.path().join("control.sock");
    let path = tmp.path().join("config.toml");
    let cfg = Config::load(&path).unwrap();
    let pipeline = Arc::new(Pipeline::new(
        Arc::new(MockWayland::new()),
        Arc::new(EchoLlm),
        Arc::new(cfg),
    ));
    let settings = Arc::new(Settings::new(pipeline.clone(), path.clone()).await);
    let server = Server::bind(&sock, pipeline.clone())
        .unwrap()
        .with_settings(settings);
    let handle = tokio::spawn(server.serve());
    assert!(matches!(
        request(&sock, Request::Status).await,
        Response::Status {
            mode_count: 4,
            paused: false,
            ..
        }
    ));
    assert!(matches!(
        request(&sock, Request::SetPaused { paused: true }).await,
        Response::Ok { .. }
    ));
    assert!(Config::load(&path).unwrap().daemon.paused);
    assert_eq!(
        request(
            &sock,
            Request::Paraphrase {
                mode: "rewrite".into()
            }
        )
        .await,
        Response::Paused
    );
    std::fs::write(&path, "[model]\ntemperature = 99\n").unwrap();
    assert!(matches!(
        request(&sock, Request::Reload).await,
        Response::Error { .. }
    ));
    assert!(matches!(
        request(&sock, Request::Status).await,
        Response::Status { paused: true, .. }
    ));
    // A second process cannot steal this daemon's listening socket.
    assert!(Server::bind(&sock, pipeline).is_err());
    assert!(matches!(
        request(&sock, Request::Shutdown).await,
        Response::Ok { .. }
    ));
    tokio::time::timeout(Duration::from_secs(2), handle)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}
