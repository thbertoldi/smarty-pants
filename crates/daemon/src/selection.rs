//! Capture the user's currently selected text:
//!   1. read PRIMARY
//!   2. if empty, synth Ctrl+C, sleep, read regular
//!
//! Inject (see inject.rs in Task 7) owns the save/restore of the regular
//! clipboard — we do not thread a "prior clipboard" value through Captured.

use crate::wayland::{ClipboardKind, Wayland};
use std::sync::Arc;
use std::time::Duration;

pub struct Captured {
    pub text: String,
}

pub async fn capture(
    wl:                Arc<dyn Wayland>,
    prefer_primary:    bool,
    ctrl_c_settle_ms:  u64,
    max_chars:         usize,
) -> anyhow::Result<Option<Captured>> {
    if prefer_primary {
        if let Some(s) = wl.read(ClipboardKind::Primary).await? {
            let s = trim_and_cap(s, max_chars);
            if !s.is_empty() {
                return Ok(Some(Captured { text: s }));
            }
        }
    }
    // Fall back to synthesize Ctrl+C.
    wl.type_combo("ctrl+c").await?;
    tokio::time::sleep(Duration::from_millis(ctrl_c_settle_ms)).await;
    let after = wl.read(ClipboardKind::Regular).await?;
    let captured = after.and_then(|s| {
        let s = trim_and_cap(s, max_chars);
        (!s.is_empty()).then_some(s)
    });
    Ok(captured.map(|text| Captured { text }))
}

fn trim_and_cap(mut s: String, max_chars: usize) -> String {
    let trimmed = s.trim();
    if trimmed.len() != s.len() {
        s = trimmed.to_owned();
    }
    if s.chars().count() > max_chars {
        s = s.chars().take(max_chars).collect();
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wayland::mock::MockWayland;

    fn arc(w: MockWayland) -> Arc<dyn Wayland> { Arc::new(w) }

    #[tokio::test]
    async fn returns_primary_when_present() {
        let w = MockWayland::new();
        w.set_primary(Some("from primary"));
        let result = capture(arc(w), true, 0, 8000).await.unwrap().unwrap();
        assert_eq!(result.text, "from primary");
    }

    #[tokio::test]
    async fn falls_back_to_ctrl_c_when_primary_empty() {
        // We simulate the user having highlighted text in an app that
        // doesn't expose PRIMARY but does respond to Ctrl+C. We pre-load
        // the mock's regular clipboard with the "selection result" the
        // app would write after Ctrl+C.
        let w = MockWayland::new();
        w.set_primary(None);
        w.set_regular(Some("highlighted text"));
        let arc_w = Arc::new(w);
        let result = capture(arc_w.clone(), true, 0, 8000).await.unwrap().unwrap();
        assert_eq!(result.text, "highlighted text");
        assert!(arc_w.combos().contains(&"ctrl+c".to_owned()));
    }

    #[tokio::test]
    async fn returns_none_when_nothing_selected() {
        let w = MockWayland::new();
        w.set_primary(None);
        w.set_regular(None);
        let result = capture(arc(w), true, 0, 8000).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn trims_and_caps_long_selection() {
        let w = MockWayland::new();
        let big = "x".repeat(10_000);
        w.set_primary(Some(&big));
        let result = capture(arc(w), true, 0, 8000).await.unwrap().unwrap();
        assert_eq!(result.text.chars().count(), 8000);
    }
}
