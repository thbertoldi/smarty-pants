# Quality review — September 2026

The review covered shared configuration and IPC, CLI lifecycle, local and API inference, model downloads, selection and delivery, tray/shortcuts, evaluation documentation, and installation/release infrastructure. This is a repository review with automated regression checks, not a formal security audit or a model hallucination benchmark.

## Changes from the review

- Fixed stale clipboard capture, checked source-window identity before automatic paste, and corrected restoration to use the clipboard from before capture.
- Added copy/review delivery so users can handle terminal paste limitations and inspect meaning before using output. Window identity parsing covers Hyprland, Sway and niri; only Hyprland has been used as the development desktop.
- Added a dedicated API model menu with validated persistence; changing a model retains its URL and credentials.
- Made detached startup report readiness or a log path, added daemon version/help, and accepted valid API responses with null tool_calls.
- Capped clipboard text allocation at 4 MiB in addition to the configured selection character limit.
- Corrected the declared Rust minimum from 1.75 to 1.88 to match the locked dependencies.
- Added CI, dependency update automation, native release bundles, installation checks, and an offline RPM source recipe. Release actions are pinned to commit IDs and run with read permissions except the publishing job.
- Reworked user-facing documentation around installation, model selection, measured behavior, and explicit remaining limitations.

## Dependency audit

`cargo-audit 0.22.2` initially reported three vulnerability advisories and two unsoundness warnings. The lockfile now uses:

| Dependency | Previous | Updated | Advisory |
| --- | --- | --- | --- |
| quick-xml (via wayland-scanner) | 0.39.4 | 0.41.0 | [RUSTSEC-2026-0194](https://rustsec.org/advisories/RUSTSEC-2026-0194.html), [RUSTSEC-2026-0195](https://rustsec.org/advisories/RUSTSEC-2026-0195.html) |
| quinn-proto | 0.11.14 | 0.11.17 | [RUSTSEC-2026-0185](https://rustsec.org/advisories/RUSTSEC-2026-0185.html) |
| anyhow | 1.0.102 | 1.0.104 | [RUSTSEC-2026-0190](https://rustsec.org/advisories/RUSTSEC-2026-0190.html) |
| event-listener | 5.4.1 | 5.4.2 | [RUSTSEC-2026-0221](https://rustsec.org/advisories/RUSTSEC-2026-0221.html) |

The subsequent audit reported **zero vulnerabilities and zero warnings**, using RustSec database commit `5a0ebedfe8bdd2e295b171f4162f8c977bcad9a5`. This is a lockfile-wide finding, not a claim that every advisory was reachable at runtime. It also does not cover every native llama.cpp issue; upstream maintenance and future audits remain necessary.

## Validation and boundaries

Regression tests use in-memory clipboard/focus mocks and a local HTTP server. They cover changed/unknown focus, rejected stale captures, terminal key combinations, restoration, review acceptance/cancellation, malformed or incomplete API output, and transactional settings. Installer checks use temporary XDG directories, real bundle binaries, a prefix containing spaces, upgrade preservation, and uninstall.

[CI](https://github.com/thbertoldi/smarty-pants/actions/workflows/ci.yml) runs formatting, strict Clippy and tests for API, CPU and Vulkan configurations, plus dependency audits and documentation/desktop metadata checks. The API job checks Rust 1.88. Release builds target Ubuntu 22.04 and check llama.cpp's CMake CPU flags: `SOURCE_DATE_EPOCH` disables its optional instruction defaults, in addition to Rust's generic x86_64 target.

The [release workflow](https://github.com/thbertoldi/smarty-pants/actions/workflows/release.yml) provides build and installation evidence for the downloadable assets. An RPM spec and vendoring script are included, but an OBS repository and distro review are not complete. Live DeepSeek calls, visual review of the user's desktop panel, and full Sway/niri sessions are outside the automated checks. See [remaining limitations](limitations.md) before treating a rewrite as verified fact.
