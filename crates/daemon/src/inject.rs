//! Write the paraphrased text back: save clipboard → set → paste → restore.
//!
//! Inject owns the full save/restore cycle for the regular clipboard.
//! `selection::capture` does NOT thread the prior clipboard through —
//! whatever is on the clipboard at the moment write() is called is what
//! we restore. This is correct whether capture went via PRIMARY (in which
//! case the user's clipboard is untouched) or via Ctrl+C (in which case
//! the user's clipboard already holds the captured selection).
//!
//! The paste keysym depends on the focused app: most GUI apps use Ctrl+V,
//! but terminals (ghostty, kitty, foot, alacritty, wezterm, gnome-terminal,
//! konsole, xterm, …) use Ctrl+Shift+V because Ctrl+V is reserved for
//! `vim`-style "insert literal next character". We auto-detect via
//! `hyprctl activewindow -j` and pick the right combo.

use crate::wayland::{ClipboardKind, Wayland};
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command;

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
    let combo = paste_combo_for_active_window().await;
    tracing::info!(combo = %combo, "paste combo selected");
    wl.type_combo(&combo).await?;
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

/// Returns "ctrl+shift+v" for known terminal apps, "ctrl+v" otherwise.
/// Falls back to "ctrl+v" if we can't determine the focused window class
/// (non-Hyprland session, hyprctl missing, JSON parse error, etc).
async fn paste_combo_for_active_window() -> String {
    match active_window_class().await {
        Some(class) if is_terminal_class(&class) => "ctrl+shift+v".into(),
        _ => "ctrl+v".into(),
    }
}

async fn active_window_class() -> Option<String> {
    std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE")?;
    let out = Command::new("hyprctl")
        .args(["activewindow", "-j"])
        .output()
        .await
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    v.get("class")?.as_str().map(|s| s.to_owned())
}

/// Known terminal window classes that paste with Ctrl+Shift+V.
fn is_terminal_class(class: &str) -> bool {
    const TERMINALS: &[&str] = &[
        "com.mitchellh.ghostty",
        "kitty",
        "foot",
        "Alacritty",
        "alacritty",
        "org.wezfurlong.wezterm",
        "wezterm",
        "gnome-terminal-server",
        "konsole",
        "org.kde.konsole",
        "xterm",
        "URxvt",
        "xfce4-terminal",
        "tilix.Tilix",
        "io.elementary.terminal",
    ];
    TERMINALS.iter().any(|t| class.eq_ignore_ascii_case(t))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wayland::mock::MockWayland;

    /// True if the recorded combos contain a single paste keysym
    /// (either Ctrl+V or Ctrl+Shift+V — depends on active-window detection).
    fn sent_one_paste(combos: &[String]) -> bool {
        combos.len() == 1 && (combos[0] == "ctrl+v" || combos[0] == "ctrl+shift+v")
    }

    #[tokio::test]
    async fn writes_then_pastes() {
        let w = Arc::new(MockWayland::new());
        write(w.clone(), "paraphrased", 0, false).await.unwrap();
        assert_eq!(
            w.read(ClipboardKind::Regular).await.unwrap().as_deref(),
            Some("paraphrased")
        );
        assert!(sent_one_paste(&w.combos()), "got {:?}", w.combos());
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
