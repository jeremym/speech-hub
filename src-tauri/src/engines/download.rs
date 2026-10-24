//! On-demand model download (Plus profile). Uses the system curl, verifies SHA-256, extracts
//! only the files we need, and reports progress as Tauri events. Nothing runs without a click.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use tauri::{AppHandle, Emitter};

pub const EVENT: &str = "models:download";

pub struct Archive {
    pub profile: &'static str,
    pub url: &'static str,
    pub sha256: &'static str,
    pub approx_bytes: u64,
    /// Files to keep (relative to the archive's top-level dir), plus a dir whose *.wav become sample voices.
    pub keep: &'static [&'static str],
    pub voices_dir: Option<&'static str>,
    pub manifest: &'static str,
}

pub const PLUS: Archive = Archive {
    profile: "plus",
    url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/sherpa-onnx-pocket-tts-int8-2026-01-26.tar.bz2",
    sha256: "2f3b88823cbbb9bf0b2477ec8ae7b3fec417b3a87b6bb5f256dba66f2ad967cb",
    approx_bytes: 98_300_000,
    keep: &["lm_flow.int8.onnx", "lm_main.int8.onnx", "encoder.onnx", "decoder.int8.onnx", "text_conditioner.onnx", "vocab.json", "token_scores.json", "LICENSE", "README.md"],
    voices_dir: Some("test_wavs"),
    manifest: r#"{ "profile": "plus", "engine": "pocket", "model": "Pocket TTS (int8)", "sampleRate": 24000, "upstream": "https://huggingface.co/kyutai/pocket-tts", "license": "CC-BY-4.0" }"#,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub profile: String,
    pub phase: String,
    pub bytes: u64,
    pub total: u64,
    pub message: Option<String>,
}

static IN_FLIGHT: AtomicBool = AtomicBool::new(false);

fn emit(app: &AppHandle, profile: &str, phase: &str, bytes: u64, total: u64, message: Option<String>) {
    let _ = app.emit(EVENT, Progress { profile: profile.into(), phase: phase.into(), bytes, total, message });
}

/// Download + install `archive` into `dest_root/<profile>`. Blocking; call from a thread.
pub fn install(app: &AppHandle, archive: &Archive, dest_root: &Path) -> Result<(), String> {
    let app2 = app.clone();
    install_with(archive, dest_root, move |phase, bytes, total, message| emit(&app2, archive.profile, phase, bytes, total, message))
}

/// Same, with a plain progress callback (testable without Tauri).
pub fn install_with(archive: &Archive, dest_root: &Path, report: impl Fn(&str, u64, u64, Option<String>)) -> Result<(), String> {
    if IN_FLIGHT.swap(true, Ordering::SeqCst) {
        return Err("a download is already running".into());
    }
    let result = install_inner(archive, dest_root, &report);
    IN_FLIGHT.store(false, Ordering::SeqCst);
    match &result {
        Ok(()) => report("done", archive.approx_bytes, archive.approx_bytes, None),
        Err(e) => report("error", 0, archive.approx_bytes, Some(e.clone())),
    }
    result
}

fn install_inner(archive: &Archive, dest_root: &Path, report: &dyn Fn(&str, u64, u64, Option<String>)) -> Result<(), String> {
    let cache = dest_root.join(".downloads");
    fs::create_dir_all(&cache).map_err(|e| e.to_string())?;
    let file = cache.join(format!("{}.tar.bz2", archive.profile));
    let part = cache.join(format!("{}.tar.bz2.part", archive.profile));

    if !file.exists() || sha256_of(&file)? != archive.sha256 {
        let _ = fs::remove_file(&file);
        let total = content_length(archive.url).unwrap_or(archive.approx_bytes);
        report("downloading", 0, total, None);
        let mut child = Command::new("/usr/bin/curl")
            .args(["-fL", "--silent", "--show-error", "--retry", "3", "--proto", "=https", "-o"])
            .arg(&part)
            .arg(archive.url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("cannot start curl: {e}"))?;
        let mut last = 0u64;
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    if !status.success() {
                        let mut err = String::new();
                        if let Some(mut e) = child.stderr.take() {
                            let _ = e.read_to_string(&mut err);
                        }
                        let _ = fs::remove_file(&part);
                        return Err(format!("download failed: {}", err.trim()));
                    }
                    break;
                }
                Ok(None) => {
                    let bytes = fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
                    if bytes != last {
                        last = bytes;
                        report("downloading", bytes, total.max(bytes), None);
                    }
                    std::thread::sleep(Duration::from_millis(300));
                }
                Err(e) => return Err(e.to_string()),
            }
        }
        fs::rename(&part, &file).map_err(|e| e.to_string())?;
    }

    report("verifying", archive.approx_bytes, archive.approx_bytes, None);
    let actual = sha256_of(&file)?;
    if actual != archive.sha256 {
        let _ = fs::remove_file(&file);
        return Err("checksum mismatch; the download was discarded. Try again.".into());
    }

    report("extracting", archive.approx_bytes, archive.approx_bytes, None);
    let tmp = dest_root.join(format!(".extract-{}", archive.profile));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    {
        let f = fs::File::open(&file).map_err(|e| e.to_string())?;
        let bz = bzip2::read::BzDecoder::new(f);
        let mut tar = tar::Archive::new(bz);
        tar.set_preserve_permissions(false);
        tar.unpack(&tmp).map_err(|e| format!("extract failed: {e}"))?;
    }
    // Archives have one top-level directory.
    let top = fs::read_dir(&tmp)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| p.is_dir())
        .ok_or("archive layout not recognized")?;
    let dest = dest_root.join(archive.profile);
    let staging = dest_root.join(format!(".staging-{}", archive.profile));
    let _ = fs::remove_dir_all(&staging);
    fs::create_dir_all(&staging).map_err(|e| e.to_string())?;
    for name in archive.keep {
        let src = top.join(name);
        if src.exists() {
            fs::rename(&src, staging.join(name)).or_else(|_| fs::copy(&src, staging.join(name)).map(|_| ())).map_err(|e| format!("{name}: {e}"))?;
        }
    }
    if let Some(vd) = archive.voices_dir {
        let vsrc = top.join(vd);
        let vdst = staging.join("voices");
        fs::create_dir_all(&vdst).map_err(|e| e.to_string())?;
        if let Ok(rd) = fs::read_dir(&vsrc) {
            for e in rd.filter_map(|e| e.ok()) {
                let p = e.path();
                if p.extension().map(|x| x == "wav").unwrap_or(false) {
                    let _ = fs::rename(&p, vdst.join(p.file_name().unwrap()));
                }
            }
        }
    }
    fs::write(staging.join("manifest.json"), archive.manifest).map_err(|e| e.to_string())?;
    let _ = fs::remove_dir_all(&tmp);
    let _ = fs::remove_dir_all(&dest);
    fs::rename(&staging, &dest).map_err(|e| e.to_string())?;
    let _ = fs::remove_file(&file);
    Ok(())
}

pub fn remove(dest_root: &Path, profile: &str) -> Result<(), String> {
    let dir = dest_root.join(profile);
    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn sha256_of(path: &Path) -> Result<String, String> {
    let mut f = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn content_length(url: &str) -> Option<u64> {
    let out = Command::new("/usr/bin/curl").args(["-sIL", "--max-time", "15", url]).output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout).to_lowercase();
    text.lines().filter_map(|l| l.strip_prefix("content-length:")).last().and_then(|v| v.trim().parse().ok())
}

#[allow(dead_code)]
pub fn is_downloading() -> bool {
    IN_FLIGHT.load(Ordering::SeqCst)
}

#[allow(dead_code)]
pub fn cache_dir(dest_root: &Path) -> PathBuf {
    dest_root.join(".downloads")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exercises verify + extract + staging using a locally cached archive (no network).
    /// Skipped unless SPEECH_HUB_MODEL_CACHE points at a dir containing the Plus tarball.
    #[test]
    fn install_from_cached_archive() {
        let Some(cache) = std::env::var_os("SPEECH_HUB_MODEL_CACHE") else { return };
        let src = PathBuf::from(cache).join("sherpa-onnx-pocket-tts-int8-2026-01-26.tar.bz2");
        if !src.exists() {
            return;
        }
        let root = std::env::temp_dir().join(format!("speechhub-dl-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join(".downloads")).unwrap();
        fs::copy(&src, root.join(".downloads/plus.tar.bz2")).unwrap();
        let phases = std::sync::Mutex::new(Vec::new());
        install_with(&PLUS, &root, |phase, _, _, msg| { phases.lock().unwrap().push(format!("{phase}{}", msg.map(|m| format!(":{m}")).unwrap_or_default())); }).unwrap();
        assert!(root.join("plus/lm_main.int8.onnx").exists());
        assert!(root.join("plus/voices").read_dir().unwrap().count() >= 1);
        assert!(root.join("plus/manifest.json").exists());
        assert!(!root.join(".downloads/plus.tar.bz2").exists(), "archive removed after install");
        let p = phases.lock().unwrap();
        assert!(p.iter().any(|x| x == "verifying") && p.last().unwrap() == "done", "{p:?}");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn sha256_matches_shasum() {
        let p = std::env::temp_dir().join(format!("speechhub-sha-{}.bin", uuid::Uuid::new_v4()));
        fs::write(&p, b"hello world").unwrap();
        assert_eq!(sha256_of(&p).unwrap(), "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9");
        let _ = fs::remove_file(p);
    }
}
