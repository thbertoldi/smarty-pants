//! Deliver a rewrite according to the user's choice and the captured window.

use crate::{selection::Captured, wayland::Wayland};
use smarty_pants_core::config::{Delivery, InjectCfg};
use std::{sync::Arc, time::Duration};

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Pasted,
    Copied(&'static str),
    Cancelled,
}

pub async fn write(
    wl: Arc<dyn Wayland>,
    captured: &Captured,
    generated: &str,
    cfg: &InjectCfg,
) -> anyhow::Result<Outcome> {
    if cfg.delivery == Delivery::Review && !wl.review(&captured.text, generated).await? {
        return Ok(Outcome::Cancelled);
    }
    wl.write_regular(generated).await?;
    match cfg.delivery {
        Delivery::Copy => return Ok(Outcome::Copied("Copy only is selected; paste when ready")),
        Delivery::Review => return Ok(Outcome::Copied("Reviewed; paste when ready")),
        Delivery::Paste => {}
    }
    let Some(window) = &captured.window else {
        return Ok(Outcome::Copied(
            "Cannot verify the focused window; paste manually",
        ));
    };
    if wl.focused_window().await.as_ref() != Some(window) {
        return Ok(Outcome::Copied(
            "Focus changed; paste in the intended window",
        ));
    }
    // This narrows the focus race; it cannot detect a changed selection inside a window.
    if wl.type_combo(window.paste_combo()).await.is_err() {
        return Ok(Outcome::Copied("Automatic paste failed; paste manually"));
    }
    tokio::time::sleep(Duration::from_millis(cfg.paste_settle_ms)).await;
    if cfg.restore_clipboard {
        if let Some(prior) = &captured.prior_clipboard {
            wl.write_regular(prior).await?;
        }
    }
    Ok(Outcome::Pasted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        focus::Window,
        wayland::{mock::MockWayland, ClipboardKind},
    };

    fn selection() -> Captured {
        Captured {
            text: "original".into(),
            window: Some(Window {
                id: "sway:1".into(),
                app_id: "kitty".into(),
            }),
            prior_clipboard: Some("before capture".into()),
        }
    }

    #[tokio::test]
    async fn restores_clipboard_from_before_capture_and_uses_terminal_paste() {
        let w = Arc::new(MockWayland::new());
        let captured = selection();
        *w.focus.lock().unwrap() = captured.window.clone();
        w.set_regular(Some("original"));
        let cfg = InjectCfg {
            restore_clipboard: true,
            paste_settle_ms: 0,
            ..Default::default()
        };
        assert_eq!(
            write(w.clone(), &captured, "rewrite", &cfg).await.unwrap(),
            Outcome::Pasted
        );
        assert_eq!(w.combos(), ["ctrl+shift+v"]);
        assert_eq!(
            w.read(ClipboardKind::Regular).await.unwrap().as_deref(),
            Some("before capture")
        );
    }

    #[tokio::test]
    async fn changed_or_unknown_focus_copies_without_pasting_or_restoring() {
        for focus in [
            None,
            Some(Window {
                id: "sway:2".into(),
                app_id: "kitty".into(),
            }),
        ] {
            let w = Arc::new(MockWayland::new());
            *w.focus.lock().unwrap() = focus;
            let cfg = InjectCfg {
                restore_clipboard: true,
                ..Default::default()
            };
            assert!(matches!(
                write(w.clone(), &selection(), "rewrite", &cfg)
                    .await
                    .unwrap(),
                Outcome::Copied(_)
            ));
            assert!(w.combos().is_empty());
            assert_eq!(
                w.read(ClipboardKind::Regular).await.unwrap().as_deref(),
                Some("rewrite")
            );
        }
    }

    #[tokio::test]
    async fn copy_mode_never_pastes_even_when_the_original_window_is_focused() {
        let w = Arc::new(MockWayland::new());
        *w.focus.lock().unwrap() = selection().window;
        let cfg = InjectCfg {
            delivery: Delivery::Copy,
            restore_clipboard: true,
            ..Default::default()
        };
        assert!(matches!(
            write(w.clone(), &selection(), "rewrite", &cfg)
                .await
                .unwrap(),
            Outcome::Copied(_)
        ));
        assert!(w.combos().is_empty());
        assert_eq!(
            w.read(ClipboardKind::Regular).await.unwrap().as_deref(),
            Some("rewrite")
        );
    }

    #[tokio::test]
    async fn review_requires_explicit_acceptance_and_never_sends_keys() {
        for accepted in [false, true] {
            let w = Arc::new(MockWayland {
                review_accepted: accepted,
                ..Default::default()
            });
            w.set_regular(Some("prior"));
            let cfg = InjectCfg {
                delivery: Delivery::Review,
                ..Default::default()
            };
            let outcome = write(w.clone(), &selection(), "rewrite", &cfg)
                .await
                .unwrap();
            assert_eq!(outcome == Outcome::Cancelled, !accepted);
            assert_eq!(
                w.read(ClipboardKind::Regular).await.unwrap().as_deref(),
                Some(if accepted { "rewrite" } else { "prior" })
            );
            assert!(w.combos().is_empty());
            assert_eq!(
                *w.reviews.lock().unwrap(),
                [("original".into(), "rewrite".into())]
            );
        }
    }
}
