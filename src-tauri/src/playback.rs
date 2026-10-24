//! Native audio output via cpal. One stream for the app's lifetime; chunks are queued as
//! mono f32 at the device rate. Stop = clear the queue, so it is instant.

use crate::engines::Pcm;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    time::Duration,
};

#[derive(Debug, thiserror::Error)]
pub enum PlaybackError {
    #[error("no audio output device")]
    NoDevice,
    #[error("audio stream error: {0}")]
    Stream(String),
}

struct Shared {
    queue: Mutex<VecDeque<f32>>,
    paused: AtomicBool,
    /// Bumped on clear(); lets a producer notice its chunks were discarded.
    generation: AtomicU64,
}

#[derive(Clone)]
pub struct Player {
    shared: Arc<Shared>,
    pub device_rate: u32,
}

impl Player {
    pub fn new() -> Result<Self, PlaybackError> {
        let shared = Arc::new(Shared {
            queue: Mutex::new(VecDeque::new()),
            paused: AtomicBool::new(false),
            generation: AtomicU64::new(0),
        });
        let (tx, rx) = mpsc::channel::<Result<u32, PlaybackError>>();
        let for_thread = shared.clone();
        std::thread::Builder::new()
            .name("speechhub-audio".into())
            .spawn(move || run_stream(for_thread, tx))
            .map_err(|e| PlaybackError::Stream(e.to_string()))?;
        let device_rate = rx
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| PlaybackError::Stream("audio thread did not start".into()))??;
        Ok(Self { shared, device_rate })
    }

    pub fn enqueue(&self, pcm: &Pcm) {
        let resampled = resample(&pcm.samples, pcm.rate, self.device_rate);
        let mut q = self.shared.queue.lock().expect("audio queue poisoned");
        q.extend(resampled);
    }

    pub fn clear(&self) {
        self.shared.generation.fetch_add(1, Ordering::SeqCst);
        self.shared.queue.lock().expect("audio queue poisoned").clear();
    }

    pub fn generation(&self) -> u64 {
        self.shared.generation.load(Ordering::SeqCst)
    }

    pub fn pause(&self) {
        self.shared.paused.store(true, Ordering::SeqCst);
    }

    pub fn resume(&self) {
        self.shared.paused.store(false, Ordering::SeqCst);
    }

    pub fn is_paused(&self) -> bool {
        self.shared.paused.load(Ordering::SeqCst)
    }

    pub fn is_drained(&self) -> bool {
        self.shared.queue.lock().expect("audio queue poisoned").is_empty()
    }
}

fn run_stream(shared: Arc<Shared>, ready: mpsc::Sender<Result<u32, PlaybackError>>) {
    let host = cpal::default_host();
    let Some(device) = host.default_output_device() else {
        let _ = ready.send(Err(PlaybackError::NoDevice));
        return;
    };
    let supported = match device.default_output_config() {
        Ok(c) => c,
        Err(e) => {
            let _ = ready.send(Err(PlaybackError::Stream(e.to_string())));
            return;
        }
    };
    let config: cpal::StreamConfig = supported.config();
    let channels = config.channels as usize;
    let rate = config.sample_rate;
    let cb_shared = shared.clone();
    let stream = device.build_output_stream(
        config.clone(),
        move |data: &mut [f32], _| {
            if cb_shared.paused.load(Ordering::Relaxed) {
                data.fill(0.0);
                return;
            }
            let mut q = cb_shared.queue.lock().expect("audio queue poisoned");
            for frame in data.chunks_mut(channels) {
                let s = q.pop_front().unwrap_or(0.0);
                for out in frame.iter_mut() {
                    *out = s;
                }
            }
        },
        |err| log::error!("audio stream error: {err}"),
        None,
    );
    let stream = match stream {
        Ok(s) => s,
        Err(e) => {
            let _ = ready.send(Err(PlaybackError::Stream(e.to_string())));
            return;
        }
    };
    if let Err(e) = stream.play() {
        let _ = ready.send(Err(PlaybackError::Stream(e.to_string())));
        return;
    }
    let _ = ready.send(Ok(rate));
    // Keep the stream alive for the life of the process.
    loop {
        std::thread::park();
    }
}

/// Linear-interpolation resampler. Good enough for speech; replace with rubato if it ever matters.
fn resample(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || input.is_empty() {
        return input.to_vec();
    }
    let ratio = from as f64 / to as f64;
    let out_len = ((input.len() as f64) / ratio).floor() as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let pos = i as f64 * ratio;
        let idx = pos.floor() as usize;
        let frac = (pos - idx as f64) as f32;
        let a = input[idx.min(input.len() - 1)];
        let b = input[(idx + 1).min(input.len() - 1)];
        out.push(a + (b - a) * frac);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::resample;

    #[test]
    fn resample_changes_length_proportionally() {
        let input: Vec<f32> = (0..22050).map(|i| (i as f32 / 100.0).sin()).collect();
        let out = resample(&input, 22050, 44100);
        assert!((out.len() as i64 - 44100).abs() <= 2);
        let same = resample(&input, 22050, 22050);
        assert_eq!(same.len(), input.len());
    }
}
