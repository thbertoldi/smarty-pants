pub mod api_llm;
pub mod backend;
pub mod inject;
pub mod language;
pub mod llm;
#[cfg(feature = "local")]
pub mod local_llm;
pub mod model_download;
pub mod pipeline;
pub mod prompt;
pub mod selection;
pub mod server;
pub mod settings;
pub mod shortcuts;
#[cfg(feature = "tray")]
pub mod tray;
pub mod wayland;
pub mod writing;

/// Test helpers for workspace-level integration tests.
///
/// Exposed at the library level (no cfg gate) so cargo integration tests
/// can use them — `cfg(test)` doesn't propagate from an integration test
/// back to the library it depends on. `#[doc(hidden)]` keeps these out of
/// the public-facing docs.
#[doc(hidden)]
pub mod testing {
    use crate::{llm::EchoLlm, pipeline::Pipeline, server::Server, wayland::mock::MockWayland};
    use smarty_pants_core::config::{Config, ModeCfg};
    use std::path::Path;
    use std::sync::Arc;

    /// Spawn a daemon with EchoLlm + MockWayland on the given socket.
    /// Returns the server-task abort handle and a clone of the mock Wayland
    /// so the test can drive it (e.g., set what's on the primary clipboard).
    pub async fn run_with_stubs(
        socket: &Path,
        primary_text: &str,
    ) -> (tokio::task::JoinHandle<()>, Arc<MockWayland>) {
        let wl = Arc::new(MockWayland::new());
        wl.set_primary(Some(primary_text));
        let mut cfg = Config::default();
        cfg.modes.insert(
            "rewrite".into(),
            ModeCfg {
                system: "rewrite".into(),
                shortcut: None,
                description: None,
                temperature: None,
                top_p: None,
                max_tokens: None,
            },
        );
        let pipe = Arc::new(Pipeline::new(wl.clone(), Arc::new(EchoLlm), Arc::new(cfg)));
        let server = Server::bind(socket, pipe).expect("bind");
        let handle = tokio::spawn(async move {
            let _ = server.serve().await;
        });
        (handle, wl)
    }
}
