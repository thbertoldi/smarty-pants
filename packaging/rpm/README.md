# RPM and Open Build Service

This recipe builds the **CPU + API + tray** variant for openSUSE x86_64. It installs binaries in `/usr/bin`, a desktop launcher, an optional systemd user unit, and the DeepSeek example. It does not download model weights at build time, enable a service automatically, or install user credentials.

The package lives in the maintainer's [OBS project](https://build.opensuse.org/package/show/home:thbertoldi:smarty-pants/smarty-pants). It is a personal repository for **openSUSE Tumbleweed x86_64**, separate from the official distribution repositories. See [installation instructions](../../docs/install.md#opensuse-tumbleweed-rpm).

The initial OBS version is `0.2.0+git20260906.8d0f57a`, a pinned snapshot after v0.2.0 that includes the new tray icon. The application's own `--version` remains `0.2.0`; RPM records the additional snapshot identifier.

## Prepare sources

Install the maintainer tools on Tumbleweed:

```sh
sudo zypper install osc obs-service-obs_scm obs-service-tar \
  obs-service-recompress obs-service-cargo obs-service-source_validator \
  spec-cleaner cargo cpio tar gzip zstd
```

Then prepare sources from the repository root, using an empty output directory:

```sh
scripts/prepare-rpm.sh
```

The script executes the manual services declared in [_service](_service) through osc's service runner; it needs no OBS credentials. `obs_scm` fetches the exact upstream commit, `tar`/`recompress` produce the source archive, and `cargo_vendor` produces `vendor.tar.zst`, its Cargo configuration and lockfile. The output contains only the spec, changelog, source services, constraints, openSUSE instructions and two source archives.

`update=false` and `respect-lockfile=true` preserve the upstream lockfile already covered by CI. The vendoring service also audits it. `source_validator` emits a policy warning preferring automatic updates; the intentional locked configuration avoids silently replacing the tested dependency set. Refresh dependencies upstream, run CI/audit, then advance the pinned revision and regenerate. Never suppress a dependency advisory to make the service succeed.

Source preparation uses the network; `%build` and `%check` use Cargo's `--frozen` mode and a fresh CARGO_HOME, so RPM builds do not fetch dependencies. The Rust packaging macros retain debug information and embed dependency audit metadata. Compilation is limited according to available memory. GCC link-time optimization is disabled for the native llama.cpp objects because Rust's LLVM linker cannot consume GCC LTO archives; the remaining distribution hardening flags are retained.

The source RPM includes vendored dependency sources, and the binary RPM includes their license notices. The initial license review found MPL-2.0 in the application and `option-ext`, with permissively licensed remaining runtime dependencies and llama.cpp. Recheck the actual vendored crate set whenever it changes, especially before any submission to a distribution project.

## Local build

Prefer an isolated OBS build environment. From the package checkout:

```sh
osc build openSUSE_Tumbleweed x86_64 smarty-pants.spec
```

On a host with accessible KVM and QEMU, use an 8 GiB VM with enough disk space for the compiler, vendored sources and debug information:

```sh
osc build --vm-type=kvm --vm-memory=8192 --vm-disk-size=16000 \
  --jobs=2 openSUSE_Tumbleweed x86_64 smarty-pants.spec
```

This supports a build without privileged host installation. Read the full build verdict, test counts and rpmlint summary. No live API key is needed for build tests.

## Maintain the OBS package

The local checkout is `/home/thbertoldi/obs_builds/home:thbertoldi:smarty-pants/smarty-pants`. On another machine:

```sh
osc checkout home:thbertoldi:smarty-pants smarty-pants
cd home:thbertoldi:smarty-pants/smarty-pants
```

Before reusing an existing checkout, run `osc status` and `osc update`, preserving local edits. Maintain the spec, service definition, constraints and openSUSE instructions in this Git repository as well as OBS. When advancing the source, update the full `_service` revision, its version format/source filename and the spec version together. Add one changelog entry preserving older entries.

Run `osc service manualrun` after changing the pinned source, or copy the files from `scripts/prepare-rpm.sh` into the checkout. Commit the source tarball and vendor tarball; exclude the intermediate `.obscpio`, `.obsinfo` and source checkout. Review `osc status` and `osc diff`, run spec-cleaner, `osc service run source_validator` and a clean build before `osc commit`.

After publishing a new version, verify the repository's RPM and metadata are available and test installation in a disposable Tumbleweed environment. Instructions for DeepSeek and migration from a user-local build are shipped as [README.openSUSE](README.openSUSE). Factory submission and additional distribution/architecture targets are separate work.

Source service documentation: [obs-service-cargo](https://github.com/openSUSE-Rust/obs-service-cargo). Build and publication results are recorded in [the repository knowledge file](../../knowledge/CERTAINS.md).
