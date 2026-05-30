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
    let req = serde_json::to_string(&Request::Paraphrase { mode: "rewrite".into() }).unwrap();
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
    let req = serde_json::to_string(&Request::Paraphrase { mode: "rewrite".into() }).unwrap();
    client.write_all(req.as_bytes()).await.unwrap();
    client.write_all(b"\n").await.unwrap();
    let mut buf = String::new();
    client.read_to_string(&mut buf).await.unwrap();

    let resp: Response = serde_json::from_str(buf.trim()).unwrap();
    assert!(matches!(resp, Response::Empty), "got {resp:?}");

    server_handle.abort();
}
