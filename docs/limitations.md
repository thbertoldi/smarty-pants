# Limitations and workarounds

## Addressed in 0.2.0

| Previous behavior | Current behavior |
| --- | --- |
| A rewrite could paste into a different window after a long generation. | The captured window identity is checked before automatic paste on Hyprland, Sway, and niri. Changed or unavailable focus falls back to copying. |
| An ignored Copy command could rewrite old clipboard text. | The fallback must observe changed clipboard text. Otherwise no generation runs. Terminal Copy uses Ctrl+Shift+C when the app is recognized. |
| Clipboard restoration after a fallback capture restored the selection, losing the earlier clipboard. | Successful automatic paste restores the clipboard saved before capture when restoration is enabled. |
| Terminal TUIs could swallow synthetic paste. | **Copy only** leaves the result ready for an explicit paste. The app cannot detect every swallowed keystroke. |
| Users needed to review model mistakes outside the app. | **Review, then copy…** shows both texts, with explicit Copy rewrite and Discard buttons. It requires Zenity. |
| API model selection was buried in connection settings. | **API model** offers DeepSeek Flash/Pro and a prefilled custom model field. URL and credentials are preserved. |
| Detached startup always reported success and hid errors. | Startup waits for a responsive daemon or reports a private log path. Launching again reuses the running daemon. |
| Installing required a Rust and C++ toolchain. | Versioned CPU and API binaries include a user installer and desktop entry. |

## Still applies

**Models can change meaning, omit facts, obey instructions embedded in selected text, or refuse requests.** Literal checks cover sets of numbers, links, email addresses, and backtick code. They do not validate names, negation, repeated quantities, associations between values, or overall meaning. The [English/Portuguese comparison](evaluations/2026-09-06-coedit-vs-qwen/report.md) includes actual failures. Use review mode for important text. CoEdIT is not suitable for our Portuguese requirement in that experiment.

**Literal checks are deliberately strict.** A correct change from `3` to `three`, for example, can be rejected. `[writing] preserve_literals = false` disables this guard; review becomes especially important. Empty, truncated, refused API responses and tool calls remain rejected regardless of this setting.

**A window check is not a document lock.** Changing the selection, tab, document, or cursor inside the same window cannot be detected. There is also a short interval between checking focus and sending the key. Keep the selection in place during automatic paste, or use copy/review delivery. PRIMARY can contain a selection from another application on some desktops; set `[capture] prefer_primary = false` to request a fresh Copy instead. Identical clipboard text cannot prove Copy succeeded, so the fallback conservatively returns no selection in that case.

**Desktop coverage is limited.** Hyprland is the primary development environment. Sway and niri focus parsers have fixture tests; their complete desktop workflows still need manual testing. Compositors must support the clipboard protocols used by wl-clipboard. GNOME/KDE support is not established. If the focused window cannot be identified, automatic paste falls back to copying and the synthetic Copy fallback is skipped to avoid interrupting an unknown terminal. Clipboard reads are capped at 4 MiB, but an unresponsive clipboard owner can still stall a read.

**Clipboard timing varies by application.** Automatic replacement in read-only fields is not possible. Some terminal programs append a paste or ignore it. Clipboard restoration is off by default, applies only after automatic paste, and cannot prove an application consumed the rewrite. If there was no prior text clipboard, the rewrite stays on it. Review cancellation leaves the clipboard as it was immediately before the dialog (a fallback Copy may already have changed it).

**Local startup still takes time.** The first rewrite downloads missing weights and loads them. Idle unloading defaults to five minutes. Keep model loaded avoids subsequent weight reloads, but llama.cpp still creates a context per request. Very large selections can exceed a model's token context even below the character limit. A busy rewrite rejects another request; there is no queue or cancellation UI.

**Release archive scope is Linux x86_64, glibc 2.35+.** The CPU bundle includes local inference and APIs; the smaller API bundle cannot switch to local inference. A separate [personal OBS repository](install.md#opensuse-tumbleweed-rpm) provides the CPU/API/tray RPM for current openSUSE Tumbleweed x86_64; the archives' glibc baseline does not describe that distro-built RPM. Vulkan/CUDA/ROCm require source builds. No Flatpak or container desktop image is provided. Archive SHA-256 files detect corruption; they are not independent publisher signatures.

**API behavior depends on the provider.** There are no automatic retries or silent fallback providers. Credentials are private files or environment variables, not a desktop keyring. Text goes to the configured remote API when requested. CI uses a local mock API; it does not spend money on live DeepSeek calls.
