#!/usr/bin/env python3
"""Fail packaging if llama.cpp's baseline build enables optional x86 instructions."""
from pathlib import Path

caches = list(Path('target/release/build').glob('llama-cpp-sys-2-*/out/build/CMakeCache.txt'))
assert caches, 'No llama.cpp release CMake cache found'
flags = ['GGML_NATIVE', 'GGML_SSE42', 'GGML_AVX', 'GGML_AVX2', 'GGML_AVX512', 'GGML_FMA', 'GGML_F16C']
for cache in caches:
    text = cache.read_text()
    for flag in flags:
        assert f'{flag}:BOOL=OFF' in text, f'{cache}: {flag} must be OFF for baseline x86_64'
print('llama.cpp CPU flags verified: baseline x86_64')
