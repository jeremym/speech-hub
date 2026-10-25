//! Does the engine's speed parameter actually change duration? Prints seconds of audio per speed.
//! cargo run -p speech-hub --example speedtest -- plus
use std::sync::{Arc, atomic::AtomicBool};

fn main() {
    let profile = std::env::args().nth(1).unwrap_or_else(|| "plus".into());
    let roots = speech_hub_lib::engines::ModelRoots::new(Some("src-tauri/resources/models".into()), std::env::temp_dir());
    let engine = speech_hub_lib::engines::create(&profile, &roots).expect("engine");
    let voice = engine.0.default_voice();
    let text = "The quick brown fox jumps over the lazy dog, and then it does it again just to be sure.";
    let cancel = Arc::new(AtomicBool::new(false));
    for speed in [0.75f32, 1.0, 1.5, 2.0] {
        let pcm = engine.0.synthesize(text, &voice, speed, &cancel).expect("synth");
        println!("{profile} speed {speed:>4}: {:.2} s audio", pcm.samples.len() as f32 / pcm.rate as f32);
    }
}
