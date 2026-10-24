//! Engine benchmark. Reports real numbers for docs/perf.md.
//!
//!   SPEECH_HUB_MODELS_DIR=src-tauri/resources/models cargo run --release -p speech-hub --example bench -- standard af_heart
//!   cargo run --release -p speech-hub --example bench -- lite amy

use speech_hub_lib::{engines::{self, ModelRoots}, text};
use std::{sync::{Arc, atomic::AtomicBool}, time::Instant};

const TEXT: &str = "Speech Hub gives anything on your Mac a voice. Select some text, press a key, and listen. \
It runs entirely on your computer, with no cloud and no account. Scripts, Shortcuts, and other apps can \
all speak through the same local engine, and a pronunciation you fix once is fixed everywhere.";

fn rss_mb() -> f64 {
    let out = std::process::Command::new("/bin/ps").args(["-o", "rss=", "-p", &std::process::id().to_string()]).output().ok();
    out.and_then(|o| String::from_utf8(o.stdout).ok()).and_then(|s| s.trim().parse::<f64>().ok()).map(|kb| kb / 1024.0).unwrap_or(0.0)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let profile = args.get(1).map(String::as_str).unwrap_or("standard");
    let roots = ModelRoots::new(Some("src-tauri/resources/models".into()), std::env::temp_dir());
    let rss0 = rss_mb();
    let t = Instant::now();
    let (engine, actual) = engines::create(profile, &roots).expect("engine");
    let load_ms = t.elapsed().as_millis();
    let voice = args.get(2).cloned().unwrap_or_else(|| engine.default_voice());
    println!("engine={actual} voice={voice} load={load_ms} ms rss_after_load={:.0} MB (+{:.0})", rss_mb(), rss_mb() - rss0);

    let cancel = Arc::new(AtomicBool::new(false));
    // Warm-up (first ORT run is slower).
    let _ = engine.synthesize("Warming up the engine.", &voice, 1.0, &cancel);

    let chunks = text::segment(TEXT);
    let t = Instant::now();
    let mut first_audio_ms = None;
    let mut audio_secs = 0.0;
    for (i, c) in chunks.iter().enumerate() {
        let t_chunk = Instant::now();
        let pcm = engine.synthesize(c, &voice, 1.0, &cancel).expect("synth");
        if first_audio_ms.is_none() {
            first_audio_ms = Some(t.elapsed().as_millis());
        }
        let secs = pcm.samples.len() as f64 / pcm.rate as f64;
        audio_secs += secs;
        println!("  chunk {i}: {:>3} chars -> {:.2}s audio in {} ms", c.chars().count(), secs, t_chunk.elapsed().as_millis());
    }
    let total = t.elapsed().as_secs_f64();
    println!(
        "chunks={} first_audio={} ms total={:.0} ms audio={:.1}s RTF={:.3} rss_peak={:.0} MB",
        chunks.len(), first_audio_ms.unwrap_or(0), total * 1000.0, audio_secs, total / audio_secs, rss_mb()
    );
}
