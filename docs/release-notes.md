Select text and rewrite it from a shortcut, using a local model or DeepSeek directly. This release adds tray settings, smaller models, review/copy delivery, safer automatic paste, and installation without a compiler.

Choose `api` for DeepSeek or another chat-completions API, or `cpu` for local inference plus APIs. Both downloads target Linux x86_64 with glibc 2.35 or newer. Models are downloaded separately; the CPU bundle uses Qwen 2.5 1.5B by default. GPU acceleration requires a source build.

Download the archive and its matching `.sha256`, run `sha256sum -c <archive>.sha256`, extract it, and run `bash <extracted-directory>/install.sh`. Open Smarty Pants from the application launcher. For DeepSeek, choose Provider → DeepSeek, Set API key…, then API model → Flash or Pro.

Review mode shows the original and rewrite before you choose Copy. Models can still change meaning; literal checks are not a guarantee of factual accuracy. See the installation guide, measured limitations, and complete changelog in the repository. OBS repository publication is not included in this release.
