# Contributing

Use Rust 1.88 or later. Local inference also needs CMake, Clang/libclang and a C++ compiler; see [source setup](docs/install.md#build-from-source). Run Cargo jobs sequentially when they share a target directory.

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo test --workspace --locked --no-default-features --features smarty-pants-daemon/tray
shellcheck packaging/linux/*.sh scripts/*.sh
desktop-file-validate packaging/linux/computer.smarty-pants.desktop
python3 scripts/check-docs.py
cargo audit
```

Install the audit tool with `cargo install cargo-audit --version 0.22.2 --locked`. CPU-only local checks use `--no-default-features --features smarty-pants-daemon/local,smarty-pants-daemon/tray`. The default daemon build includes Vulkan and tray support. CI also checks the minimum Rust version with the API build.

Add regression tests for changed capture, delivery, configuration, protocol, and API behavior. Use `MockWayland` and the local mock HTTP server; automated tests must not touch the user's clipboard or call paid APIs. Keep text and API keys out of logs, test fixtures, and screenshots. The [evaluation harness](examples/evaluation/README.md) is separate from deterministic CI tests; document hardware, model revisions, prompts, and parameters when comparing models.

Settings changes go through `Settings`, which validates before atomic persistence and coordinates with the rewrite pipeline. Config edits should preserve user comments and custom prompts. Document changed defaults and migration steps in the changelog.

For releases, update the workspace version, changelog, release notes and download examples together. Push the commit to main and a matching `vX.Y.Z` tag after local checks. The [release workflow](.github/workflows/release.yml) builds on Ubuntu 22.04 with a generic x86_64 CPU target, tests both variants, packages dependency licenses and SHA-256 files, and tests installation. Publication additionally waits for successful main-branch CI on that exact commit. `workflow_dispatch` builds downloadable workflow artifacts without publishing. When updating the OBS package, advance its spec version and pinned source service revision/filename together and regenerate its sources; the initial RPM uses a snapshot version. See [RPM/OBS](packaging/rpm/README.md) for distribution packaging.
