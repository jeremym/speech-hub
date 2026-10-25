//! Native audio output via cpal. Chunks are queued as mono f32 at the current device rate;
//! stop = clear the queue, so it is instant.
//!
//! The stream follows the system default output device: an audio thread watches for the default
//! changing (headphones plugged in, AirPods connected, HDMI lost) or the stream failing, and
//! rebuilds the stream on the new device, resampling whatever is still queued. No device at all
//! is not fatal: the player stays usable and reports itself stalled until one appears.

use crate::engines::Pcm;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

const DEVICE_POLL: Duration = Duration::from_millis(500);
/// Queue non-empty but the device has not pulled samples for this long: output is dead.
const STALL_AFTER: Duration = Duration::from_secs(3);
const FALLBACK_RATE: u32 = 48_000;

struct Shared {
    queue: Mutex<VecDeque<f32>>,
    paused: AtomicBool,
    /// Bumped on clear(); lets a producer notice its chunks were discarded.
    generation: AtomicU64,
    /// Sample rate the queue currently holds (the live device's rate).
    rate: AtomicU32,
    /// Set by the stream error callback; the audio thread rebuilds the stream.
    dead: AtomicBool,
    /// Milliseconds since `epoch` when the device last pulled from a non-empty queue.
    last_pull_ms: AtomicU64,
    epoch: Instant,
}

impl Shared {
    fn now_ms(&self) -> u64 {
        self.epoch.elapsed().as_millis() as u64
    }
}

#[derive(Clone)]
pub struct Player {
    shared: Arc<Shared>,
}

impl Player {
    pub fn new() -> Self {
        let shared = Arc::new(Shared {
            queue: Mutex::new(VecDeque::new()),
            paused: AtomicBool::new(false),
            generation: AtomicU64::new(0),
            rate: AtomicU32::new(FALLBACK_RATE),
            dead: AtomicBool::new(true),
            last_pull_ms: AtomicU64::new(0),
            epoch: Instant::now(),
        });
        let for_thread = shared.clone();
        if let Err(e) = std::thread::Builder::new().name("speechhub-audio".into()).spawn(move || run_stream(for_thread)) {
            log::error!("audio thread: {e}");
        }
        Self { shared }
    }

    pub fn enqueue(&self, pcm: &Pcm) {
        // Rate is read under the queue lock so a device switch cannot slip between the two.
        let mut q = self.shared.queue.lock().expect("audio queue poisoned");
        let rate = self.shared.rate.load(Ordering::SeqCst);
        let resampled = resample(&pcm.samples, pcm.rate, rate);
        if q.is_empty() {
            // Fresh job: the stall clock starts now, not at the last thing that played.
            self.shared.last_pull_ms.store(self.shared.now_ms(), Ordering::SeqCst);
        }
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

    /// Samples are queued but nothing has consumed them for a while: no working output device.
    pub fn is_stalled(&self) -> bool {
        if self.is_paused() || self.is_drained() {
            return false;
        }
        let last = self.shared.last_pull_ms.load(Ordering::SeqCst);
        self.shared.now_ms().saturating_sub(last) > STALL_AFTER.as_millis() as u64
    }
}

fn default_device_name(host: &cpal::Host) -> Option<String> {
    host.default_output_device().and_then(|d| d.description().ok().map(|desc| desc.name().to_string()))
}

fn run_stream(shared: Arc<Shared>) {
    let host = cpal::default_host();
    let mut stream: Option<cpal::Stream> = None;
    let mut current: Option<String> = None;
    loop {
        let wanted = default_device_name(&host);
        let needs_rebuild = stream.is_none() || shared.dead.load(Ordering::SeqCst) || wanted != current;
        if needs_rebuild {
            // Drop the old stream first: CoreAudio will not hand out a new one to the same client
            // while a stream bound to a vanished device is still alive.
            stream = None;
            match build_stream(&host, &shared) {
                Ok((s, rate)) => {
                    let old_rate = shared.rate.load(Ordering::SeqCst);
                    if old_rate != rate {
                        let mut q = shared.queue.lock().expect("audio queue poisoned");
                        let pending: Vec<f32> = q.drain(..).collect();
                        q.extend(resample(&pending, old_rate, rate));
                        shared.rate.store(rate, Ordering::SeqCst);
                    }
                    shared.dead.store(false, Ordering::SeqCst);
                    shared.last_pull_ms.store(shared.now_ms(), Ordering::SeqCst);
                    if current != wanted {
                        log::info!("audio output: {} @ {rate} Hz", wanted.as_deref().unwrap_or("unknown device"));
                    }
                    current = wanted;
                    stream = Some(s);
                }
                Err(e) => {
                    if !shared.dead.load(Ordering::SeqCst) || current.is_some() {
                        log::warn!("audio output unavailable: {e}");
                    }
                    shared.dead.store(true, Ordering::SeqCst);
                    current = None;
                }
            }
        }
        std::thread::sleep(DEVICE_POLL);
    }
}

fn build_stream(host: &cpal::Host, shared: &Arc<Shared>) -> Result<(cpal::Stream, u32), String> {
    let device = host.default_output_device().ok_or("no audio output device")?;
    let supported = device.default_output_config().map_err(|e| e.to_string())?;
    let config: cpal::StreamConfig = supported.config();
    let channels = config.channels as usize;
    let rate = config.sample_rate;
    let cb_shared = shared.clone();
    let err_shared = shared.clone();
    let stream = device
        .build_output_stream(
            config.clone(),
            move |data: &mut [f32], _| {
                if cb_shared.paused.load(Ordering::Relaxed) {
                    data.fill(0.0);
                    return;
                }
                let mut q = cb_shared.queue.lock().expect("audio queue poisoned");
                if !q.is_empty() {
                    cb_shared.last_pull_ms.store(cb_shared.now_ms(), Ordering::Relaxed);
                }
                for frame in data.chunks_mut(channels) {
                    let s = q.pop_front().unwrap_or(0.0);
                    for out in frame.iter_mut() {
                        *out = s;
                    }
                }
            },
            move |err| {
                log::warn!("audio stream error: {err}");
                err_shared.dead.store(true, Ordering::SeqCst);
            },
            None,
        )
        .map_err(|e| e.to_string())?;
    stream.play().map_err(|e| e.to_string())?;
    Ok((stream, rate))
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
