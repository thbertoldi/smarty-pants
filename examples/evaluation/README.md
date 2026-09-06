# Comparing Qwen and CoEdIT

This is an opt-in experiment using synthetic text. It does not access the
clipboard, change the daemon's settings, or install a new production backend.

`comparison.jsonl` contains 24 paired cases: 12 English and 12 Portuguese.
It includes the original 12 writing fixtures, then adds agreement, spelling,
diacritics, already-correct text, negation, email/currency literals, and embedded
instructions that must be edited rather than followed. The cases were fixed
before generating either model's comparison outputs.

The comparison uses Qwen 2.5 1.5B Instruct Q4_K_M through the application's
llama.cpp backend and CoEdIT-large in its official FP32 format through
Transformers. Both run sequentially on the CPU with four threads, greedy
decoding, a 512-token output limit, and a separate excluded warmup. The model
formats, tokenizers, prompt lengths, and runtimes differ. This compares two
usable inference setups; it does not isolate architecture or quantization.
Qwen's comparison temperature is zero; the production default remains 0.2.

Qwen uses the existing application mode prompts and language reminders.
CoEdIT uses the short instruction-plus-text format demonstrated by its
[authors](https://github.com/vipulraheja/coedit), with the task instruction
stored explicitly in each fixture. English instructions are also used for
Portuguese input. Neither setup translates the source text before editing.
The complete prompts are saved in the result files; this is not an identical
prompt comparison. Formal, professional, and technical compression tasks
are exploratory applications beyond basic grammatical correction.

## Run

Build the optimized CPU-only evaluator before running either model:

```sh
cargo build -p smarty-pants-daemon --example evaluate --release --locked \
  --no-default-features --features local -j 4
target/release/examples/evaluate examples/evaluation/comparison.toml \
  examples/evaluation/comparison.jsonl --warmup > target/qwen-comparison.jsonl
```

Install CoEdIT's evaluation dependencies in an isolated environment. The
requirements pin the packages used in the recorded run; the extra index
provides the CPU build of PyTorch. These dependencies are not needed by the
application.

```sh
uv venv target/coedit-venv --python 3.12
uv pip install --python target/coedit-venv/bin/python \
  --extra-index-url https://download.pytorch.org/whl/cpu \
  --index-strategy unsafe-best-match \
  -r examples/evaluation/coedit-requirements.txt
target/coedit-venv/bin/python examples/evaluation/coedit.py \
  examples/evaluation/comparison.jsonl \
  --metadata target/coedit-metadata.json > target/coedit-raw.jsonl
target/release/examples/evaluate --assess examples/evaluation/comparison.jsonl \
  target/coedit-raw.jsonl > target/coedit-comparison.jsonl
```

CoEdIT's script downloads the pinned official model revision if necessary
(about 3.1 GB of weights), checks the weights' SHA-256, and loads only
Safetensors without remote code. `--model-path` can point to a local copy of
that revision. Qwen uses the application's verified preset cache. Run the
models sequentially and avoid other heavy work while measuring latency.

The imported-output assessment uses the same Rust language detector and
literal-preservation check as Qwen. It rejects duplicate, missing, or unknown
case IDs and mismatched source text. All generated outputs remain available
for review, including those the application's guard would block. Substring
flags and language detection are diagnostics, not semantic correctness scores.

Recompute the recorded run's numeric summary without inference:

```sh
python3 examples/evaluation/summarize.py \
  docs/evaluations/2026-09-06-coedit-vs-qwen
```

## Review criteria

Review grammar, meaning, and task completion independently. A usable rewrite
must correct the introduced errors, remain natural in the source language,
preserve names, facts, negation and uncertainty, and avoid answering or obeying
the text. Already-correct cases should stay unchanged. Compression must retain
the instructions while making the text shorter. Treat literal formatting
violations separately from invented or omitted information.

The recorded review is a qualitative inspection by the coding assistant,
with model identities visible, and no independent human ratings. One greedy
run on 24 synthetic cases cannot establish a general hallucination rate,
statistical superiority, or production performance. Inference failures, bad
outputs, and harmless variations must all be reported rather than excluded.

See the [recorded comparison](../../docs/evaluations/2026-09-06-coedit-vs-qwen/report.md)
for outputs, observations, resource measurements, and conclusions.
