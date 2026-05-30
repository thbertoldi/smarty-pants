//! Abstraction over Wayland clipboard + keystroke synthesis.
//!
//! `RealWayland` (added in Task 8) lives in the same file behind a sub-module
//! so unit tests can be written against `MockWayland` without linking
//! `wl-clipboard-rs`.

use async_trait::async_trait;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ClipboardKind { Primary, Regular }

#[async_trait]
#[allow(dead_code)]
pub trait Wayland: Send + Sync + 'static {
    async fn read(&self, kind: ClipboardKind) -> anyhow::Result<Option<String>>;
    async fn write_regular(&self, text: &str) -> anyhow::Result<()>;
    /// Synthesize a single key combo like "ctrl+c" or "ctrl+v".
    async fn type_combo(&self, combo: &str) -> anyhow::Result<()>;
}

// ── in-memory mock for unit tests ─────────────────────────────────────
#[cfg(any(test, feature = "live-model"))]
pub mod mock {
    use super::*;
    use std::sync::Mutex;

    /// Records every interaction; lets a test seed clipboards and assert
    /// the daemon issued the expected key sequence.
    #[derive(Default)]
    pub struct MockWayland {
        pub primary:        Mutex<Option<String>>,
        pub regular:        Mutex<Option<String>>,
        pub combos:         Mutex<Vec<String>>,
        /// If set, a Ctrl+C combo causes `primary` to be copied into `regular`
        /// so the selection capture loop sees something.
        pub ctrl_c_copies_primary_into_regular: bool,
    }

    impl MockWayland {
        pub fn new() -> Self { Self::default() }

        pub fn set_primary(&self, s: Option<&str>) {
            *self.primary.lock().unwrap() = s.map(str::to_owned);
        }
        pub fn set_regular(&self, s: Option<&str>) {
            *self.regular.lock().unwrap() = s.map(str::to_owned);
        }
        pub fn combos(&self) -> Vec<String> {
            self.combos.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl Wayland for MockWayland {
        async fn read(&self, kind: ClipboardKind) -> anyhow::Result<Option<String>> {
            Ok(match kind {
                ClipboardKind::Primary => self.primary.lock().unwrap().clone(),
                ClipboardKind::Regular => self.regular.lock().unwrap().clone(),
            })
        }

        async fn write_regular(&self, text: &str) -> anyhow::Result<()> {
            *self.regular.lock().unwrap() = Some(text.to_owned());
            Ok(())
        }

        async fn type_combo(&self, combo: &str) -> anyhow::Result<()> {
            self.combos.lock().unwrap().push(combo.to_owned());
            if combo == "ctrl+c" && self.ctrl_c_copies_primary_into_regular {
                let p = self.primary.lock().unwrap().clone();
                if let Some(p) = p {
                    *self.regular.lock().unwrap() = Some(p);
                }
            }
            Ok(())
        }
    }
}

// ── tests ─────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use mock::MockWayland;

    #[tokio::test]
    async fn mock_read_returns_seeded_primary() {
        let w = MockWayland::new();
        w.set_primary(Some("hello"));
        assert_eq!(w.read(ClipboardKind::Primary).await.unwrap().as_deref(), Some("hello"));
    }

    #[tokio::test]
    async fn mock_write_then_read_regular() {
        let w = MockWayland::new();
        w.write_regular("paraphrased").await.unwrap();
        assert_eq!(w.read(ClipboardKind::Regular).await.unwrap().as_deref(), Some("paraphrased"));
    }

    #[tokio::test]
    async fn mock_records_combos() {
        let w = MockWayland::new();
        w.type_combo("ctrl+v").await.unwrap();
        assert_eq!(w.combos(), vec!["ctrl+v"]);
    }
}
