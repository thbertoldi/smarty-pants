# Changelog

## Unreleased

- Pants-with-glasses tray mascot, embedded at seven panel sizes with a transparent background, and a matching application launcher icon.

## 0.2.0 — 2026-09-06

- Direct DeepSeek and compatible APIs, optional API-only builds, and smaller embedded local presets. Qwen 2.5 1.5B is the local default; no Ollama dependency.
- Native tray controls for providers, local/API models, credentials, resources, pause, and configuration reload.
- Copy-only and review-before-copy delivery. Automatic paste verifies the captured window on Hyprland, Sway, and niri and copies when focus changes or is unknown.
- Support current Hyprland Lua shortcut dispatch with a fallback for older compositors.
- Reject stale clipboard fallback captures; recognize terminal Copy/Paste combinations; restore the clipboard from before capture when requested.
- Literal-preservation checks, bounded selections and API responses, lazy model loading and idle unloading, transactional settings, and clean socket shutdown.
- Detached startup reports readiness and provides an error log. Compatible APIs may return null tool_calls with a normal text response.
- CPU/API Linux binaries, user installer/uninstaller, application launcher entry, optional systemd service, and an offline RPM/OBS build recipe.
- CI for formatting, Clippy, tests, dependency auditing, docs, and packaging. Correct the minimum Rust version to 1.88.
- Recorded English/Portuguese Qwen/CoEdIT evaluation, rewritten README, and shareable DeepSeek installation instructions.
