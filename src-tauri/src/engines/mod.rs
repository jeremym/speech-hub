//! Engine abstraction. The hub only ever talks to `dyn Engine`.

use speechhub_protocol::{EngineCapabilities, EngineInfo, VoiceInfo};
use serde::Serialize;
use std::{path::Path, sync::atomic::AtomicBool};

#[cfg(feature = "dev-say")]
pub mod dev_say;

/// Mono PCM, f32 in [-1, 1].
pub struct Pcm {
    pub rate: u32,
    pub samples: Vec<f32>,
}

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("engine profile '{0}' is not installed")]
    NotInstalled(String),
    #[error("synthesis cancelled")]
    Cancelled,
    #[error("synthesis failed: {0}")]
    Failed(String),
}

pub trait Engine: Send + Sync {
    fn info(&self) -> EngineInfo;
    fn voices(&self) -> Vec<VoiceInfo>;
    fn default_voice(&self) -> String;
    /// Synthesize one chunk. Must poll `cancel` and return `Cancelled` promptly when set.
    fn synthesize(&self, text: &str, voice_id: &str, speed: f32, cancel: &AtomicBool) -> Result<Pcm, EngineError>;
}

/// What the wizard / Preferences show for each selectable profile (Handy-style model picker).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileCard {
    pub id: String,
    pub name: String,
    pub tagline: String,
    pub size_label: String,
    pub ram_label: String,
    /// 1–5
    pub quality: u8,
    /// 1–5
    pub speed: u8,
    pub voice_cloning: bool,
    pub bundled: bool,
    pub installed: bool,
    pub dev_only: bool,
    pub languages: Vec<String>,
    pub recommended: bool,
}

/// All known profiles, with install state resolved against the models dir.
pub fn catalog(models_dir: &Path, recommended: &str) -> Vec<ProfileCard> {
    let mut cards = vec![
        ProfileCard {
            id: "lite".into(),
            name: "Lite".into(),
            tagline: "Piper voices. Tiny and instant. Runs on any Mac, including Intel.".into(),
            size_label: "~60 MB".into(),
            ram_label: "< 150 MB".into(),
            quality: 2,
            speed: 5,
            voice_cloning: false,
            bundled: true,
            installed: models_dir.join("lite").join("model.onnx").exists(),
            dev_only: false,
            languages: vec!["en".into()],
            recommended: recommended == "lite",
        },
        ProfileCard {
            id: "standard".into(),
            name: "Standard".into(),
            tagline: "Kokoro 82M. Natural voices, CPU only, ~50 voices in 8 languages.".into(),
            size_label: "~120 MB".into(),
            ram_label: "~350 MB".into(),
            quality: 4,
            speed: 4,
            voice_cloning: false,
            bundled: true,
            installed: models_dir.join("standard").join("model.onnx").exists(),
            dev_only: false,
            languages: vec!["en".into(), "es".into(), "fr".into(), "hi".into(), "it".into(), "ja".into(), "pt".into(), "zh".into()],
            recommended: recommended == "standard",
        },
        ProfileCard {
            id: "plus".into(),
            name: "Plus".into(),
            tagline: "Pocket TTS. Clone a voice from a short sample. Separate download, CC-BY-4.0.".into(),
            size_label: "~200 MB download".into(),
            ram_label: "TBD".into(),
            quality: 4,
            speed: 3,
            voice_cloning: true,
            bundled: false,
            installed: models_dir.join("plus").join("model.onnx").exists(),
            dev_only: false,
            languages: vec!["en".into(), "fr".into()],
            recommended: false,
        },
    ];
    #[cfg(feature = "dev-say")]
    cards.push(ProfileCard {
        id: "dev-say".into(),
        name: "macOS system voice (dev placeholder)".into(),
        tagline: "Routes through Apple's `say`. Exists only until the real engines land in Milestone 2.".into(),
        size_label: "0 MB".into(),
        ram_label: "–".into(),
        quality: 1,
        speed: 5,
        voice_cloning: false,
        bundled: true,
        installed: true,
        dev_only: true,
        languages: vec!["many".into()],
        recommended: false,
    });
    cards
}

/// Instantiate the engine for a profile. Falls back to the first installed profile and
/// returns the id actually used, so the UI never lies about what's speaking.
pub fn create(profile: &str, models_dir: &Path) -> Result<(Box<dyn Engine>, String), EngineError> {
    let attempt = |id: &str| -> Result<Box<dyn Engine>, EngineError> {
        match id {
            #[cfg(feature = "dev-say")]
            "dev-say" => Ok(Box::new(dev_say::DevSayEngine::new())),
            "lite" | "standard" | "plus" => Err(EngineError::NotInstalled(id.to_string())),
            other => Err(EngineError::NotInstalled(other.to_string())),
        }
    };
    if let Ok(e) = attempt(profile) {
        return Ok((e, profile.to_string()));
    }
    for card in catalog(models_dir, "") {
        if card.installed && card.id != profile {
            if let Ok(e) = attempt(&card.id) {
                log::warn!("engine profile '{profile}' not installed; using '{}'", card.id);
                return Ok((e, card.id));
            }
        }
    }
    Err(EngineError::NotInstalled(profile.to_string()))
}

pub fn capabilities_for(id: &str) -> EngineCapabilities {
    EngineCapabilities { speed_control: true, voice_cloning: id == "plus" }
}
