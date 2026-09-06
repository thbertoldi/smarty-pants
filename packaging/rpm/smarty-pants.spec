# GCC LTO archives from llama.cpp cannot be consumed by Rust's LLVM linker.
# Keep the remaining distribution optimization and hardening flags.
%define _lto_cflags %{nil}
%global __rustflags -C target-cpu=x86-64
Name:           smarty-pants
Version:        0.2.0+git20260906.8d0f57a
Release:        0
Summary:        Tray writing assistant for English and Portuguese on Wayland
# Legal-Review-Notice: option-ext is MPL-2.0, like the application.
# Other linked crates and llama.cpp are permissively licensed. Their
# notices are installed; the source RPM includes the vendored source.
License:        MPL-2.0
URL:            https://github.com/thbertoldi/smarty-pants
Source0:        %{name}-%{version}.tar.zst
Source1:        vendor.tar.zst
Source2:        README.openSUSE
BuildRequires:  cargo >= 1.88
BuildRequires:  cargo-packaging
BuildRequires:  clang-devel
BuildRequires:  cmake >= 3.14
BuildRequires:  desktop-file-utils
BuildRequires:  fdupes
BuildRequires:  gcc-c++
BuildRequires:  help2man
BuildRequires:  memory-constraints
BuildRequires:  pkgconfig
BuildRequires:  python3
BuildRequires:  rust >= 1.88
BuildRequires:  systemd-rpm-macros
Requires:       wl-clipboard
Requires:       wtype
Recommends:     xdg-utils
Recommends:     zenity
# The CPU baseline check and dependency-license export target x86_64.
ExclusiveArch:  x86_64

%description
Rewrite selected text using a small embedded local model, DeepSeek, or a
compatible API. Includes tray settings, review and clipboard delivery,
keyboard shortcut integration, and an optional systemd user service.
This package enables CPU inference and APIs. Model weights download on demand.

%prep
%autosetup -a1
rm rust-toolchain.toml
cp %{SOURCE2} README.openSUSE

%build
%limit_build -m 2000
export CARGO_HOME=$PWD/.cargo-home
# Native CPU instructions must never leak from the build machine into packages.
export SOURCE_DATE_EPOCH=${SOURCE_DATE_EPOCH:-0}
export CFLAGS="%{optflags}"
export CXXFLAGS="%{optflags}"
export CMAKE_BUILD_PARALLEL_LEVEL=$RPM_BUILD_NCPUS
%{cargo_build} --frozen --workspace --no-default-features --features smarty-pants-daemon/local,smarty-pants-daemon/tray

python3 scripts/check-portable-build.py
# Record the effective native flags in the build log for review.
sed -n '/^CXX_FLAGS =/p' target/release/build/llama-cpp-sys-2-*/out/build/src/CMakeFiles/llama.dir/flags.make
help2man --no-info --name="Wayland writing assistant" target/release/smarty-pants > smarty-pants.1
help2man --no-info --name="Smarty Pants writing assistant daemon" target/release/smarty-pants-daemon > smarty-pants-daemon.1

%check
%limit_build -m 2000
export CARGO_HOME=$PWD/.cargo-home
export SOURCE_DATE_EPOCH=${SOURCE_DATE_EPOCH:-0}
export CFLAGS="%{optflags}"
export CXXFLAGS="%{optflags}"
export CMAKE_BUILD_PARALLEL_LEVEL=$RPM_BUILD_NCPUS
%{cargo_test} --release --frozen --workspace --no-default-features --features smarty-pants-daemon/local,smarty-pants-daemon/tray
desktop-file-validate packaging/linux/computer.smarty-pants.desktop

%install
export CARGO_HOME=$PWD/.cargo-home
export CARGO_NET_OFFLINE=true
python3 scripts/bundle-licenses.py cpu target/dependency-licenses
install -d %{buildroot}%{_licensedir}/%{name}
cp -a LICENSE target/dependency-licenses %{buildroot}%{_licensedir}/%{name}/
%fdupes %{buildroot}%{_licensedir}/%{name}
install -Dm755 target/release/smarty-pants %{buildroot}%{_bindir}/smarty-pants
install -Dm755 target/release/smarty-pants-daemon %{buildroot}%{_bindir}/smarty-pants-daemon
install -Dm644 packaging/linux/computer.smarty-pants.desktop %{buildroot}%{_datadir}/applications/computer.smarty-pants.desktop
install -Dm644 docs/assets/app-icon.png %{buildroot}%{_datadir}/pixmaps/computer.smarty-pants.png
install -Dm644 examples/deepseek.toml %{buildroot}%{_datadir}/smarty-pants/examples/deepseek.toml
install -Dm644 packaging/systemd/smarty-pants.service %{buildroot}%{_userunitdir}/smarty-pants.service
sed -i 's|ExecStart=.*|ExecStart=%{_bindir}/smarty-pants-daemon|' %{buildroot}%{_userunitdir}/smarty-pants.service
install -Dm644 smarty-pants.1 %{buildroot}%{_mandir}/man1/smarty-pants.1
install -Dm644 smarty-pants-daemon.1 %{buildroot}%{_mandir}/man1/smarty-pants-daemon.1

%pre
%{systemd_user_pre smarty-pants.service}

%post
%{systemd_user_post smarty-pants.service}

%preun
%{systemd_user_preun smarty-pants.service}

%postun
%{systemd_user_postun smarty-pants.service}

%files
%license %{_licensedir}/%{name}/
%doc README.md CHANGELOG.md docs/limitations.md README.openSUSE
%{_bindir}/smarty-pants
%{_bindir}/smarty-pants-daemon
%{_mandir}/man1/smarty-pants.1%{?ext_man}
%{_mandir}/man1/smarty-pants-daemon.1%{?ext_man}
%{_datadir}/applications/computer.smarty-pants.desktop
%{_datadir}/pixmaps/computer.smarty-pants.png
%{_datadir}/smarty-pants/
%{_userunitdir}/smarty-pants.service

%changelog
