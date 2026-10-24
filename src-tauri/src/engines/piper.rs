//! Lite profile: a few single-speaker Piper (VITS, int8) voices via sherpa-onnx.
//! Each voice is its own small model, loaded on first use and kept.

use super::{Engine, EngineError, Pcm, capabilities_for, phonemize, sherpa::{SherpaTts, num_threads}};
use serde::Deserialize;
use sherpa_onnx::{OfflineTtsModelConfig, OfflineTtsVitsModelConfig};
use speechhub_protocol::{EngineInfo, VoiceInfo};
use std::{collections::HashMap, path::{Path, PathBuf}, sync::{Arc, Mutex, atomic::AtomicBool}};

#[derive(Deserialize)]
struct Manifest {
    voices: Vec<ManifestVoice>,
}

#[derive(Deserialize, Clone)]
struct ManifestVoice {
    id: String,
    name: String,
    #[serde(default)]
    gender: Option<String>,
    dir: String,
    stem: String,
}

pub struct PiperEngine {
    root: PathBuf,
    voices: Vec<ManifestVoice>,
    loaded: Mutex<HashMap<String, Arc<SherpaTts>>>,
}

impl PiperEngine {
    pub fn load(dir: &Path) -> Result<Self, EngineError> {
        let manifest = std::fs::read(dir.join("manifest.json")).map_err(|_| EngineError::NotInstalled("lite".into()))?;
        let manifest: Manifest = serde_json::from_slice(&manifest).map_err(|e| EngineError::Failed(format!("lite manifest: {e}")))?;
        if manifest.voices.is_empty() {
            return Err(EngineError::NotInstalled("lite".into()));
        }
        phonemize::warm_up();
        let engine = Self { root: dir.to_path_buf(), voices: manifest.voices, loaded: Mutex::new(HashMap::new()) };
        // Load the default voice eagerly so the first request is fast.
        let first = engine.voices[0].id.clone();
        engine.tts_for(&first)?;
        Ok(engine)
    }

    fn tts_for(&self, voice_id: &str) -> Result<Arc<SherpaTts>, EngineError> {
        if let Some(t) = self.loaded.lock().unwrap().get(voice_id) {
            return Ok(t.clone());
        }
        let v = self.voices.iter().find(|v| v.id == voice_id).unwrap_or(&self.voices[0]);
        let dir = self.root.join(&v.dir);
        let model = OfflineTtsModelConfig {
            vits: OfflineTtsVitsModelConfig {
                model: Some(dir.join(format!("{}.onnx", v.stem)).display().to_string()),
                tokens: Some(dir.join("tokens.txt").display().to_string()),
                ..Default::default()
            },
            num_threads: num_threads().min(4),
            provider: Some("cpu".into()),
            ..Default::default()
        };
        let tts = Arc::new(SherpaTts::create(model)?);
        self.loaded.lock().unwrap().insert(v.id.clone(), tts.clone());
        Ok(tts)
    }
}

impl Engine for PiperEngine {
    fn info(&self) -> EngineInfo {
        EngineInfo { id: "lite".into(), name: "Piper".into(), loaded: true, capabilities: capabilities_for("lite") }
    }
    fn voices(&self) -> Vec<VoiceInfo> {
        self.voices
            .iter()
            .map(|v| VoiceInfo {
                id: v.id.clone(),
                name: v.name.clone(),
                language: "en-US".into(),
                engine_id: "lite".into(),
                gender: v.gender.clone(),
                custom: false,
            })
            .collect()
    }
    fn default_voice(&self) -> String {
        self.voices[0].id.clone()
    }
    fn synthesize(&self, text: &str, voice_id: &str, speed: f32, cancel: &Arc<AtomicBool>) -> Result<Pcm, EngineError> {
        let tts = self.tts_for(voice_id)?;
        let cps = phonemize::to_codepoints(text, "en-us")?;
        tts.generate_phonemes(text, cps, 0, speed, cancel)
    }
}
