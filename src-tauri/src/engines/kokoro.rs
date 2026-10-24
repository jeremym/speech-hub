//! Standard profile: Kokoro-82M (fp32) via sherpa-onnx, phonemized by piper-phonemize.

use super::{Engine, EngineError, Pcm, capabilities_for, phonemize, sherpa::{SherpaTts, num_threads}};
use sherpa_onnx::{OfflineTtsKokoroModelConfig, OfflineTtsModelConfig};
use speechhub_protocol::{EngineInfo, VoiceInfo};
use std::{path::Path, sync::{Arc, atomic::AtomicBool}};

/// Speaker table baked into kokoro-multi-lang-v1_0 (read from the ONNX `speaker_names` metadata).
const SPEAKERS: [&str; 54] = [
    "af_alloy", "af_aoede", "af_bella", "af_heart", "af_jessica", "af_kore", "af_nicole", "af_nova", "af_river",
    "af_sarah", "af_sky", "am_adam", "am_echo", "am_eric", "am_fenrir", "am_liam", "am_michael", "am_onyx", "am_puck",
    "am_santa", "bf_alice", "bf_emma", "bf_isabella", "bf_lily", "bm_daniel", "bm_fable", "bm_george", "bm_lewis",
    "ef_dora", "em_alex", "ff_siwis", "hf_alpha", "hf_beta", "hm_omega", "hm_psi", "if_sara", "im_nicola", "jf_alpha",
    "jf_gongitsune", "jf_nezumi", "jf_tebukuro", "jm_kumo", "pf_dora", "pm_alex", "pm_santa", "zf_xiaobei", "zf_xiaoni",
    "zf_xiaoxiao", "zf_xiaoyi", "zm_yunjian", "zm_yunxi", "zm_yunxia", "zm_yunyang", "em_santa",
];

pub const DEFAULT_VOICE: &str = "af_heart";

pub struct KokoroEngine {
    tts: SherpaTts,
    voices: Vec<VoiceInfo>,
}

impl KokoroEngine {
    pub fn load(dir: &Path) -> Result<Self, EngineError> {
        let p = |f: &str| Some(dir.join(f).display().to_string());
        if !dir.join("model.onnx").exists() {
            return Err(EngineError::NotInstalled("standard".into()));
        }
        let model = OfflineTtsModelConfig {
            kokoro: OfflineTtsKokoroModelConfig {
                model: p("model.onnx"),
                voices: p("voices.bin"),
                tokens: p("tokens.txt"),
                ..Default::default()
            },
            num_threads: num_threads(),
            provider: Some("cpu".into()),
            ..Default::default()
        };
        phonemize::warm_up();
        let tts = SherpaTts::create(model)?;
        let n = (tts.num_speakers().max(0) as usize).min(SPEAKERS.len());
        let mut voices: Vec<VoiceInfo> = SPEAKERS[..n].iter().map(|id| describe(id)).collect();
        voices.sort_by(|a, b| {
            b.language.starts_with("en").cmp(&a.language.starts_with("en")).then(a.language.cmp(&b.language)).then(a.name.cmp(&b.name))
        });
        Ok(Self { tts, voices })
    }

    fn sid(&self, voice_id: &str) -> i32 {
        SPEAKERS.iter().position(|s| *s == voice_id).unwrap_or(3) as i32
    }
}

fn describe(id: &str) -> VoiceInfo {
    let mut chars = id.chars();
    let lang = match chars.next() {
        Some('a') => "en-US",
        Some('b') => "en-GB",
        Some('e') => "es-ES",
        Some('f') => "fr-FR",
        Some('h') => "hi-IN",
        Some('i') => "it-IT",
        Some('j') => "ja-JP",
        Some('p') => "pt-BR",
        Some('z') => "zh-CN",
        _ => "en-US",
    };
    let gender = match chars.next() {
        Some('f') => Some("F".to_string()),
        Some('m') => Some("M".to_string()),
        _ => None,
    };
    let raw = id.split_once('_').map(|(_, n)| n).unwrap_or(id);
    let mut name: String = raw.chars().next().map(|c| c.to_uppercase().collect::<String>()).unwrap_or_default();
    name.push_str(&raw[raw.char_indices().nth(1).map(|(i, _)| i).unwrap_or(raw.len())..]);
    VoiceInfo { id: id.to_string(), name, language: lang.into(), engine_id: "standard".into(), gender, custom: false }
}

impl Engine for KokoroEngine {
    fn info(&self) -> EngineInfo {
        EngineInfo { id: "standard".into(), name: "Kokoro 82M".into(), loaded: true, capabilities: capabilities_for("standard") }
    }
    fn voices(&self) -> Vec<VoiceInfo> {
        self.voices.clone()
    }
    fn default_voice(&self) -> String {
        DEFAULT_VOICE.into()
    }
    fn synthesize(&self, text: &str, voice_id: &str, speed: f32, cancel: &Arc<AtomicBool>) -> Result<Pcm, EngineError> {
        let voice = if SPEAKERS.contains(&voice_id) { voice_id } else { DEFAULT_VOICE };
        let cps = phonemize::to_codepoints(text, phonemize::espeak_voice_for_kokoro(voice))?;
        self.tts.generate_phonemes(text, cps, self.sid(voice), speed, cancel)
    }
}
