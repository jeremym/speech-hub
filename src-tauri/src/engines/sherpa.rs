//! Shared sherpa-onnx wrapper used by the Kokoro and Piper engines.

use super::{EngineError, Pcm};
use sherpa_onnx::{GenerationConfig, OfflineTts, OfflineTtsConfig, OfflineTtsModelConfig};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

pub struct SherpaTts {
    tts: OfflineTts,
    /// Serializes generate() calls: a cancelled job may still be inside the engine when the
    /// next one starts, and we don't want two inferences fighting for the CPU.
    gate: Mutex<()>,
    pub rate: u32,
}

/// Threads for ONNX Runtime: half the cores, clamped to 2..=8 (M1: 4, M3 Ultra: 8).
pub fn num_threads() -> i32 {
    let n = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    ((n / 2).clamp(2, 8)) as i32
}

impl SherpaTts {
    pub fn create(model: OfflineTtsModelConfig) -> Result<Self, EngineError> {
        let config = OfflineTtsConfig { model, max_num_sentences: 1, silence_scale: 0.2, ..Default::default() };
        let tts = OfflineTts::create(&config)
            .ok_or_else(|| EngineError::Failed("sherpa-onnx could not load the model (see log)".into()))?;
        let rate = tts.sample_rate().max(1) as u32;
        Ok(Self { tts, gate: Mutex::new(()), rate })
    }

    pub fn num_speakers(&self) -> i32 {
        self.tts.num_speakers()
    }

    /// Synthesize pre-phonemized text. Cancellation is checked between engine callbacks.
    pub fn generate_phonemes(
        &self,
        text_for_debug: &str,
        codepoints: Vec<i32>,
        sid: i32,
        speed: f32,
        cancel: &Arc<AtomicBool>,
    ) -> Result<Pcm, EngineError> {
        let config = GenerationConfig {
            sid,
            speed: speed.clamp(0.5, 3.0),
            phoneme_codepoints: Some(vec![codepoints]),
            ..Default::default()
        };
        self.generate(text_for_debug, config, cancel)
    }

    pub fn generate(&self, text: &str, config: GenerationConfig, cancel: &Arc<AtomicBool>) -> Result<Pcm, EngineError> {
        let _gate = self.gate.lock().unwrap_or_else(|e| e.into_inner());
        if cancel.load(Ordering::SeqCst) {
            return Err(EngineError::Cancelled);
        }
        let flag = cancel.clone();
        let audio = self
            .tts
            .generate_with_config(text, &config, Some(move |_chunk: &[f32], _progress: f32| !flag.load(Ordering::Relaxed)));
        if cancel.load(Ordering::SeqCst) {
            return Err(EngineError::Cancelled);
        }
        let audio = audio.ok_or_else(|| EngineError::Failed("synthesis returned no audio".into()))?;
        Ok(Pcm { rate: audio.sample_rate().max(1) as u32, samples: audio.samples().to_vec() })
    }
}
