# Model and runtime licenses

Speech Hub itself is GPL-3.0-or-later. Everything it bundles or downloads is listed here with its
own license. Verified 2026-10-24 against the files actually shipped.

| Component | Where | License | Notes |
|---|---|---|---|
| **Kokoro-82M v1.0** (Standard) | bundled `resources/models/standard` | Apache-2.0 | ONNX export redistributed by sherpa-onnx (`kokoro-multi-lang-v1_0`). Upstream: hexgrad/Kokoro-82M. fp32 chosen over int8: int8 is ~3x slower on Apple Silicon CPUs. |
| **Piper voices** amy / ryan / lessac, medium int8 (Lite) | bundled `resources/models/lite` | Piper: MIT. Voice datasets: see each `MODEL_CARD` (amy: Mimic3 voices; lessac/ryan: public LJSpeech-style corpora) | ONNX redistributed by sherpa-onnx. |
| **Pocket TTS** int8 (Plus) | downloaded on demand to `~/Library/Application Support/com.jeremym.speechhub/models/plus` | CC-BY-4.0 | Kyutai weights are gated on Hugging Face and carry a prohibited-use notice ("voice impersonation or cloning without explicit and lawful consent"). The sherpa-onnx README for this archive says "non-commercial"; the actual LICENSE file in the archive is plain CC-BY-4.0. We show the license and the consent requirement before downloading, and never bundle it. |
| **sherpa-onnx 2.0.0** | statically linked | Apache-2.0 | Includes ONNX Runtime (MIT). As of 2.0.0 sherpa-onnx no longer contains espeak-ng. |
| **piper-phonemize** (+ espeak-ng, ucd) | statically linked | **GPL-3.0-or-later** | This is why the project is GPL. Text → IPA for Kokoro and Piper. espeak-ng data files are embedded in the binary and extracted to a temp dir on first use. |
| cpal, hound, Tauri, React, etc. | linked / bundled | MIT / Apache-2.0 | Standard permissive deps. |

## Attribution text (shown in About → Licenses)

- Kokoro-82M: © hexgrad, Apache License 2.0.
- Piper: © Michael Hansen (rhasspy), MIT License.
- Pocket TTS: © Kyutai, CC BY 4.0. Changes: converted to ONNX (KevinAHM), quantized to int8 (sherpa-onnx).
- espeak-ng: © Jonathan Duddington, Reece Dunn and contributors, GPL-3.0-or-later.
- sherpa-onnx: © Xiaomi Corporation and k2-fsa contributors, Apache License 2.0.
- ONNX Runtime: © Microsoft, MIT License.

## Rules

- Nothing is downloaded without an explicit click. Lite and Standard are inside the app.
- Downloads are verified by SHA-256 (see `scripts/fetch-models.sh` and `src-tauri/src/engines/download.rs`).
- Custom voices created with Plus stay on the user's disk. They are never uploaded anywhere.
