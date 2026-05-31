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
#[doc(hidden)]
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

// ── real implementation backed by wl-clipboard-rs + wtype subprocess ──
pub mod real {
    use super::*;
    use std::io::Read;
    use tokio::process::Command;

    pub struct RealWayland;

    impl RealWayland {
        pub fn new() -> Self { Self }
    }

    impl Default for RealWayland {
        fn default() -> Self { Self::new() }
    }

    #[async_trait]
    impl Wayland for RealWayland {
        async fn read(&self, kind: ClipboardKind) -> anyhow::Result<Option<String>> {
            use wl_clipboard_rs::paste::{
                get_contents, ClipboardType, Error, MimeType, Seat,
            };
            let target = match kind {
                ClipboardKind::Primary => ClipboardType::Primary,
                ClipboardKind::Regular => ClipboardType::Regular,
            };
            // wl-clipboard-rs is sync — run on blocking pool.
            let result = tokio::task::spawn_blocking(move || {
                match get_contents(target, Seat::Unspecified, MimeType::Text) {
                    Ok((mut pipe, _)) => {
                        let mut buf = String::new();
                        pipe.read_to_string(&mut buf).map_err(|e| {
                            anyhow::anyhow!("read clipboard pipe: {e}")
                        })?;
                        Ok::<Option<String>, anyhow::Error>(Some(buf))
                    }
                    // Treat "no seats" / "empty clipboard" / "no MIME type" as
                    // "selection unavailable" rather than fatal errors.
                    Err(Error::NoSeats)
                    | Err(Error::ClipboardEmpty)
                    | Err(Error::NoMimeType) => Ok(None),
                    Err(e) => Err(anyhow::anyhow!("wl-clipboard: {e}")),
                }
            })
            .await
            .map_err(|e| anyhow::anyhow!("join: {e}"))??;
            Ok(result)
        }

        async fn write_regular(&self, text: &str) -> anyhow::Result<()> {
            use wl_clipboard_rs::copy::{MimeType, Options, Source};
            let text = text.to_owned();
            tokio::task::spawn_blocking(move || {
                let opts = Options::new();
                opts.copy(Source::Bytes(text.into_bytes().into()), MimeType::Text)
                    .map_err(|e| anyhow::anyhow!("wl-copy: {e}"))
            })
            .await
            .map_err(|e| anyhow::anyhow!("join: {e}"))?
        }

        async fn type_combo(&self, combo: &str) -> anyhow::Result<()> {
            // combo formatted as "ctrl+v" or "ctrl+c".
            //
            // On Hyprland we prefer `hyprctl dispatch sendshortcut`, which uses
            // the compositor's own input synthesis pipeline. wtype's
            // virtual-keyboard protocol path silently no-ops on at least some
            // Hyprland versions (observed on Hyprland 0.52.x with wtype 0.4) —
            // the keysym never reaches the focused app. Hyprland's
            // sendshortcut is the same mechanism Hyprland uses for its own
            // `bind = …, sendshortcut, …` declarations, so it's the most
            // reliable synthesis path inside a Hyprland session.
            //
            // Off Hyprland we fall back to wtype's `-M <mod> -k <KEY>` form
            // (matching Handy's invocation in src-tauri/src/clipboard.rs).
            let parts: Vec<&str> = combo.split('+').collect();
            let (mods, key): (Vec<&str>, &str) = match parts.split_last() {
                Some((last, rest)) => (rest.to_vec(), *last),
                None => return Err(anyhow::anyhow!("empty combo")),
            };

            if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some() {
                // Format: `hyprctl dispatch sendshortcut "MODS, KEY,"`
                // MODS is space-separated uppercase ("CTRL", "CTRL SHIFT").
                // Key is the X11 keysym name; uppercase letter is the
                // convention Hyprland uses elsewhere.
                let mods_arg = mods
                    .iter()
                    .map(|m| m.to_ascii_uppercase())
                    .collect::<Vec<_>>()
                    .join(" ");
                let key_arg = key.to_ascii_uppercase();
                let arg = format!("{mods_arg}, {key_arg},");
                tracing::info!(arg = %arg, "synth via hyprctl dispatch sendshortcut");
                let output = Command::new("hyprctl")
                    .args(["dispatch", "sendshortcut", &arg])
                    .output()
                    .await
                    .map_err(|e| anyhow::anyhow!("spawn hyprctl: {e}"))?;
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    return Err(anyhow::anyhow!(
                        "hyprctl dispatch sendshortcut exited {} stderr={stderr}",
                        output.status
                    ));
                }
                // hyprctl prints "ok" on success and a non-"ok" body on
                // failure even when exit status is 0 — defensive check.
                let stdout = String::from_utf8_lossy(&output.stdout);
                if !stdout.trim().starts_with("ok") {
                    return Err(anyhow::anyhow!(
                        "hyprctl dispatch sendshortcut returned: {}",
                        stdout.trim()
                    ));
                }
                return Ok(());
            }

            // Non-Hyprland fallback: wtype with `-M MOD -k KEY` form.
            tracing::info!(combo = %combo, "synth via wtype");
            let mut cmd = Command::new("wtype");
            for m in &mods {
                cmd.arg("-M").arg(m);
            }
            cmd.arg("-k").arg(key);
            let output = cmd.output().await
                .map_err(|e| anyhow::anyhow!("spawn wtype: {e}"))?;
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(anyhow::anyhow!(
                    "wtype exited {} stderr={stderr}", output.status
                ));
            }
            Ok(())
        }
    }
}
