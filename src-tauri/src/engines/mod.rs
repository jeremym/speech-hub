//! Engine abstraction. The hub only ever talks to `dyn Engine`.
//!
//! Profiles (PLAN.md §4): `lite` = Piper, `standard` = Kokoro, `plus` = Pocket TTS (downloaded).
//! Models live in one of several roots: the app bundle's resources (bundled profiles) and the
//! app data dir (downloaded profiles). `SPEECH_HUB_MODELS_DIR` overrides both (benchmarks, tests).

use speechhub_protocol::{EngineCapabilities, EngineInfo, VoiceInfo};
use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicBool},
};

#[cfg(feature = "dev-say")]
pub mod dev_say;
pub mod kokoro;
pub mod phonemize;
pub mod piper;
pub mod sherpa;

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
    fn synthesize(&self, text: &str, voice_id: &str, speed: f32, cancel: &Arc<AtomicBool>) -> Result<Pcm, EngineError>;
}

/// Where to look for model directories, in priority order.
#[derive(Clone, Debug)]
pub struct ModelRoots {
    pub roots: Vec<PathBuf>,
    /// Where downloaded (non-bundled) profiles are written.
    pub download_root: PathBuf,
}

impl ModelRoots {
    pub fn new(bundled: Option<PathBuf>, download_root: PathBuf) -> Self {
        let mut roots = Vec::new();
        if let Ok(over) = std::env::var("SPEECH_HUB_MODELS_DIR") {
            roots.push(PathBuf::from(over));
        }
        if let Some(b) = bundled {
            roots.push(b);
        }
        roots.push(download_root.clone());
        Self { roots, download_root }
    }

    /// Directory for a profile, if any root has it installed.
    pub fn find(&self, profile: &str) -> Option<PathBuf> {
        self.roots.iter().map(|r| r.join(profile)).find(|d| is_installed(profile, d))
    }

    pub fn is_installed(&self, profile: &str) -> bool {
        self.find(profile).is_some()
    }
}

fn is_installed(profile: &str, dir: &Path) -> bool {
    match profile {
        "standard" => dir.join("model.onnx").exists() && dir.join("voices.bin").exists(),
        "lite" => dir.join("manifest.json").exists(),
        "plus" => dir.join("lm_main.int8.onnx").exists() && dir.join("vocab.json").exists(),
        #[cfg(feature = "dev-say")]
        "dev-say" => true,
        _ => false,
    }
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
    pub license: String,
}

/// All known profiles, with install state resolved against the model roots.
pub fn catalog(roots: &ModelRoots, recommended: &str) -> Vec<ProfileCard> {
    #[allow(unused_mut)]
    let mut cards = vec![
        ProfileCard {
            id: "lite".into(),
            name: "Lite".into(),
            tagline: "Three Piper voices. Tiny and instant. Fine on any Mac, including Intel and 8 GB machines.".into(),
            size_label: "53 MB".into(),
            ram_label: "< 150 MB".into(),
            quality: 2,
            speed: 5,
            voice_cloning: false,
            bundled: true,
            installed: roots.is_installed("lite"),
            dev_only: false,
            languages: vec!["en".into()],
            recommended: recommended == "lite",
            license: "MIT".into(),
        },
        ProfileCard {
            id: "standard".into(),
            name: "Standard".into(),
            tagline: "Kokoro 82M. Natural voices, CPU only. 54 voices across 9 languages.".into(),
            size_label: "337 MB".into(),
            ram_label: "~500 MB".into(),
            quality: 4,
            speed: 4,
            voice_cloning: false,
            bundled: true,
            installed: roots.is_installed("standard"),
            dev_only: false,
            languages: vec!["en".into(), "es".into(), "fr".into(), "hi".into(), "it".into(), "ja".into(), "pt".into(), "zh".into()],
            recommended: recommended == "standard",
            license: "Apache-2.0".into(),
        },
        ProfileCard {
            id: "plus".into(),
            name: "Plus".into(),
            tagline: "Pocket TTS by Kyutai. Very fast, and clones a voice from a 10-second sample. Separate download.".into(),
            size_label: "98 MB download (194 MB installed)".into(),
            ram_label: "~400 MB".into(),
            quality: 4,
            speed: 5,
            voice_cloning: true,
            bundled: false,
            installed: roots.is_installed("plus"),
            dev_only: false,
            languages: vec!["en".into(), "fr".into()],
            recommended: false,
            license: "CC-BY-4.0".into(),
        },
    ];
    #[cfg(feature = "dev-say")]
    cards.push(ProfileCard {
        id: "dev-say".into(),
        name: "macOS system voice (dev placeholder)".into(),
        tagline: "Routes through Apple's `say`. Development only.".into(),
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
        license: "n/a".into(),
    });
    cards
}

/// Instantiate the engine for a profile. Falls back to the first installed profile and
/// returns the id actually used, so the UI never lies about what's speaking.
pub fn create(profile: &str, roots: &ModelRoots) -> Result<(Box<dyn Engine>, String), EngineError> {
    let attempt = |id: &str| -> Result<Box<dyn Engine>, EngineError> {
        let dir = roots.find(id).ok_or_else(|| EngineError::NotInstalled(id.to_string()))?;
        match id {
            "standard" => Ok(Box::new(kokoro::KokoroEngine::load(&dir)?)),
            "lite" => Ok(Box::new(piper::PiperEngine::load(&dir)?)),
            "plus" => Err(EngineError::NotInstalled("plus (arrives in Milestone 6)".into())),
            #[cfg(feature = "dev-say")]
            "dev-say" => Ok(Box::new(dev_say::DevSayEngine::new())),
            other => Err(EngineError::NotInstalled(other.to_string())),
        }
    };
    match attempt(profile) {
        Ok(e) => return Ok((e, profile.to_string())),
        Err(e) => log::warn!("engine '{profile}': {e}"),
    }
    // Fallback order: best bundled engine first.
    for id in ["standard", "lite"] {
        if id != profile && roots.is_installed(id) {
            if let Ok(e) = attempt(id) {
                log::warn!("engine profile '{profile}' unavailable; using '{id}'");
                return Ok((e, id.to_string()));
            }
        }
    }
    Err(EngineError::NotInstalled(profile.to_string()))
}

pub fn capabilities_for(id: &str) -> EngineCapabilities {
    EngineCapabilities { speed_control: true, voice_cloning: id == "plus" }
}
