# Model selection and writing quality

Research checked September 6, 2026. The target languages are English and Portuguese.

The repository already runs GGUF models inside the daemon through llama.cpp. Previously, startup hardcoded Qwen 2.5 7B even though `model.name` defaulted to Gemma 3 1B. Model selection now honors configuration.

| Candidate | Fit for this tool | Decision |
| --- | --- | --- |
| [Qwen 2.5 1.5B Instruct GGUF](https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct-GGUF) | Model family supports English and Portuguese; Apache 2.0; existing ChatML backend supports it. Q4_K_M weights are 1,117,320,736 bytes. | Default to reduce resource use; validate fidelity on actual writing. |
| [Qwen 2.5 3B Instruct GGUF](https://huggingface.co/Qwen/Qwen2.5-3B-Instruct-GGUF) | Larger multilingual alternative with 2,104,932,768-byte Q4_K_M weights. Its model card specifies the Qwen Research license. | Optional preset; review model terms for your use. |
| [Qwen 2.5 7B Instruct GGUF](https://huggingface.co/bartowski/Qwen2.5-7B-Instruct-GGUF) | Existing model, 4,683,074,240-byte weights. | Retained for comparisons and harder writing tasks. |
| [Grammarly CoEdIT-large](https://huggingface.co/grammarly/coedit-large) | An actual editing specialist: 770M parameters, based on FLAN-T5. English scope, CC-BY-NC-4.0, and a different model/prompt contract from the current GGUF chat path. | Evaluated separately: useful English corrections, but Portuguese corruption in all 12 Portuguese cases. Not integrated into the daemon. |

These sources do not establish a comparable hallucination rate for this tool's bilingual rewrite tasks. Parameter count, quantization, and temperature alone cannot establish factual fidelity. The smaller default is a resource choice, not a demonstrated quality improvement. It has about 76% fewer weight bytes than the previous 7B preset; RAM, VRAM, and latency savings must be measured separately.

The daemon uses a default temperature of 0.2, explicit fact/uncertainty preservation instructions for general rewriting, and per-mode overrides. It rejects empty outputs and outputs that run out of tokens/context, rather than pasting an incomplete rewrite. These checks do not detect all factual changes or refusals.

The optional `writing.preserve_literals` check is enabled by default. It compares numeric literals and links/email addresses, and checks code contents before a result reaches the clipboard. It blocks missing or invented numbers but can also reject harmless date or numeral formatting. It cannot establish semantic equivalence or detect all changed names, negations, and claims.

## Direct APIs

[DeepSeek's current API](https://api-docs.deepseek.com/api/create-chat-completion/) exposes `deepseek-v4-flash` and `deepseek-v4-pro`. This tool defaults to Flash with thinking disabled for short text transformations. Only the final `content` field is used. The client validates `finish_reason` and never pastes reasoning, tool calls, or incomplete responses. Network failure does not switch providers or retry paid requests automatically.

Compatible servers can be configured separately. API-only builds omit llama.cpp entirely, and API mode never initializes the embedded backend. A remote provider sees the selected text; local inference keeps text on the device after downloading model weights.

## Repeatable English/Portuguese evaluation

The [synthetic corpus](../examples/evaluation/writing.jsonl) includes grammar fixes, uncertain claims, negation, questions that must remain questions, academic claims, professional posts, and prompt compression with literal identifiers. It is intentionally small and is not a hallucination benchmark.

```sh
cargo run -p smarty-pants-daemon --example evaluate --locked -- \
  examples/evaluation/local.toml examples/evaluation/writing.jsonl \
  > target/writing-evaluation.jsonl
```

The example uses CPU inference and a fixed seed, loads/downloads the selected preset, and does not access the clipboard, shortcuts, or tray. Change the model name in a copy of the config to compare candidates. An API configuration sends the synthetic cases to that provider and may incur charges.

Each JSONL result includes the input and output, elapsed time, process RSS, missing literal facts, unexpected phrases, and detected language. The first successful request includes download/load time and should be separated from subsequent latency measurements. RSS does not include GPU memory.

Review every output for grammar, meaning, negation, uncertainty, omitted facts, added claims, and natural Portuguese. Exact substring checks are prompts for review: they can flag valid stylistic variation and miss invented facts. Add representative anonymized examples before making a quality claim, and compare multiple seeds or repeated API requests. Smaller models may be adequate for grammar yet struggle with compression or tone changes.

### Observed local smoke results

[Stored generation results](evaluations/2026-09-06-qwen-1.5b-cpu.jsonl) contain all 12 inputs and outputs from one run of the evaluation config, with GPU offload disabled, four threads, seed 42, and a development build. These are small-sample observations, not production performance estimates or a hallucination rate:

- All 12 outputs were detected as the expected language; no generation failed.
- Warm-request median was 20.6 seconds. Observed process RSS after requests was about 1.22 GiB. The first request included cached-weight verification/loading. GPU compute buffer allocation was zero.
- General grammar corrections worked in both languages. English date and digit-to-word formatting violated exact-literal instructions, and are blocked by the default check.
- Academic output retained the measured facts in this run. An earlier run omitted the entire methods sentence, which is why explicit preservation checks matter.
- Portuguese compression mostly reformatted the original rather than shortening it. Also review modality changes such as “might”/“would” and shifts such as “we”/“the team”; literal checks cannot decide whether these are acceptable.

The artifact retains generation results and fixture flags; apply the current validation code when assessing which outputs would be accepted. GPU latency, the 7B comparison, and live DeepSeek responses have not been measured here. The smaller preset is useful for constrained corrections, with the retained presets/API path available for evaluation on harder tasks.

### CoEdIT comparison

The [full comparison](evaluations/2026-09-06-coedit-vs-qwen/report.md) records 24 paired inputs (12 English and 12 Portuguese), all 48 outputs, tokenizer diagnostics, and per-case review notes. Both models used greedy decoding, four CPU threads, and an excluded warmup; Qwen used an optimized release build. This differs from the earlier development-build, temperature-0.2 smoke run.

CoEdIT's English median was 1.52 seconds versus Qwen's 5.17 seconds. Peak process RAM was 3.29 GiB for CoEdIT FP32 versus 1.86 GiB for Qwen Q4_K_M. The runtimes, precision, prompt lengths, and outputs differ, so these describe the tested configurations rather than an isolated model-size advantage.

CoEdIT's tokenizer lost Portuguese characters in all 12 Portuguese inputs, and all 12 generated Portuguese outputs had visible language problems. Its English output also damaged the name João and changed a technical quoted value from `"ready"` to `"done"`. Qwen handled Portuguese better, but omitted the measured methods in the English academic case and followed embedded instructions to write poems in both languages. The literal guard did not catch those poems. Prompt wrappers and language detection do not establish faithful rewriting.

See the [reproduction instructions](../examples/evaluation/README.md) to rerun the comparison. The review was qualitative, unblinded, and performed by the coding assistant. This small synthetic sample establishes neither a general hallucination rate nor statistical superiority. The running installation and default configuration were not changed for this experiment.

## Desktop smoke check

1. Start the daemon with a tray host available. Check provider/model and lazy-load status.
2. Change a model or GPU setting in the tray, restart the daemon, and confirm the value persisted.
3. Test a harmless English sentence and a Portuguese sentence in an editor with the configured hotkey.
4. Pause, trigger a rewrite, and verify the selection is unchanged. Resume.
5. Unload the local model and verify memory is released; the next rewrite should reload it.
6. Configure DeepSeek and run a synthetic rewrite. Confirm a bad key or low token limit leaves the selection unchanged.
7. Edit a prompt and reload. Try an invalid model or malformed TOML; the working configuration should remain active.
8. Quit from the tray and verify the socket disappears. CLI and shortcuts should also work when no tray host is available.
