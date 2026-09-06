# RPM and Open Build Service

This recipe builds the **CPU + API + tray** variant for openSUSE x86_64. It installs binaries in `/usr/bin`, a desktop launcher, an optional systemd user unit, and the DeepSeek example. It does not download model weights at build time, enable a service automatically, or install user credentials.

The project has **no published OBS repository yet**. GitHub release archives are the ready-to-install distribution path. This recipe is the starting point for a reviewed distro package; OBS repository setup and build validation must be completed before advertising `zypper install smarty-pants`.

## Prepare sources

From a clean, committed checkout, with Rust/Cargo, Git and zstd installed:

```sh
scripts/prepare-rpm.sh
```

The script exports the committed source and uses `cargo vendor --locked` to prepare `vendor.tar.zst` with a relative `.cargo/config.toml`. Dependency versions remain fixed by Cargo.lock. Source preparation uses the network; `%build` and `%check` use Cargo's `--frozen` mode and a fresh CARGO_HOME, so RPM builds do not fetch dependencies.

Review third-party licenses and native llama.cpp dependencies before distro submission. The standalone bundles include dependency license files; a distro package may require additional license annotations, bundled-library metadata and distribution-specific policies.

## Local build

Use an openSUSE build environment that provides Rust/Cargo 1.88 or newer, the spec's build dependencies, and `rpm-build`. For example, after installing those prerequisites:

```sh
mkdir -p target/rpmbuild/{SOURCES,SPECS,BUILD,BUILDROOT,RPMS,SRPMS}
cp target/rpm-sources/*.tar.* target/rpmbuild/SOURCES/
cp packaging/rpm/smarty-pants.spec target/rpmbuild/SPECS/
rpmbuild -ba --define "_topdir $PWD/target/rpmbuild" target/rpmbuild/SPECS/smarty-pants.spec
```

A clean OBS build is preferable to inheriting a developer machine's toolchain. Confirm the resulting RPM installs on a test machine, runs both `--version` commands, launches the tray, and rewrites synthetic English/Portuguese examples. No live API key is needed for build tests.

## Submit through OBS

Choose an OBS project you own and a current openSUSE Tumbleweed x86_64 build target. Create a `smarty-pants` package in that project, then copy the prepared spec, source archive and vendor archive into its `osc checkout`. Review `osc status` and `osc diff` before `osc addremove` and `osc commit`.

OBS builds the committed spec in an isolated environment. Inspect `osc results` and the complete build log, fix distribution review findings, then publish the project repository through OBS. Do not describe the package as available until the build and repository publication succeed. Publishing an OBS project requires the maintainer's account and project choice; this repository does not include credentials.

For maintained OBS source services, see [openSUSE-Rust/obs-service-cargo](https://github.com/openSUSE-Rust/obs-service-cargo). A service can replace the manual vendoring step later. [Cargo's vendor documentation](https://doc.rust-lang.org/cargo/commands/cargo-vendor.html) describes the source replacement mechanism.
