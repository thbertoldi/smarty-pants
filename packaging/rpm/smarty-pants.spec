Name:           smarty-pants
Version:        0.2.0
Release:        0
Summary:        Tray writing assistant for English and Portuguese on Wayland
License:        MPL-2.0
URL:            https://github.com/thbertoldi/smarty-pants
Source0:        %{name}-%{version}.tar.gz
Source1:        vendor.tar.zst
BuildRequires:  cargo >= 1.88
BuildRequires:  rust >= 1.88
BuildRequires:  gcc-c++
BuildRequires:  clang-devel
BuildRequires:  cmake
BuildRequires:  pkgconfig
BuildRequires:  python3
BuildRequires:  desktop-file-utils
Requires:       wl-clipboard
Requires:       wtype
Recommends:     zenity
Recommends:     xdg-utils
ExclusiveArch:  x86_64

%description
Rewrite selected text using a small embedded local model, DeepSeek, or a
compatible API. Includes tray settings, review and clipboard delivery,
keyboard shortcut integration, and an optional systemd user service.
This package enables CPU inference and APIs. Model weights download on demand.

%prep
%autosetup -a1

%build
export CARGO_HOME=$PWD/.cargo-home
# Native CPU instructions must never leak from the build machine into packages.
export RUSTFLAGS="-C target-cpu=x86-64"
export SOURCE_DATE_EPOCH=${SOURCE_DATE_EPOCH:-0}
export CMAKE_BUILD_PARALLEL_LEVEL=%{_smp_build_ncpus}
cargo build --release --frozen --workspace --no-default-features --features smarty-pants-daemon/local,smarty-pants-daemon/tray

python3 scripts/check-portable-build.py

%check
export CARGO_HOME=$PWD/.cargo-home
export RUSTFLAGS="-C target-cpu=x86-64"
export SOURCE_DATE_EPOCH=${SOURCE_DATE_EPOCH:-0}
cargo test --release --frozen --workspace --no-default-features --features smarty-pants-daemon/local,smarty-pants-daemon/tray
desktop-file-validate packaging/linux/computer.smarty-pants.desktop

%install
install -Dm755 target/release/smarty-pants %{buildroot}%{_bindir}/smarty-pants
install -Dm755 target/release/smarty-pants-daemon %{buildroot}%{_bindir}/smarty-pants-daemon
install -Dm644 packaging/linux/computer.smarty-pants.desktop %{buildroot}%{_datadir}/applications/computer.smarty-pants.desktop
install -Dm644 docs/assets/smartypants.png %{buildroot}%{_datadir}/pixmaps/computer.smarty-pants.png
install -Dm644 examples/deepseek.toml %{buildroot}%{_datadir}/smarty-pants/examples/deepseek.toml
install -Dm644 packaging/systemd/smarty-pants.service %{buildroot}%{_userunitdir}/smarty-pants.service
sed -i 's|ExecStart=.*|ExecStart=%{_bindir}/smarty-pants-daemon|' %{buildroot}%{_userunitdir}/smarty-pants.service

%files
%license LICENSE
%doc README.md CHANGELOG.md docs/limitations.md
%{_bindir}/smarty-pants
%{_bindir}/smarty-pants-daemon
%{_datadir}/applications/computer.smarty-pants.desktop
%{_datadir}/pixmaps/computer.smarty-pants.png
%{_datadir}/smarty-pants/
%{_userunitdir}/smarty-pants.service

%changelog
