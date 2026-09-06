# Verified repository facts

## Model and provider integration

- Local inference is embedded through `llama-cpp-2`; the repository has no Ollama runtime requirement.
- The daemon's configured local preset is selected in `backend.rs`; its default is Qwen 2.5 1.5B Instruct Q4_K_M.
- Local weights load on demand. Idle unloading defaults to 300 seconds, and the tray/CLI can unload explicitly.
- API providers are constructed without initializing llama.cpp or downloading local weights.
- A build using `--no-default-features --features tray` excludes the optional llama.cpp dependency.
- DeepSeek requests use chat-completions messages and explicitly disable thinking. Credentials come from an environment variable or a separate file.
- A 12-case synthetic English/Portuguese run is recorded under `docs/evaluations/`; it is not a hallucination benchmark.
- A separate 24-case CoEdIT/Qwen comparison records all 48 generations under `docs/evaluations/2026-09-06-coedit-vs-qwen/`, with exact prompts, model/package versions, file hashes, and qualitative review notes.
- That comparison used greedy CPU inference, four threads, excluded warmups, and an optimized Qwen release build. English median times were 1.52 seconds for CoEdIT FP32 and 5.17 seconds for Qwen Q4_K_M; peak process RAM was 3.29 and 1.86 GiB respectively.
- CoEdIT's tokenizer produced unknown tokens on all 12 Portuguese inputs. The stored outputs show corrupted Portuguese and, in the academic case, a reversal of the sample-size limitation.
- In that run Qwen omitted the English methods sentence and followed embedded poem-writing instructions in both languages. The literal guard blocked the missing numerical methods but allowed the poems.
- CoEdIT is an opt-in Python evaluation helper, not a daemon provider. The experiment did not change the user's installed service or default configuration.
- `examples/deepseek.toml` selects DeepSeek V4 Flash and inherits all four built-in mode prompts. `docs/install-deepseek.md` contains the shareable openSUSE/Hyprland installation and upgrade instructions.
- The installation guide includes a fresh-clone path for another machine and preserves existing user settings, service overrides, and development changes during upgrades.

## Desktop and settings

- The native tray uses StatusNotifierItem and DBusMenu through `ksni`. Zenity is optional and used for API configuration/key dialogs.
- Tray settings and socket requests use the same settings service. Changes validate before atomic persistence, and custom prompts/comments survive tray edits.
- A rewrite, model unload, and settings transaction cannot run simultaneously through the pipeline.
- Shutdown requests cancel the server and the daemon removes its socket. A second daemon cannot unlink an active socket.
- Empty/incomplete output and detected literal changes do not reach the paste step. Oversized selections are rejected before inference.

## Verification on September 6, 2026

- `cargo test --workspace --locked` passed 56 tests, plus documentation tests.
- Daemon tests also passed with `--no-default-features --features tray`.
- `cargo clippy --workspace --all-targets --locked -- -D warnings` passed.
- The comparison evaluator also passed strict Clippy and an optimized CPU build. Both saved 24-case result files reproduced the current Rust assessments exactly; five malformed imports (missing/duplicate/unknown IDs, changed source text, and missing output/error) were rejected without partial output.
- The saved comparison statistics, corpus/config/prompt/package/output hashes, and inclusion of all 48 outputs verbatim in the report were checked successfully.
- An isolated D-Bus smoke check passed for both local and API startup: tray registration/menu export, pause persistence, API configuration, CLI status, invalid reload, and tray shutdown. Neither startup created a model data directory.
- Live DeepSeek calls and visual interaction with the user's desktop panel were not tested. The existing installed systemd service was left running.
- The installation documentation's shell syntax, TOML snippets, and local links passed checks; the packaged unit passed `systemd-analyze --user verify`.
- An API-only workspace build and private D-Bus check verified startup from the DeepSeek example without credentials, CLI status/reload, four inherited modes, tray menu export, and clean shutdown. The check triggered no rewrite or live API call and created no model data directory.
- During the documentation review, the existing installed service was active, the user config file did not exist, Waybar's tray module was enabled, and Hyprland's portal reported no registered global shortcuts. Installation instructions record these as dated observations to recheck.
