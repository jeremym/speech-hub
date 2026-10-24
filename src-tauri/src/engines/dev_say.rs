//! DEV-ONLY placeholder engine: renders text to PCM with macOS `say`, so the real
//! segment → synthesize → playback pipeline can be exercised before sherpa-onnx (M2).
//! Compiled only with the `dev-say` feature. Not a product feature.

use super::{Engine, EngineError, Pcm, capabilities_for};
use speechhub_protocol::{EngineInfo, VoiceInfo};
use std::{
    io::Write,
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

pub struct DevSayEngine {
    voices: Vec<VoiceInfo>,
}

impl DevSayEngine {
    pub fn new() -> Self {
        Self { voices: list_say_voices() }
    }
}

fn list_say_voices() -> Vec<VoiceInfo> {
    let out = Command::new("/usr/bin/say").arg("-v").arg("?").output();
    let Ok(out) = out else { return vec![] };
    let text = String::from_utf8_lossy(&out.stdout);
    let mut voices: Vec<VoiceInfo> = text
        .lines()
        .filter_map(|line| {
            let (head, _comment) = line.split_once('#').unwrap_or((line, ""));
            let mut parts = head.split("  ").filter(|p| !p.trim().is_empty());
            let name = parts.next()?.trim().to_string();
            let lang = parts.next()?.trim().to_string();
            Some(VoiceInfo {
                id: name.clone(),
                name,
                language: lang.replace('_', "-"),
                engine_id: "dev-say".into(),
                gender: None,
                custom: false,
            })
        })
        .collect();
    // English first, then alphabetical: a long list is useless in a picker otherwise.
    voices.sort_by(|a, b| {
        b.language.starts_with("en").cmp(&a.language.starts_with("en")).then(a.name.cmp(&b.name))
    });
    voices
}

impl Engine for DevSayEngine {
    fn info(&self) -> EngineInfo {
        EngineInfo {
            id: "dev-say".into(),
            name: "macOS system voice (dev placeholder)".into(),
            loaded: true,
            capabilities: capabilities_for("dev-say"),
        }
    }

    fn voices(&self) -> Vec<VoiceInfo> {
        self.voices.clone()
    }

    fn default_voice(&self) -> String {
        for preferred in ["Samantha", "Alex", "Daniel"] {
            if self.voices.iter().any(|v| v.id == preferred) {
                return preferred.into();
            }
        }
        self.voices.first().map(|v| v.id.clone()).unwrap_or_default()
    }

    fn synthesize(&self, text: &str, voice_id: &str, speed: f32, cancel: &AtomicBool) -> Result<Pcm, EngineError> {
        if cancel.load(Ordering::Relaxed) {
            return Err(EngineError::Cancelled);
        }
        let path = std::env::temp_dir().join(format!("speechhub-devsay-{}.wav", uuid::Uuid::new_v4()));
        let wpm = (175.0 * speed).round() as u32;
        let mut cmd = Command::new("/usr/bin/say");
        if !voice_id.is_empty() {
            cmd.arg("-v").arg(voice_id);
        }
        cmd.arg("-r")
            .arg(wpm.to_string())
            .arg("-o")
            .arg(&path)
            .arg("--file-format=WAVE")
            .arg("--data-format=LEI16@22050")
            .arg("-f")
            .arg("-")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        let mut child = cmd.spawn().map_err(|e| EngineError::Failed(format!("cannot run say: {e}")))?;
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(text.as_bytes());
        }
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => {
                    if cancel.load(Ordering::Relaxed) {
                        let _ = child.kill();
                        let _ = child.wait();
                        let _ = std::fs::remove_file(&path);
                        return Err(EngineError::Cancelled);
                    }
                    std::thread::sleep(Duration::from_millis(15));
                }
                Err(e) => return Err(EngineError::Failed(e.to_string())),
            }
        };
        if !status.success() {
            let _ = std::fs::remove_file(&path);
            return Err(EngineError::Failed(format!("say exited with {status}")));
        }
        let pcm = read_wav(&path);
        let _ = std::fs::remove_file(&path);
        pcm
    }
}

fn read_wav(path: &std::path::Path) -> Result<Pcm, EngineError> {
    let mut reader = hound::WavReader::open(path).map_err(|e| EngineError::Failed(e.to_string()))?;
    let spec = reader.spec();
    let channels = spec.channels.max(1) as usize;
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => {
            let max = (1u32 << (spec.bits_per_sample - 1)) as f32;
            reader.samples::<i32>().filter_map(Result::ok).map(|s| s as f32 / max).collect()
        }
        hound::SampleFormat::Float => reader.samples::<f32>().filter_map(Result::ok).collect(),
    };
    let mono: Vec<f32> = if channels == 1 {
        samples
    } else {
        samples.chunks(channels).map(|c| c.iter().sum::<f32>() / channels as f32).collect()
    };
    Ok(Pcm { rate: spec.sample_rate, samples: mono })
}
