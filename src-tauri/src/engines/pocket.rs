//! Plus profile: Pocket TTS (Kyutai) via sherpa-onnx. Every voice is a short reference recording;
//! "custom voices" are just more recordings the user added with consent.

use super::{Engine, EngineError, Pcm, capabilities_for, sherpa::{SherpaTts, num_threads}};
use serde::{Deserialize, Serialize};
use sherpa_onnx::{GenerationConfig, OfflineTtsModelConfig, OfflineTtsPocketModelConfig};
use speechhub_protocol::{EngineInfo, VoiceInfo};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, atomic::AtomicBool},
};

/// Reference clips longer than this are trimmed (Kyutai recommends ~10 s).
const MAX_REFERENCE_SECS: f32 = 30.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomVoice {
    pub id: String,
    pub name: String,
    /// WAV file name inside the voices dir.
    pub file: String,
    pub consent: bool,
    pub created_at: u64,
}

/// Where user-added voices live: `<app data>/voices/{voices.json, *.wav}`.
#[derive(Clone)]
pub struct VoiceLibrary {
    pub dir: PathBuf,
}

impl VoiceLibrary {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }
    fn index_path(&self) -> PathBuf {
        self.dir.join("voices.json")
    }
    pub fn list(&self) -> Vec<CustomVoice> {
        fs::read(self.index_path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }
    fn save(&self, list: &[CustomVoice]) -> Result<(), String> {
        fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        let tmp = self.index_path().with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_vec_pretty(list).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        fs::rename(tmp, self.index_path()).map_err(|e| e.to_string())
    }

    /// Import a recording. Validated and re-encoded as mono 16-bit WAV, trimmed to 30 s.
    pub fn add(&self, source_wav: &Path, name: &str, consent: bool) -> Result<CustomVoice, String> {
        if !consent {
            return Err("you must confirm you have the speaker's consent".into());
        }
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 60 {
            return Err("give the voice a name (1–60 characters)".into());
        }
        let (samples, rate) = read_reference(source_wav)?;
        let secs = samples.len() as f32 / rate as f32;
        if secs < 2.0 {
            return Err(format!("the recording is only {secs:.1}s; use at least 2 seconds of clear speech (5–15 is best)"));
        }
        fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        let id = format!("custom-{}", uuid::Uuid::new_v4());
        let file = format!("{id}.wav");
        let spec = hound::WavSpec { channels: 1, sample_rate: rate, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
        let mut w = hound::WavWriter::create(self.dir.join(&file), spec).map_err(|e| e.to_string())?;
        for s in &samples {
            w.write_sample((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16).map_err(|e| e.to_string())?;
        }
        w.finalize().map_err(|e| e.to_string())?;
        let voice = CustomVoice {
            id,
            name: name.to_string(),
            file,
            consent,
            created_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0),
        };
        let mut list = self.list();
        list.push(voice.clone());
        self.save(&list)?;
        Ok(voice)
    }

    pub fn remove(&self, id: &str) -> Result<(), String> {
        let mut list = self.list();
        if let Some(v) = list.iter().find(|v| v.id == id) {
            let _ = fs::remove_file(self.dir.join(&v.file));
        }
        list.retain(|v| v.id != id);
        self.save(&list)
    }
}

/// Decode a WAV to mono f32, trimmed to MAX_REFERENCE_SECS.
pub fn read_reference(path: &Path) -> Result<(Vec<f32>, u32), String> {
    let mut reader = hound::WavReader::open(path).map_err(|e| format!("not a readable WAV file: {e}"))?;
    let spec = reader.spec();
    let channels = spec.channels.max(1) as usize;
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => {
            let max = (1u64 << (spec.bits_per_sample - 1)) as f32;
            reader.samples::<i32>().filter_map(Result::ok).map(|s| s as f32 / max).collect()
        }
        hound::SampleFormat::Float => reader.samples::<f32>().filter_map(Result::ok).collect(),
    };
    let mut mono: Vec<f32> = if channels == 1 { samples } else { samples.chunks(channels).map(|c| c.iter().sum::<f32>() / channels as f32).collect() };
    let max_len = (MAX_REFERENCE_SECS * spec.sample_rate as f32) as usize;
    mono.truncate(max_len);
    if mono.is_empty() {
        return Err("the WAV file has no audio".into());
    }
    Ok((mono, spec.sample_rate))
}

pub struct PocketEngine {
    tts: SherpaTts,
    model_dir: PathBuf,
    library: VoiceLibrary,
    /// Decoded reference audio per voice id.
    cache: Mutex<HashMap<String, Arc<(Vec<f32>, u32)>>>,
}

impl PocketEngine {
    pub fn load(dir: &Path, library: VoiceLibrary) -> Result<Self, EngineError> {
        let p = |f: &str| Some(dir.join(f).display().to_string());
        if !dir.join("lm_main.int8.onnx").exists() {
            return Err(EngineError::NotInstalled("plus".into()));
        }
        let model = OfflineTtsModelConfig {
            pocket: OfflineTtsPocketModelConfig {
                lm_flow: p("lm_flow.int8.onnx"),
                lm_main: p("lm_main.int8.onnx"),
                encoder: p("encoder.onnx"),
                decoder: p("decoder.int8.onnx"),
                text_conditioner: p("text_conditioner.onnx"),
                vocab_json: p("vocab.json"),
                token_scores_json: p("token_scores.json"),
                voice_embedding_cache_capacity: 8,
            },
            num_threads: num_threads(),
            provider: Some("cpu".into()),
            ..Default::default()
        };
        let tts = SherpaTts::create(model)?;
        Ok(Self { tts, model_dir: dir.to_path_buf(), library, cache: Mutex::new(HashMap::new()) })
    }

    /// Sample voices shipped inside the model archive (`voices/*.wav`).
    fn sample_voices(&self) -> Vec<(String, String, PathBuf)> {
        let dir = self.model_dir.join("voices");
        let mut v: Vec<(String, String, PathBuf)> = fs::read_dir(&dir)
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| p.extension().map(|x| x == "wav").unwrap_or(false))
                    .map(|p| {
                        let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                        (format!("sample-{stem}"), pretty_sample_name(&stem), p)
                    })
                    .collect()
            })
            .unwrap_or_default();
        v.sort_by(|a, b| a.1.cmp(&b.1));
        v
    }

    fn reference_for(&self, voice_id: &str) -> Result<Arc<(Vec<f32>, u32)>, EngineError> {
        if let Some(r) = self.cache.lock().unwrap().get(voice_id) {
            return Ok(r.clone());
        }
        let path = if let Some(v) = self.library.list().into_iter().find(|v| v.id == voice_id) {
            self.library.dir.join(v.file)
        } else if let Some((_, _, p)) = self.sample_voices().into_iter().find(|(id, _, _)| id == voice_id) {
            p
        } else {
            // Unknown voice: first sample voice.
            self.sample_voices().into_iter().next().map(|(_, _, p)| p).ok_or_else(|| EngineError::Failed("no voices available for Plus; add one in Voices".into()))?
        };
        let decoded = Arc::new(read_reference(&path).map_err(EngineError::Failed)?);
        self.cache.lock().unwrap().insert(voice_id.to_string(), decoded.clone());
        Ok(decoded)
    }
}

/// "bria" → "Bria", "sample_fr_hibiki_crepes" → "Hibiki Crepes (French)".
fn pretty_sample_name(stem: &str) -> String {
    let mut lang: Option<&str> = None;
    let words: Vec<String> = stem
        .split(['_', '-'])
        .filter(|w| !w.is_empty() && *w != "sample")
        .filter(|w| match *w {
            "fr" => { lang = Some("French"); false }
            "en" => { lang = Some("English"); false }
            "de" => { lang = Some("German"); false }
            "es" => { lang = Some("Spanish"); false }
            _ => true,
        })
        .map(|w| { let mut c = w.chars(); c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default() })
        .collect();
    match lang {
        Some(l) => format!("{} ({l})", words.join(" ")),
        None => words.join(" "),
    }
}

impl Engine for PocketEngine {
    fn info(&self) -> EngineInfo {
        EngineInfo { id: "plus".into(), name: "Pocket TTS".into(), loaded: true, capabilities: capabilities_for("plus") }
    }

    fn voices(&self) -> Vec<VoiceInfo> {
        let mut out: Vec<VoiceInfo> = self
            .library
            .list()
            .into_iter()
            .map(|v| VoiceInfo { id: v.id, name: v.name, language: "en".into(), engine_id: "plus".into(), gender: None, custom: true })
            .collect();
        out.extend(self.sample_voices().into_iter().map(|(id, name, _)| VoiceInfo {
            id,
            language: if name.contains("(French)") { "fr".into() } else { "en".into() },
            name: format!("{name} · sample"),
            engine_id: "plus".into(),
            gender: None,
            custom: false,
        }));
        out
    }

    fn default_voice(&self) -> String {
        self.library.list().first().map(|v| v.id.clone()).or_else(|| self.sample_voices().first().map(|(id, _, _)| id.clone())).unwrap_or_default()
    }

    fn synthesize(&self, text: &str, voice_id: &str, speed: f32, cancel: &Arc<AtomicBool>) -> Result<Pcm, EngineError> {
        let reference = self.reference_for(voice_id)?;
        let config = GenerationConfig {
            speed: speed.clamp(0.5, 3.0),
            reference_audio: Some(reference.0.clone()),
            reference_sample_rate: reference.1 as i32,
            ..Default::default()
        };
        self.tts.generate(text, config, cancel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine_wav(path: &Path, secs: f32) {
        let spec = hound::WavSpec { channels: 2, sample_rate: 16000, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
        let mut w = hound::WavWriter::create(path, spec).unwrap();
        for i in 0..(16000.0 * secs) as usize {
            let v = ((i as f32 / 16000.0 * 440.0 * std::f32::consts::TAU).sin() * 8000.0) as i16;
            w.write_sample(v).unwrap();
            w.write_sample(v).unwrap();
        }
        w.finalize().unwrap();
    }

    #[test]
    fn library_add_requires_consent_and_length() {
        let dir = std::env::temp_dir().join(format!("speechhub-voices-{}", uuid::Uuid::new_v4()));
        let lib = VoiceLibrary::new(dir.clone());
        let wav = dir.join("in.wav");
        fs::create_dir_all(&dir).unwrap();
        sine_wav(&wav, 5.0);
        assert!(lib.add(&wav, "Me", false).is_err());
        assert!(lib.add(&wav, "", true).is_err());
        let v = lib.add(&wav, "Me", true).unwrap();
        assert_eq!(lib.list().len(), 1);
        let (mono, rate) = read_reference(&dir.join(&v.file)).unwrap();
        assert_eq!(rate, 16000);
        assert!((mono.len() as f32 / 16000.0 - 5.0).abs() < 0.01, "re-encoded as mono, same length");
        let short = dir.join("short.wav");
        sine_wav(&short, 1.0);
        assert!(lib.add(&short, "Short", true).unwrap_err().contains("at least 2 seconds"));
        lib.remove(&v.id).unwrap();
        assert!(lib.list().is_empty());
        assert!(!dir.join(&v.file).exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn sample_names_are_pretty() {
        assert_eq!(pretty_sample_name("bria"), "Bria");
        assert_eq!(pretty_sample_name("sample_fr_hibiki_crepes"), "Hibiki Crepes (French)");
    }

    #[test]
    fn long_references_are_trimmed() {
        let dir = std::env::temp_dir().join(format!("speechhub-voices-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let wav = dir.join("long.wav");
        sine_wav(&wav, 45.0);
        let (mono, rate) = read_reference(&wav).unwrap();
        assert_eq!(mono.len(), (MAX_REFERENCE_SECS * rate as f32) as usize);
        let _ = fs::remove_dir_all(dir);
    }
}
