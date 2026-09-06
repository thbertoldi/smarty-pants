//! Abstraction over Wayland clipboard + keystroke synthesis.
//!
//! Unit tests use `MockWayland` without accessing the desktop clipboard.

use async_trait::async_trait;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ClipboardKind {
    Primary,
    Regular,
}

#[async_trait]
#[allow(dead_code)]
pub trait Wayland: Send + Sync + 'static {
    async fn read(&self, kind: ClipboardKind) -> anyhow::Result<Option<String>>;
    async fn write_regular(&self, text: &str) -> anyhow::Result<()>;
    /// Synthesize a single key combo like "ctrl+c" or "ctrl+v".
    async fn type_combo(&self, combo: &str) -> anyhow::Result<()>;
    async fn focused_window(&self) -> Option<crate::focus::Window>;
    async fn review(&self, original: &str, generated: &str) -> anyhow::Result<bool>;
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
        pub primary: Mutex<Option<String>>,
        pub regular: Mutex<Option<String>>,
        pub combos: Mutex<Vec<String>>,
        /// If set, a Ctrl+C combo causes `primary` to be copied into `regular`
        /// so the selection capture loop sees something.
        pub ctrl_c_copies_primary_into_regular: bool,
        pub focus: Mutex<Option<crate::focus::Window>>,
        pub review_accepted: bool,
        pub reviews: Mutex<Vec<(String, String)>>,
    }

    impl MockWayland {
        pub fn new() -> Self {
            Self::default()
        }

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
            if matches!(combo, "ctrl+c" | "ctrl+shift+c") && self.ctrl_c_copies_primary_into_regular
            {
                let p = self.primary.lock().unwrap().clone();
                if let Some(p) = p {
                    *self.regular.lock().unwrap() = Some(p);
                }
            }
            Ok(())
        }

        async fn focused_window(&self) -> Option<crate::focus::Window> {
            self.focus.lock().unwrap().clone()
        }

        async fn review(&self, original: &str, generated: &str) -> anyhow::Result<bool> {
            self.reviews
                .lock()
                .unwrap()
                .push((original.into(), generated.into()));
            Ok(self.review_accepted)
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
        assert_eq!(
            w.read(ClipboardKind::Primary).await.unwrap().as_deref(),
            Some("hello")
        );
    }

    #[tokio::test]
    async fn mock_write_then_read_regular() {
        let w = MockWayland::new();
        w.write_regular("paraphrased").await.unwrap();
        assert_eq!(
            w.read(ClipboardKind::Regular).await.unwrap().as_deref(),
            Some("paraphrased")
        );
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

    async fn hyprland_shortcut(
        program: &std::ffi::OsStr,
        mods: &str,
        key: &str,
    ) -> anyhow::Result<()> {
        let lua = format!(
            "hl.dsp.send_shortcut({{mods={},key={}}})",
            serde_json::json!(mods),
            serde_json::json!(key),
        );
        let mut output = Command::new(program)
            .args(["dispatch", &lua])
            .output()
            .await
            .map_err(|e| anyhow::anyhow!("spawn hyprctl: {e}"))?;

        // Older compositors reject the Lua dispatcher before sending any keys.
        // Retry only that rejection: other errors must not risk a second paste.
        if String::from_utf8_lossy(&output.stdout)
            .trim()
            .eq_ignore_ascii_case("invalid dispatcher")
        {
            let legacy = format!("{mods}, {key},");
            output = Command::new(program)
                .args(["dispatch", "sendshortcut", &legacy])
                .output()
                .await
                .map_err(|e| anyhow::anyhow!("spawn hyprctl: {e}"))?;
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        anyhow::ensure!(
            output.status.success() && stdout.trim() == "ok",
            "Hyprland shortcut failed: {} {}",
            stdout.trim(),
            String::from_utf8_lossy(&output.stderr).trim(),
        );
        Ok(())
    }

    pub struct RealWayland;

    impl RealWayland {
        pub fn new() -> Self {
            Self
        }
    }

    impl Default for RealWayland {
        fn default() -> Self {
            Self::new()
        }
    }

    #[async_trait]
    impl Wayland for RealWayland {
        async fn focused_window(&self) -> Option<crate::focus::Window> {
            crate::focus::current().await
        }

        async fn review(&self, original: &str, generated: &str) -> anyhow::Result<bool> {
            crate::review::show(original, generated).await
        }

        async fn read(&self, kind: ClipboardKind) -> anyhow::Result<Option<String>> {
            use wl_clipboard_rs::paste::{get_contents, ClipboardType, Error, MimeType, Seat};
            let target = match kind {
                ClipboardKind::Primary => ClipboardType::Primary,
                ClipboardKind::Regular => ClipboardType::Regular,
            };
            // wl-clipboard-rs is sync — run on blocking pool.
            let result = tokio::task::spawn_blocking(move || {
                match get_contents(target, Seat::Unspecified, MimeType::Text) {
                    Ok((pipe, _)) => {
                        let mut buf = String::new();
                        // Bound allocations even before capture.max_chars is checked.
                        const MAX_BYTES: u64 = 4 * 1024 * 1024;
                        pipe.take(MAX_BYTES + 1)
                            .read_to_string(&mut buf)
                            .map_err(|e| anyhow::anyhow!("read clipboard pipe: {e}"))?;
                        anyhow::ensure!(
                            buf.len() as u64 <= MAX_BYTES,
                            "clipboard text exceeds 4 MiB"
                        );
                        Ok::<Option<String>, anyhow::Error>(Some(buf))
                    }
                    // Treat "no seats" / "empty clipboard" / "no MIME type" as
                    // "selection unavailable" rather than fatal errors.
                    Err(Error::NoSeats) | Err(Error::ClipboardEmpty) | Err(Error::NoMimeType) => {
                        Ok(None)
                    }
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
            // On Hyprland we prefer the native shortcut dispatcher, which uses
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
                // Lua Hyprland uses hl.dsp.send_shortcut({mods, key}); older
                // versions use `hyprctl dispatch sendshortcut "MODS, KEY,"`.
                // MODS is space-separated uppercase ("CTRL", "CTRL SHIFT").
                // Key is the X11 keysym name; uppercase letter is the
                // convention Hyprland uses elsewhere.
                let mods_arg = mods
                    .iter()
                    .map(|m| m.to_ascii_uppercase())
                    .collect::<Vec<_>>()
                    .join(" ");
                let key_arg = key.to_ascii_uppercase();
                tracing::info!(combo = %combo, "synth via Hyprland shortcut dispatcher");
                return hyprland_shortcut(std::ffi::OsStr::new("hyprctl"), &mods_arg, &key_arg)
                    .await;
            }

            // Non-Hyprland fallback: wtype with `-M MOD -k KEY` form.
            tracing::info!(combo = %combo, "synth via wtype");
            let mut cmd = Command::new("wtype");
            for m in &mods {
                cmd.arg("-M").arg(m);
            }
            cmd.arg("-k").arg(key);
            let output = cmd
                .output()
                .await
                .map_err(|e| anyhow::anyhow!("spawn wtype: {e}"))?;
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(anyhow::anyhow!(
                    "wtype exited {} stderr={stderr}",
                    output.status
                ));
            }
            Ok(())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::os::unix::fs::PermissionsExt;

        fn fake_hyprctl(reply: &str, code: u8) -> tempfile::TempDir {
            let dir = tempfile::tempdir().unwrap();
            let program = dir.path().join("hyprctl");
            std::fs::write(
                &program,
                format!(
                    "#!/bin/sh\ncd -- \"$(dirname -- \"$0\")\"\nprintf '%s\\n' \"$*\" >> calls\nif [ \"$2\" = sendshortcut ]; then printf 'ok\\n'; exit 0; fi\nprintf '%s\\n' '{reply}'\nexit {code}\n"
                ),
            )
            .unwrap();
            std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
            dir
        }

        #[tokio::test]
        async fn lua_success_sends_only_one_shortcut() {
            let dir = fake_hyprctl("ok", 0);
            hyprland_shortcut(dir.path().join("hyprctl").as_os_str(), "CTRL SHIFT", "V")
                .await
                .unwrap();
            let calls = std::fs::read_to_string(dir.path().join("calls")).unwrap();
            assert_eq!(calls.lines().count(), 1);
            assert!(calls.contains("hl.dsp.send_shortcut"));
        }

        #[tokio::test]
        async fn unknown_dispatcher_retries_with_the_legacy_protocol() {
            let dir = fake_hyprctl("Invalid dispatcher", 1);
            hyprland_shortcut(dir.path().join("hyprctl").as_os_str(), "CTRL", "C")
                .await
                .unwrap();
            let calls = std::fs::read_to_string(dir.path().join("calls")).unwrap();
            assert_eq!(calls.lines().count(), 2);
            assert_eq!(calls.lines().last(), Some("dispatch sendshortcut CTRL, C,"));
        }

        #[tokio::test]
        async fn an_action_error_never_retries_even_with_a_success_exit_code() {
            let dir = fake_hyprctl("window not found", 0);
            assert!(
                hyprland_shortcut(dir.path().join("hyprctl").as_os_str(), "CTRL", "V")
                    .await
                    .is_err()
            );
            let calls = std::fs::read_to_string(dir.path().join("calls")).unwrap();
            assert_eq!(calls.lines().count(), 1);
        }
    }
}
