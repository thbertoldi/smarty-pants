<p align="center">
  <img src="docs/assets/smartypants.png" alt="Smarty Pants logo" width="220">
</p>

<h1 align="center">Smarty Pants</h1>
<p align="center">Select text. Choose a tone. Keep writing.</p>
<p align="center">
  <a href="https://github.com/thbertoldi/smarty-pants/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/thbertoldi/smarty-pants/actions/workflows/ci.yml/badge.svg"></a>
  <a href="https://github.com/thbertoldi/smarty-pants/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/thbertoldi/smarty-pants"></a>
  <a href="LICENSE"><img alt="License: MPL 2.0" src="https://img.shields.io/badge/license-MPL--2.0-blue"></a>
  <img alt="Linux Wayland" src="https://img.shields.io/badge/desktop-Linux%20%2F%20Wayland-7c3aed">
</p>

A tray writing assistant for **English and Portuguese**. Rewrite, condense, or change the tone of selected text from a keyboard shortcut. Run a small model on your computer, or connect directly to DeepSeek or a compatible API. **No Ollama required.**

- **Four writing modes:** grammar and fluency, academic, LinkedIn, and condense.
- **Choose how results arrive:** automatic paste, copy only, or review the original and rewrite before copying.
- **Switch models from the tray:** local presets, DeepSeek Flash/Pro, and custom API model IDs. Changes apply without restarting.
- **Use resources when needed:** local weights load on the first rewrite and unload after five idle minutes. API builds contain no local inference engine.
- **Preserve important details:** checks reject changed numbers, links, email addresses, and code literals. These checks do not guarantee that meaning is preserved; [see the measured limitations](docs/limitations.md).

## Install

Download a Linux x86_64 archive and its matching `.sha256` file from [Releases](https://github.com/thbertoldi/smarty-pants/releases/latest).

| Download | Choose it for |
| --- | --- |
| `…-linux-x86_64-api.tar.gz` | **DeepSeek or another API.** Smallest application; no model download or GPU setup. |
| `…-linux-x86_64-cpu.tar.gz` | **Local writing plus APIs.** Includes llama.cpp for CPU inference; weights download on first use. |

Requires a compatible Wayland desktop, `wl-clipboard`, and `wtype`. Install `zenity` for settings and review dialogs. Binaries target glibc 2.35 or newer (Ubuntu 22.04+, current openSUSE Tumbleweed). GPU builds are available [from source](docs/install.md#build-from-source).

For example, after downloading the API archive and checksum into the current directory:

```sh
sha256sum -c smarty-pants-0.2.0-linux-x86_64-api.tar.gz.sha256
tar -xzf smarty-pants-0.2.0-linux-x86_64-api.tar.gz
bash smarty-pants-0.2.0-linux-x86_64-api/install.sh
```

Open **Smarty Pants** in your application launcher. The installer adds binaries under `~/.local/bin`, a desktop entry, and an optional user service. It preserves existing settings and custom service files. [Installation, upgrades, login startup, and uninstall →](docs/install.md)

## Use DeepSeek

1. Install the **API** bundle and open Smarty Pants. On a fresh installation, this bundle selects DeepSeek automatically.
2. Open the tray → **Provider → DeepSeek (cloud)**, then **Set API key…**. Paste your key into the local dialog.
3. Choose **API model → DeepSeek V4 Flash** or **DeepSeek V4 Pro**. The default is Flash; **Custom model…** accepts another model ID.
4. Choose **Rewrite delivery → Review, then copy…**, bind a shortcut, and try the examples below.

The key is saved in a separate private file, outside the TOML configuration. Selected text is sent to DeepSeek when you trigger a rewrite; its API usage is billed by DeepSeek. The client uses non-thinking mode. [Current DeepSeek models and API documentation](https://api-docs.deepseek.com/).

**Handing installation to another agent?** Share [the DeepSeek setup instructions](docs/install-deepseek.md). They cover the openSUSE/Hyprland environment, existing service migration, private key entry, and English/Portuguese checks.

## Use a local model

Install the CPU bundle, then choose **Provider → Local (on this device)**. The default is **Qwen 2.5 1.5B Instruct Q4_K_M**, about **1.1 GB** of weights. No local model loads while using an API provider.

**Tray → Local model (download size)** offers Qwen 1.5B, 3B, and 7B, plus Gemma 3 1B. Pick a preset; the next rewrite loads it. **Resource use** controls idle unloading and explicit unload. GPU acceleration requires a GPU-enabled source build; the CPU bundle does not add GPU support through a setting.

We compared Qwen with the smaller writing-specialized **CoEdIT** on 24 English/Portuguese examples. CoEdIT was faster in English, but its tokenizer and outputs corrupted Portuguese in all 12 Portuguese cases. It remains an evaluation option. [Model choices](docs/models.md) · [Full comparison and raw outputs](docs/evaluations/2026-09-06-coedit-vs-qwen/report.md).

## Write from a shortcut

Bind a free key combination in your compositor to:

```sh
smarty-pants trigger --mode rewrite
```

Other modes: `academic`, `linkedin`, `condense`. The daemon also supports the GlobalShortcuts portal. [Hyprland, Sway, and niri setup →](docs/install.md#keyboard-shortcuts)

Select text in an editable field, keep the selection in place, and press your shortcut. For a first check:

| Language | Example input |
| --- | --- |
| English | `We has tested 24 samples and the results looks promising.` |
| Portuguese | `Nós testou 24 amostras e os resultado parece promissor.` |

Check grammar, meaning, and the unchanged `24`. Review mode displays both texts and only copies after you choose **Copy rewrite**. Paste it into the intended field yourself.

Automatic paste checks the source window on Hyprland, Sway, and niri. If focus changes or cannot be verified, the rewrite stays on the clipboard and the tray/CLI explains why. Terminal applications that swallow synthetic paste work better with **Copy only**. [Remaining limitations →](docs/limitations.md)

## Configure

Most daily settings live in the tray. Advanced settings and custom prompts are TOML:

```sh
smarty-pants config path
smarty-pants config edit
smarty-pants config reload
smarty-pants status
```

See the commented [configuration example](examples/config.toml) and [DeepSeek example](examples/deepseek.toml). Settings validate before saving; tray edits preserve comments and custom prompts. An invalid reload leaves the running configuration intact.

## Development and packaging

The Rust workspace separates shared configuration/protocol (`core`), desktop/inference (`daemon`), and commands (`cli`). Rust **1.88+** is required; release builds use **1.91.1**. CI checks formatting, strict Clippy, tests, dependency advisories, desktop metadata, and documentation links. Release jobs test real bundle installation before publishing.

[Contributing and checks](CONTRIBUTING.md) · [RPM / Open Build Service recipe](packaging/rpm/README.md) · [Quality review](docs/quality-review.md) · [Changelog](CHANGELOG.md)

Native packages are the first distribution target because the app integrates with the host clipboard, compositor commands, and session bus. Flatpak would need additional desktop integration work; a container is useful for builds, but inconvenient as the desktop runtime. There is no published OBS repository yet.

## Acknowledgments and license

[Handy](https://github.com/cjpais/Handy) inspired the lightweight desktop workflow and helped inform the Wayland clipboard integration. Local inference uses [llama.cpp](https://github.com/ggml-org/llama.cpp) through [llama-cpp-rs](https://github.com/utilityai/llama-cpp-rs).

Smarty Pants is licensed under [MPL-2.0](LICENSE). Model weights have their own licenses and are downloaded separately. Release bundles include dependency license files.
