//! Capture PRIMARY, or a newly copied selection. Never rewrite an old clipboard.

use crate::{
    focus::Window,
    wayland::{ClipboardKind, Wayland},
};
use std::{sync::Arc, time::Duration};

pub struct Captured {
    pub text: String,
    pub window: Option<Window>,
    pub prior_clipboard: Option<String>,
}

pub async fn capture(
    wl: Arc<dyn Wayland>,
    prefer_primary: bool,
    ctrl_c_settle_ms: u64,
    max_chars: usize,
) -> anyhow::Result<Option<Captured>> {
    let window = wl.focused_window().await;
    let prior_clipboard = wl.read(ClipboardKind::Regular).await?;
    if prefer_primary {
        if let Some(s) = wl.read(ClipboardKind::Primary).await? {
            let text = trim_and_validate(s, max_chars)?;
            if !text.is_empty() {
                return Ok(Some(Captured {
                    text,
                    window,
                    prior_clipboard,
                }));
            }
        }
    }
    // Unknown terminals may interpret Ctrl+C as Interrupt instead of Copy.
    if window.is_none() {
        return Ok(None);
    }
    // A focus change during capture must not send Copy to a different window.
    anyhow::ensure!(
        window == wl.focused_window().await,
        "focus changed while capturing; select the text and retry"
    );
    wl.type_combo(window.as_ref().map(Window::copy_combo).unwrap_or("ctrl+c"))
        .await?;
    tokio::time::sleep(Duration::from_millis(ctrl_c_settle_ms)).await;
    let after = wl.read(ClipboardKind::Regular).await?;
    // The application may ignore Copy. Identical text is ambiguous: fail closed.
    if after == prior_clipboard {
        return Ok(None);
    }
    let captured = after
        .map(|s| trim_and_validate(s, max_chars))
        .transpose()?
        .filter(|s| !s.is_empty());
    Ok(captured.map(|text| Captured {
        text,
        window,
        prior_clipboard,
    }))
}

fn trim_and_validate(s: String, max_chars: usize) -> anyhow::Result<String> {
    let text = s.trim();
    anyhow::ensure!(
        text.chars().count() <= max_chars,
        "selection exceeds capture.max_chars ({max_chars}); select less text or raise the limit"
    );
    Ok(text.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wayland::mock::MockWayland;

    #[tokio::test]
    async fn primary_preserves_original_clipboard() {
        let w = Arc::new(MockWayland::new());
        w.set_primary(Some("from primary"));
        w.set_regular(Some("prior"));
        let result = capture(w.clone(), true, 0, 8000).await.unwrap().unwrap();
        assert_eq!(result.text, "from primary");
        assert_eq!(result.prior_clipboard.as_deref(), Some("prior"));
        assert!(w.combos().is_empty());
    }

    #[tokio::test]
    async fn fallback_captures_changed_text_and_preserves_original_clipboard() {
        let w = Arc::new(MockWayland {
            ctrl_c_copies_primary_into_regular: true,
            ..Default::default()
        });
        w.set_primary(Some("highlighted text"));
        w.set_regular(Some("prior"));
        *w.focus.lock().unwrap() = Some(Window {
            id: "sway:1".into(),
            app_id: "kitty".into(),
        });
        let result = capture(w.clone(), false, 0, 8000).await.unwrap().unwrap();
        assert_eq!(result.text, "highlighted text");
        assert_eq!(result.prior_clipboard.as_deref(), Some("prior"));
        assert_eq!(w.combos(), ["ctrl+shift+c"]);
    }

    #[tokio::test]
    async fn ignored_copy_never_rewrites_stale_clipboard() {
        for prior in [None, Some("stale text")] {
            let w = Arc::new(MockWayland::new());
            *w.focus.lock().unwrap() = Some(Window {
                id: "sway:1".into(),
                app_id: "firefox".into(),
            });
            w.set_regular(prior);
            assert!(capture(w, true, 0, 8000).await.unwrap().is_none());
        }
    }

    #[tokio::test]
    async fn rejects_long_selection_instead_of_silently_dropping_its_tail() {
        let w = Arc::new(MockWayland::new());
        w.set_primary(Some(&"x".repeat(10_000)));
        assert!(capture(w, true, 0, 8000).await.is_err());
    }
}
