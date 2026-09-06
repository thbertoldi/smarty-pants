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

## Installation and delivery review (0.2.0)

- Delivery settings now support automatic paste, copy only, and an explicit Zenity review/copy dialog. Review text travels over stdin rather than process arguments.
- Automatic paste compares the captured window identity with the current identity; unknown or changed focus leaves the generated text on the clipboard. Focus parsing supports Hyprland addresses, Sway container IDs and niri window IDs.
- Synthetic Copy must change the regular clipboard before it is accepted as a selection. Unknown window identities skip this fallback. Recognized terminals use Ctrl+Shift+C and Ctrl+Shift+V.
- Optional restoration after automatic paste uses the clipboard saved before capture. Copy and review delivery retain the generated text for manual paste.
- The tray has a dedicated API model menu. Model-only changes preserve endpoint and credential settings and reject blank model IDs.
- Detached CLI startup waits for daemon readiness, recognizes an already running daemon, and reports startup failures with an XDG state log path.
- Clipboard text reads are capped at 4 MiB before the selection character check.
- The post-update Cargo audit reported no vulnerabilities and no warnings using cargo-audit 0.22.2 and RustSec database commit 5a0ebedfe8bdd2e295b171f4162f8c977bcad9a5. Updated packages include wayland-scanner/quick-xml, quinn-proto, anyhow and event-listener.
- Local strict Clippy and API/default-feature tests passed. Isolated API startup/tray and lifecycle checks passed without accessing the desktop clipboard, making live API requests, or downloading models.
- A bundle made with local API binaries passed fresh install, paths-with-spaces, upgrade preservation and uninstall checks in temporary XDG directories. The RPM spec parses and documentation links, shell scripts and desktop metadata pass their checks.
- llama.cpp's CMake configuration enables optional x86 instructions by default when GGML_NATIVE is off unless SOURCE_DATE_EPOCH is set. Release/RPM builds set that environment variable and verify the resulting CMake flags alongside a generic Rust x86_64 target.
- GitHub release automation and an offline RPM/OBS source recipe are included. OBS publication and full Sway/niri desktop testing have not been performed.
- A Hyprland shortcut compatibility change was present concurrently in the working tree. Review against the current Hyprland dispatcher documentation and 64 API-build tests passed: it tries the Lua dispatcher and only falls back to legacy syntax on an explicit invalid-dispatcher rejection.
- GitHub's Rust 1.88 API check passed. The first CI run exposed a missing Ubuntu 22.04 glslc package, and the next exposed commas in cache keys; CI now uses Ubuntu 24.04 and named flavor cache keys. Downloadable binary builds remain on Ubuntu 22.04.

## Verified desktop deployment on September 6, 2026

- The installed CLI and daemon use the committed DeepSeek/tray update `923a1eb`; the daemon additionally includes the Lua-compatible Hyprland shortcut patch in `crates/daemon/src/wayland.rs`.
- The daemon was built with `--locked --no-default-features --features tray`. The existing user systemd unit remains enabled and active, pointing to `~/.cargo/bin/smarty-pants-daemon`.
- This deployment uses DeepSeek V4 Flash with all four built-in modes, unpaused and without a resident local model. The user entered the API key through the native tray dialog; it is stored outside the repository in a file with mode `0600`.
- The portal rejected shortcut registration with `An app id is required`. The desktop's Lua bindings now invoke the CLI directly for Super+R/A/I/C, and its user config disables the unused portal shortcut session.
- Hyprland 0.56 accepts `hl.dsp.send_shortcut({mods, key})`. The compatibility patch uses that API and retries the old dispatcher only after an explicit `Invalid dispatcher` rejection, avoiding duplicate input on other failures.
- The patch is present in both the isolated deployment worktree and the newer development working tree. The ongoing 0.2.0 changes were not included in this installed build.
- The deployed source passed 59 tests and strict Clippy with the API-only/tray feature set. The added checks cover successful Lua dispatch, legacy rejection/fallback, and action errors that must not be retried.
- Two live tests submitted synthetic English and Portuguese PRIMARY selections through the installed CLI and DeepSeek, then verified replacement in a disposable GTK editor via native Hyprland paste. Both preserved names, negation and language while correcting grammar; end-to-end times were 1.30 and 1.72 seconds. These are smoke checks, not a writing-quality benchmark.
- The four Lua bindings loaded successfully, but physical hotkey presses were not part of those two live checks. The temporary editor was removed; original window focus, pointer position and clipboard-restoration preference were restored after verification.
- The 0.2.0 RPM recipe built a binary/source RPM with vendored sources, a fresh CARGO_HOME, and Cargo --frozen. Its 61 tests, baseline CMake CPU-flag check, desktop-file validation, and extracted CLI/daemon lifecycle checks passed. The local build used rustup and rpmbuild --nodeps; OBS BuildRequires resolution and clean-machine installation are still unverified.

## Published 0.2.0 verification

- Release v0.2.0 was published from commit fc43fc35a3963493dbdadb34b52e13925afb9778 at https://github.com/thbertoldi/smarty-pants/releases/tag/v0.2.0.
- CI run 34019430631 passed all jobs: Rust 1.88 API, Rust 1.91.1 CPU and Vulkan, formatting, strict Clippy, 64 tests per configuration, documentation/shell/desktop checks, and the dependency audit.
- Release run 34019432615 passed both bundle builds, installation and lifecycle checks, then required the successful CI run for the same commit before publication.
- The final API and CPU archives are approximately 7.8 and 9.2 MiB, excluding separately downloaded model weights. Both contain the release commit in SOURCE and require no glibc symbol newer than 2.34; their documented/tested baseline remains Ubuntu 22.04/glibc 2.35.
- Both final workflow artifacts passed fresh install, upgrade preservation, uninstall, daemon readiness, repeated launch, private logging, clean shutdown, and invalid-config startup checks in temporary XDG directories on the openSUSE development host.
- The published archives and SHA-256 files were downloaded from the GitHub release. Their hashes matched both the checksums and the already-tested workflow artifacts.

## Tray artwork and deployment on September 6, 2026

- The tray now embeds a transparent pants-with-glasses mascot in seven sizes: 16, 20, 22, 24, 32, 48, and 64 pixels. The RGBA exports total 36,560 bytes, convert once to StatusNotifierItem ARGB, and require no new runtime dependency or installed icon theme. Normal and attention states export the same mascot.
- `docs/assets/tray-icon.png` is the generated master; `docs/assets/app-icon.png` is the 256-pixel launcher export used by bundle and RPM packaging. The original GitHub logo is unchanged. `docs/assets/README.md` records the built-in generation tool, reference, prompt and ImageMagick export command.
- Export regeneration reproduced every committed small image byte for byte on the development host. Light/dark panel previews were inspected. A private D-Bus check verified all seven exported sizes, exact ARGB channel order, transparency, empty theme icon names, and matching attention images.
- API/tray formatting, strict Clippy and all 64 existing tests passed. An optimized API build passed lifecycle checks; its bundle contained the matching launcher PNG and passed fresh installation, upgrade preservation and uninstall checks. Shell, desktop metadata, RPM spec parsing and documentation checks passed.
- The optimized API/tray CLI and daemon from the updated 0.2.0 workspace replaced the earlier installed 923a1eb-based build. The user service was restarted while idle; provider/model, pause state, four modes, configuration file and service unit were preserved. Previous binaries are backed up under `~/.local/state/smarty-pants/backups/tray-icon-20260906-131903`.
- The running daemon registered with Waybar and exported the seven mascot pixmaps. A crop of the 32-pixel panel confirmed the new icon was rendered at the configured 16-pixel tray size. This icon verification triggered no rewrite or live API request.

## OBS packaging verification on September 6, 2026

- Created the personal OBS project `home:thbertoldi:smarty-pants`, targeting `openSUSE:Tumbleweed/standard` on x86_64 with debug packages enabled. The checkout is `/home/thbertoldi/obs_builds/home:thbertoldi:smarty-pants/smarty-pants`; the existing top-level home project's repositories were left unchanged.
- The initial RPM version is `0.2.0+git20260906.8d0f57a`, pinned to upstream commit `8d0f57a6205fcda2e95782b1953178ab874fb0fa`. The workspace's own version remains 0.2.0. Manual OBS services produce the source and vendor archives; both source and vendor lockfiles match upstream exactly, and the vendor service's cargo audit passed without ignored advisories.
- Source regeneration through `scripts/prepare-rpm.sh` reproduced both archives byte-for-byte. SHA-256: source `f440306403b3246a12772b51d5108fbe631b8fa94028219af1c2f0ff3e2c62b5`; vendor `3d891949fc8948c42f2cfaf71b070c962688efb61dfc5065f5635382e74c8c3d`.
- The full local `osc build` passed in an isolated rootless KVM environment using Tumbleweed Rust/Cargo 1.98, distribution native compilers, a fresh CARGO_HOME and frozen/offline dependencies. All 64 tests and the generic x86_64 CMake flag check passed. Final rpmlint: zero errors, zero warnings and zero badness. Source, binary, debuginfo and debugsource RPMs were produced.
- GCC's native LTO objects failed when linked by Rust's LLVM linker; a standalone C/archive/linker probe reproduced the incompatibility. The spec disables only the GCC LTO flags. Both final binaries were verified as PIE with full RELRO, a non-executable stack and no RPATH/RUNPATH.
- A fresh Tumbleweed container installed the RPM and its declared runtime dependencies; both version commands worked before adding test utilities. The final RPM passed CLI/daemon lifecycle checks and a private D-Bus check of DeepSeek example startup, all four modes, model menu choices, seven exact mascot pixmaps, reload and shutdown. No model download or live API request was used.
- The RPM installs `/usr/bin` binaries, the matching launcher icon, CLI/daemon manual pages, a DeepSeek example, `README.openSUSE`, and an optional `/usr/lib/systemd/user/smarty-pants.service`. Reinstall and uninstall checks preserved private config/key files and a custom user service/drop-in. The user's actual desktop installation and credentials were not changed during packaging.
- The reviewed vendored runtime license set contains MPL-2.0 in the application and `option-ext`, with permissive licenses in the other runtime crates and llama.cpp. The source RPM includes vendored sources. Deduplication preserved the content of all 432 installed license/attribution files.
- Spec-cleaner produced no diff; source-validator, changelog lint/integrity, shell and documentation checks passed. Source-validator's preference for `update=true` is documented: the recipe preserves the audited upstream lockfile instead. Bugzilla checking through the installed packaging skill was unavailable without a configured/provided API-key path; no Bugzilla resolution was claimed.
- OBS source revision 1 built successfully on the server and published RPM `0.2.0+git20260906.8d0f57a-1.1`. The server log records 64 passing tests, the CPU portability check and zero rpmlint errors/warnings/badness. The public repository is `https://download.opensuse.org/repositories/home:/thbertoldi:/smarty-pants/openSUSE_Tumbleweed/`.
- The repository metadata and RPM signatures were verified against the authenticated OBS CLI's signing-key fingerprint `B280AF263B82F4BB73C37849804E0C8FADFEDCCF`. The published main RPM is 5,978,616 bytes; SHA-256 `7f4b33a5b8779e3129396a9eda301b4eb82bde8ab6d8300338f320177c996a64` also matches the signed repository metadata.
- A second fresh Tumbleweed container installed the published RPM from that repository with strict GPG checks, documentation and recommendations enabled. Zypper supplied Zenity, clipboard tools and manuals; RPM file verification and the published CLI/daemon lifecycle checks passed. The base container's `/usr/etc/zypp/zypp.conf.d` disables documentation and recommendations, so these settings were explicitly enabled for this desktop-installation check.
