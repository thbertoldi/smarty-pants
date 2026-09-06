"""Opt-in CPU evaluation of CoEdIT; independent of the daemon and clipboard."""

import argparse
import hashlib
import importlib.metadata
import json
import platform
import sys
import time
from pathlib import Path

MODEL = "grammarly/coedit-large"
REVISION = "5637bcdf9d8d4419f97c8cfea36f7d35c79232b6"
WEIGHTS_SHA256 = "c692f68bca5c6899801bc9fad626fefd3359ccd95e26dccae4dd72186fd98852"
WARMUP = "Fix grammatical errors in this text: This sentence have a small grammatical error."


def memory_kib(field):
    for line in Path("/proc/self/status").read_text().splitlines():
        if line.startswith(field + ":"):
            return int(line.split()[1])
    return None


def sha256(path):
    with Path(path).open("rb") as handle:
        return hashlib.file_digest(handle, "sha256").hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("cases", type=Path)
    parser.add_argument("--model-path", type=Path, help="Use already downloaded official files")
    parser.add_argument("--metadata", type=Path, required=True)
    parser.add_argument("--threads", type=int, default=4)
    args = parser.parse_args()
    if args.threads < 1:
        parser.error("--threads must be positive")
    cases = [json.loads(line) for line in args.cases.read_text().splitlines() if line.strip()]
    if not cases or len({case["id"] for case in cases}) != len(cases):
        parser.error("the corpus must be nonempty and contain unique IDs")
    for case in cases:
        if not case.get("coedit_instruction") or not case.get("text"):
            parser.error("every case needs coedit_instruction and text")

    import torch
    from huggingface_hub import snapshot_download
    from transformers import AutoModelForSeq2SeqLM, AutoTokenizer

    torch.set_num_threads(args.threads)
    torch.set_num_interop_threads(1)
    torch.manual_seed(42)
    torch.use_deterministic_algorithms(True)
    source = args.model_path or Path(snapshot_download(
        MODEL, revision=REVISION,
        allow_patterns=["*.json", "spiece.model", "model.safetensors"],
        ignore_patterns=["trainer_state.json"],
    ))
    # Verify weights even when --model-path points to a local directory.
    start = time.perf_counter()
    if sha256(source / "model.safetensors") != WEIGHTS_SHA256:
        raise RuntimeError("CoEdIT weights SHA-256 mismatch")
    verification_ms = (time.perf_counter() - start) * 1000
    start = time.perf_counter()
    tokenizer = AutoTokenizer.from_pretrained(source, local_files_only=True, trust_remote_code=False)
    model = AutoModelForSeq2SeqLM.from_pretrained(
        source, local_files_only=True, trust_remote_code=False,
        use_safetensors=True, dtype=torch.float32,
    ).to("cpu").eval()
    load_ms = (time.perf_counter() - start) * 1000

    def generate(prompt):
        start = time.perf_counter()
        inputs = tokenizer(prompt, return_tensors="pt", truncation=False)
        if inputs.input_ids.shape[1] > 512:
            raise ValueError("input exceeds the evaluation's 512-token limit")
        with torch.inference_mode():
            ids = model.generate(
                **inputs, do_sample=False, num_beams=1, max_new_tokens=512,
                use_cache=True,
            )[0]
        output = tokenizer.decode(ids, skip_special_tokens=True).strip()
        elapsed_ms = (time.perf_counter() - start) * 1000
        ended = ids[-1].item() == tokenizer.eos_token_id
        return {
            "output": output,
            "ms": round(elapsed_ms, 3),
            "input_tokens": inputs.input_ids.shape[1],
            "output_tokens": len(ids) - 1,
            "ended_with_eos": ended,
            "unknown_input_tokens": (inputs.input_ids == tokenizer.unk_token_id).sum().item(),
            "tokenizer_roundtrip": tokenizer.decode(inputs.input_ids[0], skip_special_tokens=False),
            "rss_kib": memory_kib("VmRSS"),
            "peak_rss_kib": memory_kib("VmHWM"),
            **({"error": "empty or incomplete generation"} if not output or not ended else {}),
        }

    warmup = generate(WARMUP)
    metadata = {
        "model": MODEL, "revision": REVISION, "weights_sha256": WEIGHTS_SHA256,
        "weights_bytes": (source / "model.safetensors").stat().st_size,
        "parameter_count": sum(p.numel() for p in model.parameters()),
        "device": "cpu", "dtype": "float32", "threads": args.threads,
        "inter_op_threads": 1, "seed": 42, "do_sample": False, "num_beams": 1,
        "max_new_tokens": 512, "python": platform.python_version(),
        "packages": {name: importlib.metadata.version(name) for name in (
            "torch", "transformers", "sentencepiece", "tokenizers", "safetensors", "huggingface-hub"
        )},
        "corpus_sha256": sha256(args.cases),
        "model_file_sha256": {p.name: sha256(p) for p in sorted(source.iterdir())
                              if p.is_file() and p.name != "model.safetensors"},
        "verification_ms": round(verification_ms, 3), "load_ms": round(load_ms, 3),
        "warmup": warmup,
    }
    args.metadata.parent.mkdir(parents=True, exist_ok=True)
    args.metadata.write_text(json.dumps(metadata, ensure_ascii=False, indent=2) + "\n")
    failures = 0
    for index, case in enumerate(cases, 1):
        prompt = case["coedit_instruction"].rstrip(": ") + ": " + case["text"]
        row = {
            "id": case["id"], "model": MODEL, "revision": REVISION,
            "mode": case["mode"], "expected_language": case["language"],
            "input": case["text"], "prompt": prompt, "first_request_includes_load": False,
        }
        try:
            row.update(generate(prompt))
        except Exception as error:
            row["error"] = str(error)
        failures += int("error" in row)
        print(json.dumps(row, ensure_ascii=False), flush=True)
        print(f"{index}/{len(cases)} {case['id']}: {row.get('ms', 'error')} ms", file=sys.stderr, flush=True)
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
