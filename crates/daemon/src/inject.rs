//! Write the paraphrased text back: save clipboard → set → Ctrl+V → restore.
//!
//! Inject owns the full save/restore cycle for the regular clipboard.
//! `selection::capture` does NOT thread the prior clipboard through —
//! whatever is on the clipboard at the moment write() is called is what
//! we restore. This is correct whether capture went via PRIMARY (in which
//! case the user's clipboard is untouched) or via Ctrl+C (in which case
//! the user's clipboard already holds the captured selection).

use crate::wayland::{ClipboardKind, Wayland};
use std::sync::Arc;
use std::time::Duration;

pub async fn write(
    wl:                Arc<dyn Wayland>,
    generated:         &str,
    paste_settle_ms:   u64,
    restore_clipboard: bool,
) -> anyhow::Result<()> {
    let prior = if restore_clipboard {
        wl.read(ClipboardKind::Regular).await?
    } else {
        None
    };
    wl.write_regular(generated).await?;
    wl.type_combo("ctrl+v").await?;
    tokio::time::sleep(Duration::from_millis(paste_settle_ms)).await;

    if restore_clipboard {
        if let Some(prior) = prior {
            wl.write_regular(&prior).await?;
        }
        // If prior was None, the user had nothing on the clipboard; we leave
        // the generated text on it. This is a deliberate trade-off — the
        // wl-clipboard protocol has no "clear clipboard" primitive that
        // wl-clipboard-rs exposes cleanly, and leaving the latest paste on
        // the clipboard is consistent with normal Ctrl+V behavior.
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wayland::mock::MockWayland;

    #[tokio::test]
    async fn writes_then_pastes() {
        let w = Arc::new(MockWayland::new());
        write(w.clone(), "paraphrased", 0, false).await.unwrap();
        assert_eq!(
            w.read(ClipboardKind::Regular).await.unwrap().as_deref(),
            Some("paraphrased")
        );
        assert_eq!(w.combos(), vec!["ctrl+v"]);
    }

    #[tokio::test]
    async fn restores_prior_clipboard_when_enabled() {
        let w = Arc::new(MockWayland::new());
        w.write_regular("ORIGINAL").await.unwrap();
        write(w.clone(), "paraphrased", 0, true).await.unwrap();
        assert_eq!(
            w.read(ClipboardKind::Regular).await.unwrap().as_deref(),
            Some("ORIGINAL")
        );
    }

    #[tokio::test]
    async fn does_not_restore_when_disabled() {
        let w = Arc::new(MockWayland::new());
        w.write_regular("ORIGINAL").await.unwrap();
        write(w.clone(), "paraphrased", 0, false).await.unwrap();
        assert_eq!(
            w.read(ClipboardKind::Regular).await.unwrap().as_deref(),
            Some("paraphrased")
        );
    }
}
