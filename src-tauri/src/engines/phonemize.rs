//! Text → IPA phonemes via piper-phonemize (espeak-ng). sherpa-onnx 2.x no longer phonemizes,
//! so every Kokoro/Piper request passes through here. espeak-ng is not thread-safe: serialize.

use super::EngineError;
use std::sync::Mutex;

static LOCK: Mutex<()> = Mutex::new(());

/// Phonemize one chunk. Sentences espeak finds inside the chunk are joined with a space so the
/// chunk stays one synthesis unit (our segmentation already bounds chunk size).
pub fn to_codepoints(text: &str, espeak_voice: &str) -> Result<Vec<i32>, EngineError> {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let result = piper_phonemize::phonemize(text, espeak_voice)
        .or_else(|| {
            log::warn!("espeak voice '{espeak_voice}' unavailable; falling back to en-us");
            piper_phonemize::phonemize(text, "en-us")
        })
        .ok_or_else(|| EngineError::Failed("phonemizer failed".into()))?;
    let mut out: Vec<i32> = Vec::new();
    for i in 0..result.num_sentences() {
        if let Some(s) = result.get_phonemes_as_string(i) {
            if !out.is_empty() {
                out.push(' ' as i32);
            }
            out.extend(s.chars().map(|c| c as i32));
        }
    }
    if out.is_empty() {
        return Err(EngineError::Failed("text produced no phonemes".into()));
    }
    Ok(out)
}

/// Warm up espeak-ng (first call extracts its data files; ~400 ms).
pub fn warm_up() {
    let _ = to_codepoints("hello", "en-us");
}

/// espeak voice for a Kokoro voice id. First letter = language, second = gender.
/// Names verified against the embedded espeak-ng data ("en" is British RP; "en-gb" does not exist).
pub fn espeak_voice_for_kokoro(voice_id: &str) -> &'static str {
    match voice_id.chars().next() {
        Some('a') => "en-us",
        Some('b') => "en",
        Some('e') => "es",
        Some('f') => "fr",
        Some('h') => "hi",
        Some('i') => "it",
        Some('j') => "ja",
        Some('p') => "pt-br",
        Some('z') => "cmn",
        _ => "en-us",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phonemizes_english() {
        let cps = to_codepoints("Hello world.", "en-us").unwrap();
        let s: String = cps.iter().map(|&c| char::from_u32(c as u32).unwrap()).collect();
        assert!(s.contains("həl"), "{s}");
    }

    #[test]
    fn every_kokoro_language_phonemizes() {
        for prefix in ["a", "b", "e", "f", "h", "i", "j", "p", "z"] {
            let v = espeak_voice_for_kokoro(prefix);
            assert!(to_codepoints("Hello world.", v).is_ok(), "voice {v} failed");
        }
    }

    #[test]
    fn handles_oov_words() {
        // The whole point of phonemizing ourselves: made-up words still get spoken.
        let cps = to_codepoints("LangGraph Kubernetes flibbertigibbet", "en-us").unwrap();
        assert!(cps.len() > 20);
    }
}
