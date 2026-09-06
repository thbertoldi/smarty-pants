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

The [release workflow](https://github.com/thbertoldi/smarty-pants/actions/workflows/release.yml) provides build and installation evidence for the downloadable assets. The RPM package also passes an isolated Tumbleweed KVM build with distribution compilers, offline vendored dependencies, all 64 tests, and zero rpmlint errors, warnings or badness. Both installed binaries have PIE, full RELRO, a non-executable stack and no RPATH/RUNPATH. GCC LTO is disabled for native objects because Rust's LLVM linker cannot read GCC LTO archives; the other distribution hardening flags remain enabled.

A fresh Tumbleweed container resolved the RPM's declared dependencies. Installed-package checks covered daemon lifecycle, private D-Bus tray/menu/pixmap export, DeepSeek example startup without model downloads, and preservation of private configuration, credentials and custom service files during reinstall/removal. All 432 license and attribution files retained their contents after deduplication. Bugzilla lookup through the packaging skill was unavailable without configured credentials; no Bugzilla issue-resolution claim is made.

Live DeepSeek calls, visual review of the user's desktop panel, and full Sway/niri sessions are outside the automated checks. The personal OBS package has not undergone official distribution acceptance. See [remaining limitations](limitations.md) before treating a rewrite as verified fact.

## Published release evidence

[CI run 34019430631](https://github.com/thbertoldi/smarty-pants/actions/runs/34019430631) passed all jobs, including 64 tests in each API/CPU/Vulkan configuration. [Release run 34019432615](https://github.com/thbertoldi/smarty-pants/actions/runs/34019432615) published [v0.2.0](https://github.com/thbertoldi/smarty-pants/releases/tag/v0.2.0) from `fc43fc35a3963493dbdadb34b52e13925afb9778` after build, installation, lifecycle and CI checks succeeded.

Both final bundles also passed temporary-directory installation and lifecycle checks on the openSUSE development host. Published downloads matched the tested artifacts byte-for-byte and passed their SHA-256 checks. Archive sizes are approximately 7.8 MiB (API) and 9.2 MiB (CPU), excluding model weights.

The [personal OBS package](https://build.opensuse.org/package/show/home:thbertoldi:smarty-pants/smarty-pants) published `0.2.0+git20260906.8d0f57a-1.1` for Tumbleweed x86_64. The server build also passed all 64 tests, the CPU portability check and rpmlint with zero errors, warnings or badness. The public repository metadata signature, RPM signature and download checksums were verified against the signing key obtained through the authenticated OBS CLI. The main RPM is approximately 5.7 MiB, excluding model weights and optional debug packages.

A second fresh Tumbleweed container installed that published RPM through Zypper with strict repository signature checks. With documentation and recommended packages enabled, the install supplied Zenity, clipboard tools and manual pages. RPM file verification and the published binaries' lifecycle checks passed.
