# Smarty Pants — Linux release

Requires Linux x86_64, glibc 2.35+, a supported Wayland session, `wl-clipboard` and `wtype`. Install `zenity` for settings and review dialogs. A StatusNotifierItem tray host, such as Waybar with its tray module, is needed for the tray menu.

From this extracted directory, run:

```sh
bash install.sh
```

Open **Smarty Pants** from your application launcher. Binaries install under `~/.local/bin`; add that directory to PATH for terminal commands and compositor shortcuts. Optional login startup instructions appear after installation. Existing configuration and custom service files are preserved. A running old daemon must be stopped/restarted before the new binary takes effect.

The `FLAVOR` file identifies this bundle. **api** includes DeepSeek and compatible APIs, with no local inference engine. A fresh API installation selects DeepSeek; enter a key through **Tray → Set API key…**, then use **API model → DeepSeek V4 Flash / Pro**. **cpu** also includes local inference, using Qwen 2.5 1.5B by default; model weights download separately on the first rewrite. GPU acceleration requires a source build.

Bind a compositor shortcut to `smarty-pants trigger --mode rewrite`. Other modes are `academic`, `linkedin`, and `condense`. Try **Rewrite delivery → Review, then copy…** to compare the original and generated text before copying. AI output can change meaning even when literal checks pass.

[Complete installation and upgrade guide](https://github.com/thbertoldi/smarty-pants/blob/v0.2.0/docs/install.md) · [DeepSeek agent instructions](https://github.com/thbertoldi/smarty-pants/blob/v0.2.0/docs/install-deepseek.md) · [Known limitations](https://github.com/thbertoldi/smarty-pants/blob/v0.2.0/docs/limitations.md)

`SOURCE` records the build commit. `licenses/` contains dependency license files and a manifest. The application is MPL-2.0 (see `LICENSE`); model weights have separate licenses.
