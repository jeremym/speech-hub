#!/usr/bin/env bash
# Fetch the bundled speech models into src-tauri/resources/models.
#
#   scripts/fetch-models.sh            # Lite + Standard (what the app bundles)
#   scripts/fetch-models.sh --plus     # also Pocket TTS (dev only; the app downloads it at runtime)
#
# Archives are cached in .cache/models (or $SPEECH_HUB_MODEL_CACHE) and verified by SHA-256.
# Files not needed at runtime (espeak-ng-data, Chinese dicts, lexicons) are not copied:
# phonemization happens in-process via piper-phonemize, so sherpa-onnx 2.x never reads them.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
DEST="$ROOT/src-tauri/resources/models"
CACHE="${SPEECH_HUB_MODEL_CACHE:-$ROOT/.cache/models}"
BASE="https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models"
WANT_PLUS=0
[ "${1:-}" = "--plus" ] && WANT_PLUS=1
mkdir -p "$CACHE" "$DEST"

fetch() { # name sha256
  local name=$1 sha=$2 f="$CACHE/$1.tar.bz2"
  if [ ! -f "$f" ]; then
    echo "↓ $name"
    curl -fL --progress-bar -o "$f.part" "$BASE/$name.tar.bz2" && mv "$f.part" "$f"
  fi
  if ! echo "$sha  $f" | shasum -a 256 -c - >/dev/null 2>&1; then
    echo "✗ checksum mismatch: $name" >&2; rm -f "$f"; exit 1
  fi
  echo "✓ $name"
}

extract() { # archive-name dest-dir files...
  local name=$1 dest=$2; shift 2
  local tmp; tmp=$(mktemp -d)
  tar xjf "$CACHE/$name.tar.bz2" -C "$tmp" --strip-components=1
  mkdir -p "$dest"
  for f in "$@"; do cp "$tmp/$f" "$dest/"; done
  rm -rf "$tmp"
}

# ---------------------------------------------------------------- Standard: Kokoro 82M v1.0 (fp32)
# int8 is 3x slower than fp32 on Apple Silicon CPUs (measured, see docs/perf.md), so we ship fp32.
KOKORO=kokoro-multi-lang-v1_0
fetch $KOKORO c5f7e2d2caf082bc1d20fb70334a61d99d20b484500aad32e7cf84c128ea3298
if [ ! -f "$DEST/standard/model.onnx" ]; then
  extract $KOKORO "$DEST/standard" model.onnx voices.bin tokens.txt LICENSE README.md
fi
cat > "$DEST/standard/manifest.json" <<JSON
{ "profile": "standard", "engine": "kokoro", "model": "Kokoro-82M v1.0 (fp32)", "sampleRate": 24000,
  "source": "$BASE/$KOKORO.tar.bz2", "upstream": "https://huggingface.co/hexgrad/Kokoro-82M", "license": "Apache-2.0" }
JSON

# ---------------------------------------------------------------- Lite: three Piper voices (int8)
declare -a LITE=(
  "vits-piper-en_US-amy-medium-int8 bd23c0aa629eb3719448582f45ede49e8fa6a679061fed5eab16a6a6fd8e7e82 amy en_US-amy-medium"
  "vits-piper-en_US-ryan-medium-int8 376eb489d42e98cb49f0e13a633e8d88580bd247ed8aacc298a733210404f771 ryan en_US-ryan-medium"
  "vits-piper-en_US-lessac-medium-int8 f1c6d0295cf16087b05f80fdca5b44daca5cd78e2c425d419a42ba34929805f9 lessac en_US-lessac-medium"
)
for entry in "${LITE[@]}"; do
  read -r name sha voice stem <<<"$entry"
  fetch "$name" "$sha"
  if [ ! -f "$DEST/lite/$voice/$stem.onnx" ]; then
    extract "$name" "$DEST/lite/$voice" "$stem.onnx" "$stem.onnx.json" tokens.txt MODEL_CARD
  fi
done
cat > "$DEST/lite/manifest.json" <<JSON
{ "profile": "lite", "engine": "piper", "model": "Piper VITS medium (int8)", "sampleRate": 22050,
  "voices": [ { "id": "amy", "name": "Amy", "gender": "F", "dir": "amy", "stem": "en_US-amy-medium" },
              { "id": "ryan", "name": "Ryan", "gender": "M", "dir": "ryan", "stem": "en_US-ryan-medium" },
              { "id": "lessac", "name": "Lessac", "gender": "M", "dir": "lessac", "stem": "en_US-lessac-medium" } ],
  "upstream": "https://github.com/rhasspy/piper", "license": "MIT (Piper) / see MODEL_CARD per voice" }
JSON

# ---------------------------------------------------------------- Plus (optional, dev convenience)
if [ $WANT_PLUS = 1 ]; then
  POCKET=sherpa-onnx-pocket-tts-int8-2026-01-26
  fetch $POCKET 2f3b88823cbbb9bf0b2477ec8ae7b3fec417b3a87b6bb5f256dba66f2ad967cb
  if [ ! -f "$DEST/plus/lm_main.int8.onnx" ]; then
    extract $POCKET "$DEST/plus" lm_flow.int8.onnx lm_main.int8.onnx encoder.onnx decoder.int8.onnx text_conditioner.onnx vocab.json token_scores.json LICENSE README.md
    mkdir -p "$DEST/plus/voices" && tmp=$(mktemp -d) && tar xjf "$CACHE/$POCKET.tar.bz2" -C "$tmp" --strip-components=1 && cp "$tmp"/test_wavs/*.wav "$DEST/plus/voices/" && rm -rf "$tmp"
  fi
  cat > "$DEST/plus/manifest.json" <<JSON
{ "profile": "plus", "engine": "pocket", "model": "Pocket TTS (int8)", "sampleRate": 24000,
  "source": "$BASE/$POCKET.tar.bz2", "upstream": "https://huggingface.co/kyutai/pocket-tts", "license": "CC-BY-4.0" }
JSON
fi

echo
du -sh "$DEST"/* 2>/dev/null
echo "Models ready in $DEST"
