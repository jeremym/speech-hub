# Performance (measured)

All numbers are real measurements from `cargo run --release -p speech-hub --example bench`.
Add a row when you measure on a new machine; never type a number you didn't see.

## 2026-10-24 · Apple M3 Ultra (20P+8E), macOS 26.6, 96 GB · release build · ORT threads = 8

Benchmark text: 4 chunks, 285 characters, ~16 s of speech.

| Profile | Model | Load | First audio | RTF | RSS after load | RSS peak |
|---|---|---|---|---|---|---|
| Standard | Kokoro-82M v1.0 fp32 | 371 ms | 340 ms | **0.13** | 563 MB | 729 MB |
| Lite | Piper amy medium int8 | 637 ms | 380 ms | **0.105** | 85 MB | 247 MB |
| Plus | Pocket TTS int8, sample voice "bria" | 297 ms | 261 ms | **0.095** | 373 MB | 636 MB |

First audio = time from request to the first chunk (one sentence, ~45 chars) being ready to play.
RTF = synthesis time ÷ audio duration (lower is better; 0.13 = 7.7× faster than real time).

### Why fp32 for Kokoro, not int8

Same machine, same text, standalone spike (`max_num_sentences=1`, whole text as one unit):

| Variant | Threads | RTF |
|---|---|---|
| Kokoro int8 | 2 | 0.74 |
| Kokoro int8 | 4 | 0.59 |
| Kokoro fp32 | 4 | 0.20 |
| Kokoro fp32 | 8 | 0.13 |

Int8 is ~3× slower on Apple Silicon CPUs under ONNX Runtime; on an M1 with 4 threads it would
hover near real time. fp32 costs 212 MB more on disk and ~300 MB more RSS. We took the size hit.

### Pocket TTS (Plus)

Fastest of the three and clones voices from a 10-second clip. Optional only because of licensing
(CC-BY-4.0 with a consent clause, gated upstream). Reference-audio embedding is cached per voice by
sherpa-onnx, so the first request with a new voice pays a little extra.

## Open measurements

- M1 8 GB: expected Standard RTF ≈ 0.3–0.4 with 4 threads. Needs a real run.
- Idle memory after unload (settings → "Unload the model when idle").
- Cold start of the `.app` to first tray icon.
