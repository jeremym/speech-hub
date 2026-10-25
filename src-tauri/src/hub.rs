//! Speech Hub: the single pipeline every entry point goes through.
//! request → validate → normalize → (pronunciation, M5) → segment → engine → player.

use crate::{
    accessibility::{self, AxError}, engines::{self, Engine, EngineError, ModelRoots}, playback::Player, pronunciation::RuleStore,
    settings::{RepeatBehavior, SettingsStore}, text, windows,
};
use speechhub_protocol::{EngineInfo, ErrorCode, HubState, Progress, SpeakOptions, Status, VoiceInfo};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager};

pub const STATUS_EVENT: &str = "hub:status";
pub const READER_TEXT_EVENT: &str = "reader:text";

#[derive(Debug, thiserror::Error)]
pub enum HubError {
    #[error("{0}")]
    Engine(#[from] EngineError),
    #[error("{0}")]
    Accessibility(#[from] accessibility::AxError),
    #[error("engine is still loading")]
    EngineNotReady,
    #[error("{0}")]
    Invalid(String),
}

impl HubError {
    pub fn code(&self) -> ErrorCode {
        match self {
            HubError::Engine(EngineError::NotInstalled(_)) | HubError::EngineNotReady => ErrorCode::EngineNotReady,
            HubError::Engine(_) => ErrorCode::Internal,
            HubError::Accessibility(accessibility::AxError::NotTrusted) => ErrorCode::PermissionRequired,
            HubError::Accessibility(_) => ErrorCode::SelectionUnavailable,
            HubError::Invalid(_) => ErrorCode::InvalidRequest,
        }
    }
}

struct Job {
    id: String,
    cancel: Arc<AtomicBool>,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionPreview {
    pub preview: String,
    pub chars: usize,
    pub app_name: Option<String>,
}

#[derive(Clone, serde::Serialize)]
pub struct RenderInfo {
    pub path: String,
    pub seconds: f32,
    pub sample_rate: u32,
    pub voice_id: String,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReaderText {
    pub text: String,
    pub title: Option<String>,
}

struct Inner {
    app: AppHandle,
    settings: SettingsStore,
    roots: ModelRoots,
    rules: RuleStore,
    player: Player,
    engine: Mutex<Option<Arc<dyn Engine>>>,
    status: Mutex<Status>,
    job: Mutex<Option<Arc<Job>>>,
    pending_reader: Mutex<Option<ReaderText>>,
    last_activity: Mutex<Instant>,
    /// Selection captured the moment the palette opened (the palette itself then has focus).
    selection_snapshot: Mutex<Option<(accessibility::Selection, Instant)>>,
    /// A background capture for the palette is still running.
    selection_pending: AtomicBool,
    selection_generation: AtomicU64,
}

#[derive(Clone)]
pub struct Hub {
    inner: Arc<Inner>,
}

impl Hub {
    pub fn new(app: AppHandle, settings: SettingsStore, roots: ModelRoots, rules: RuleStore) -> Result<Self, String> {
        let player = Player::new().map_err(|e| e.to_string())?;
        let s = settings.get();
        let hub = Hub {
            inner: Arc::new(Inner {
                app,
                settings,
                roots,
                rules,
                player,
                engine: Mutex::new(None),
                status: Mutex::new(Status {
                    state: HubState::Idle,
                    voice_id: s.voice_id.clone(),
                    speed: s.speed,
                    ..Default::default()
                }),
                job: Mutex::new(None),
                pending_reader: Mutex::new(None),
                last_activity: Mutex::new(Instant::now()),
                selection_snapshot: Mutex::new(None),
                selection_pending: AtomicBool::new(false),
                selection_generation: AtomicU64::new(0),
            }),
        };
        hub.load_engine(&s.engine_profile);
        hub.start_idle_watch();
        Ok(hub)
    }

    /// Text exactly as the engine will receive it: pronunciation rules, then normalization.
    pub fn prepare(&self, text: &str) -> Vec<String> {
        text::segment(&self.inner.rules.apply(text))
    }

    /// Unload the model to free memory. The next request reloads it.
    pub fn unload_engine(&self) {
        if self.is_busy() {
            return;
        }
        let had = self.inner.engine.lock().unwrap().take().is_some();
        if had {
            log::info!("engine unloaded after idle period");
            self.set_status(|st| {
                st.state = HubState::Idle;
                st.engine.loaded = false;
            });
        }
    }

    fn start_idle_watch(&self) {
        let hub = self.clone();
        std::thread::Builder::new()
            .name("speechhub-idle".into())
            .spawn(move || loop {
                std::thread::sleep(Duration::from_secs(30));
                let minutes = hub.inner.settings.get().idle_unload_minutes;
                if minutes == 0 || hub.is_busy() {
                    continue;
                }
                let idle = hub.inner.last_activity.lock().unwrap().elapsed();
                if idle >= Duration::from_secs(minutes as u64 * 60) && hub.inner.engine.lock().unwrap().is_some() {
                    hub.unload_engine();
                }
            })
            .expect("spawn idle watcher");
    }

    /// Engine, loading it first if it was unloaded. Blocks the caller for the load (~0.5 s).
    fn engine_or_load(&self) -> Result<Arc<dyn Engine>, HubError> {
        if let Some(e) = self.inner.engine.lock().unwrap().clone() {
            return Ok(e);
        }
        let st = self.status();
        if st.state == HubState::LoadingModel {
            return Err(HubError::EngineNotReady);
        }
        let profile = self.inner.settings.get().engine_profile;
        self.set_status(|st| st.state = HubState::LoadingModel);
        match engines::create(&profile, &self.inner.roots) {
            Ok((engine, _)) => {
                let engine: Arc<dyn Engine> = Arc::from(engine);
                let info = engine.info();
                *self.inner.engine.lock().unwrap() = Some(engine.clone());
                self.set_status(|st| {
                    st.state = HubState::Ready;
                    st.engine = info;
                });
                Ok(engine)
            }
            Err(e) => {
                self.set_status(|st| {
                    st.state = HubState::Error;
                    st.error = Some(e.to_string());
                });
                Err(e.into())
            }
        }
    }

    // ---------- engine lifecycle ----------

    /// Load (or switch to) an engine profile on a worker thread. State: LoadingModel → Ready | Error.
    pub fn load_engine(&self, profile: &str) {
        self.cancel_current();
        self.set_status(|st| {
            st.state = HubState::LoadingModel;
            st.error = None;
            st.engine = EngineInfo { id: profile.into(), name: profile.into(), loaded: false, ..Default::default() };
        });
        let hub = self.clone();
        let profile = profile.to_string();
        std::thread::Builder::new()
            .name("speechhub-engine-load".into())
            .spawn(move || match engines::create(&profile, &hub.inner.roots) {
                Ok((engine, actual_id)) => {
                    let engine: Arc<dyn Engine> = Arc::from(engine);
                    let info = engine.info();
                    let voices = engine.voices();
                    let mut voice = hub.inner.settings.get().voice_id;
                    if voice.is_empty() || !voices.iter().any(|v| v.id == voice) {
                        voice = engine.default_voice();
                    }
                    *hub.inner.engine.lock().unwrap() = Some(engine);
                    hub.set_status(|st| {
                        st.state = HubState::Ready;
                        st.engine = info;
                        st.voice_id = voice;
                    });
                    if actual_id != profile {
                        log::warn!("requested engine '{profile}', running '{actual_id}'");
                    }
                }
                Err(e) => {
                    *hub.inner.engine.lock().unwrap() = None;
                    hub.set_status(|st| {
                        st.state = HubState::Error;
                        st.error = Some(e.to_string());
                    });
                }
            })
            .expect("spawn engine loader");
    }

    pub fn voices(&self) -> Vec<VoiceInfo> {
        self.inner.engine.lock().unwrap().as_ref().map(|e| e.voices()).unwrap_or_default()
    }

    pub fn roots(&self) -> &ModelRoots {
        &self.inner.roots
    }

    /// Voice list changed (custom voice added/removed): tell the UI, and re-validate the current voice.
    pub fn refresh_voices(&self) {
        let voices = self.voices();
        let current = self.status().voice_id;
        if !voices.is_empty() && !voices.iter().any(|v| v.id == current) {
            let fallback = self.inner.engine.lock().unwrap().as_ref().map(|e| e.default_voice()).unwrap_or_default();
            self.set_status(|st| st.voice_id = fallback);
        } else {
            self.set_status(|_| {});
        }
        let _ = self.inner.app.emit("voices:changed", ());
    }

    pub fn set_voice(&self, voice_id: &str) {
        self.set_status(|st| st.voice_id = voice_id.to_string());
    }

    pub fn set_speed(&self, speed: f32) {
        self.set_status(|st| st.speed = speed.clamp(0.5, 3.0));
    }

    // ---------- status ----------

    pub fn status(&self) -> Status {
        self.inner.status.lock().unwrap().clone()
    }

    fn set_status(&self, f: impl FnOnce(&mut Status)) {
        let snapshot = {
            let mut st = self.inner.status.lock().unwrap();
            f(&mut st);
            st.clone()
        };
        let _ = self.inner.app.emit(STATUS_EVENT, &snapshot);
    }

    // ---------- requests ----------

    /// Speak now. Replaces the current reading. Returns the request id.
    pub fn speak(&self, text: &str, options: SpeakOptions) -> Result<String, HubError> {
        let chunks = self.prepare(text);
        if chunks.is_empty() {
            return Err(HubError::Invalid("nothing to speak".into()));
        }
        *self.inner.last_activity.lock().unwrap() = Instant::now();
        let engine = self.engine_or_load()?;
        self.cancel_current();
        self.inner.player.resume();

        let job = Arc::new(Job { id: uuid::Uuid::new_v4().to_string(), cancel: Arc::new(AtomicBool::new(false)) });
        *self.inner.job.lock().unwrap() = Some(job.clone());
        *self.inner.last_activity.lock().unwrap() = Instant::now();

        let (voice, speed) = {
            let st = self.inner.status.lock().unwrap();
            (
                options.voice_id.clone().filter(|v| !v.is_empty()).unwrap_or_else(|| st.voice_id.clone()),
                options.speed.unwrap_or(st.speed),
            )
        };
        let total = chunks.len() as u32;
        self.set_status(|st| {
            st.state = HubState::Synthesizing;
            st.request_id = Some(job.id.clone());
            st.title = options.title.clone();
            st.error = None;
            st.progress = Progress { synthesized: 0, total };
        });
        if self.inner.settings.get().show_mini_player && !windows::is_visible(&self.inner.app, windows::READER) {
            windows::show_player(&self.inner.app);
        }

        let hub = self.clone();
        let job_for_thread = job.clone();
        std::thread::Builder::new()
            .name("speechhub-synth".into())
            .spawn(move || hub.run_job(job_for_thread, engine, chunks, voice, speed))
            .expect("spawn synth thread");
        Ok(job.id.clone())
    }

    fn run_job(&self, job: Arc<Job>, engine: Arc<dyn Engine>, chunks: Vec<String>, voice: String, speed: f32) {
        let player = &self.inner.player;
        let generation = player.generation();
        let mut synthesized = 0u32;
        for chunk in &chunks {
            if job.cancel.load(Ordering::SeqCst) {
                return;
            }
            match engine.synthesize(chunk, &voice, speed, &job.cancel) {
                Ok(pcm) => {
                    if job.cancel.load(Ordering::SeqCst) || player.generation() != generation {
                        return;
                    }
                    player.enqueue(&pcm);
                    synthesized += 1;
                    self.set_status(|st| {
                        if st.request_id.as_deref() == Some(&job.id) {
                            st.progress.synthesized = synthesized;
                            if st.state == HubState::Synthesizing {
                                st.state = if player.is_paused() { HubState::Paused } else { HubState::Playing };
                            }
                        }
                    });
                }
                Err(EngineError::Cancelled) => return,
                Err(e) => {
                    log::error!("synthesis failed: {e}");
                    self.finish(&job, Some(e.to_string()));
                    return;
                }
            }
        }
        // Everything is queued; wait for the device to drain it.
        while !player.is_drained() {
            if job.cancel.load(Ordering::SeqCst) || player.generation() != generation {
                return;
            }
            std::thread::sleep(Duration::from_millis(40));
        }
        self.finish(&job, None);
    }

    fn finish(&self, job: &Job, error: Option<String>) {
        let mut slot = self.inner.job.lock().unwrap();
        if slot.as_ref().map(|j| j.id.as_str()) == Some(job.id.as_str()) {
            *slot = None;
            drop(slot);
            self.set_status(|st| {
                st.state = if error.is_some() { HubState::Error } else { HubState::Ready };
                st.error = error;
                st.request_id = None;
                st.progress = Progress::default();
            });
            *self.inner.last_activity.lock().unwrap() = Instant::now();
            windows::schedule_hide_player(&self.inner.app, Duration::from_millis(1800));
            if self.status().state == HubState::Error {
                let hub = self.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(2500));
                    hub.set_status(|st| {
                        if st.state == HubState::Error && st.request_id.is_none() {
                            st.state = HubState::Ready;
                            st.error = None;
                        }
                    });
                });
            }
        }
    }

    fn cancel_current(&self) {
        if let Some(job) = self.inner.job.lock().unwrap().take() {
            job.cancel.store(true, Ordering::SeqCst);
        }
        self.inner.player.clear();
    }

    pub fn stop(&self) {
        let had_job = self.inner.job.lock().unwrap().is_some();
        self.cancel_current();
        self.inner.player.resume();
        if had_job || self.status().state == HubState::Paused {
            self.set_status(|st| {
                st.state = HubState::Ready;
                st.request_id = None;
                st.title = None;
                st.error = None;
                st.progress = Progress::default();
            });
        }
        windows::schedule_hide_player(&self.inner.app, Duration::from_millis(600));
    }

    pub fn pause(&self) {
        if matches!(self.status().state, HubState::Playing | HubState::Synthesizing) {
            self.inner.player.pause();
            self.set_status(|st| st.state = HubState::Paused);
        }
    }

    pub fn resume(&self) {
        if self.status().state == HubState::Paused {
            self.inner.player.resume();
            self.set_status(|st| st.state = HubState::Playing);
        }
    }

    pub fn toggle_pause(&self) {
        match self.status().state {
            HubState::Paused => self.resume(),
            HubState::Playing | HubState::Synthesizing => self.pause(),
            _ => {}
        }
    }

    pub fn is_busy(&self) -> bool {
        matches!(self.status().state, HubState::Synthesizing | HubState::Playing | HubState::Paused)
    }

    /// Open the Reader with text. Does not speak.
    pub fn open(&self, text: &str, title: Option<String>) {
        let payload = ReaderText { text: text.to_string(), title };
        *self.inner.pending_reader.lock().unwrap() = Some(payload.clone());
        windows::show(&self.inner.app, windows::READER);
        let _ = self.inner.app.emit(READER_TEXT_EVENT, &payload);
    }

    pub fn take_pending_reader(&self) -> Option<ReaderText> {
        self.inner.pending_reader.lock().unwrap().take()
    }

    /// Open Quick Speak. Resolve the target app first: once the palette is key, the focused app
    /// is us and Accessibility would report "nothing selected". The read itself runs in the
    /// background because browsers can take seconds to expose their selection.
    pub fn open_palette(&self) {
        *self.inner.selection_snapshot.lock().unwrap() = None;
        let generation = self.inner.selection_generation.fetch_add(1, Ordering::SeqCst) + 1;
        let target = accessibility::Target::frontmost().ok();
        windows::show(&self.inner.app, windows::PALETTE);
        let Some(target) = target else { return };
        self.inner.selection_pending.store(true, Ordering::SeqCst);
        let hub = self.clone();
        std::thread::spawn(move || {
            let snap = target.capture(accessibility::WARMUP_BUDGET, |_| {}).ok();
            if hub.inner.selection_generation.load(Ordering::SeqCst) == generation {
                let found = snap.is_some();
                *hub.inner.selection_snapshot.lock().unwrap() = snap.map(|s| (s, Instant::now()));
                hub.inner.selection_pending.store(false, Ordering::SeqCst);
                if found {
                    let _ = hub.inner.app.emit("palette:selection", ());
                }
            }
        });
    }

    /// What the palette's "Speak selected text" row will read, if anything.
    pub fn selection_preview(&self) -> Option<SelectionPreview> {
        let guard = self.inner.selection_snapshot.lock().unwrap();
        let (sel, at) = guard.as_ref()?;
        if at.elapsed() > Duration::from_secs(300) {
            return None;
        }
        let words: String = sel.text.split_whitespace().take(12).collect::<Vec<_>>().join(" ");
        Some(SelectionPreview { preview: words, chars: sel.text.chars().count(), app_name: sel.app_name.clone() })
    }

    fn has_snapshot(&self) -> bool {
        self.inner.selection_pending.load(Ordering::SeqCst) || self.inner.selection_snapshot.lock().unwrap().is_some()
    }

    /// Blocks while a palette capture is still in flight (never on the main thread).
    fn take_fresh_snapshot(&self) -> Option<accessibility::Selection> {
        let deadline = Instant::now() + accessibility::WARMUP_BUDGET + Duration::from_millis(500);
        while self.inner.selection_pending.load(Ordering::SeqCst) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
        }
        let mut guard = self.inner.selection_snapshot.lock().unwrap();
        match guard.take() {
            Some((sel, at)) if at.elapsed() <= Duration::from_secs(300) => Some(sel),
            _ => None,
        }
    }

    /// Option+Escape: capture the selection and speak it, honoring the repeat behavior.
    /// From the palette, the snapshot taken when it opened is used instead of a live capture.
    ///
    /// Returns as soon as the target app is known; the capture and the speech start in the
    /// background so a slow browser never freezes the UI (the shortcut handler runs on the main thread).
    pub fn speak_selection(&self) -> Result<String, HubError> {
        let settings = self.inner.settings.get();
        if self.is_busy() && settings.repeat_shortcut == RepeatBehavior::Stop {
            self.stop();
            return Ok(String::new());
        }
        let target = if self.has_snapshot() {
            None
        } else {
            match accessibility::Target::frontmost() {
                Ok(t) => Some(t),
                Err(e) => return Err(self.selection_failed(e)),
            }
        };
        let hub = self.clone();
        std::thread::spawn(move || {
            let slowed = AtomicBool::new(false);
            let captured = match hub.take_fresh_snapshot() {
                Some(sel) => Ok(sel),
                None => target
                    .ok_or(AxError::NoFocusedElement)
                    .or_else(|_| accessibility::Target::frontmost())
                    .and_then(|t| {
                        t.capture(accessibility::WARMUP_BUDGET, |name| {
                            slowed.store(true, Ordering::SeqCst);
                            let name = name.unwrap_or("this app");
                            hub.set_loading_source(Some(&format!("Waking up {name}’s accessibility…")));
                        })
                    }),
            };
            if slowed.load(Ordering::SeqCst) {
                hub.set_loading_source(None);
            }
            match captured {
                Ok(sel) => {
                    let title = Some(match sel.app_name {
                        Some(app) => format!("Selection from {app}"),
                        None => "Selected text".to_string(),
                    });
                    if let Err(e) = hub.speak(&sel.text, SpeakOptions { title, ..Default::default() }) {
                        log::warn!("speak selection: {e}");
                    }
                }
                Err(e) => {
                    hub.selection_failed(e);
                }
            }
        });
        Ok(String::new())
    }

    /// Nothing usable was selected: run the configured fallback Source, else show why.
    fn selection_failed(&self, e: AxError) -> HubError {
        if !crate::sources::run_fallback(&self.inner.app, &self.inner.settings) {
            self.notice(e.to_string());
        }
        e.into()
    }

    /// Mark that a Source is fetching text (shown in the mini player while a command runs).
    pub fn set_loading_source(&self, name: Option<&str>) {
        let name = name.map(|n| n.to_string());
        let busy = self.is_busy();
        self.set_status(|st| match &name {
            Some(n) => {
                if !busy {
                    st.state = HubState::LoadingSource;
                }
                st.title = Some(n.clone());
            }
            None => {
                if st.state == HubState::LoadingSource {
                    st.state = HubState::Ready;
                }
            }
        });
        if name.is_some() && self.inner.settings.get().show_mini_player {
            windows::show_player(&self.inner.app);
        }
    }

    /// Show a transient message in the mini player without changing playback.
    pub fn notice(&self, message: String) {
        let busy = self.is_busy();
        self.set_status(|st| {
            if !busy {
                st.state = HubState::Error;
            }
            st.error = Some(message);
        });
        if self.inner.settings.get().show_mini_player {
            windows::show_player(&self.inner.app);
        }
        let hub = self.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(2800));
            hub.set_status(|st| {
                st.error = None;
                if st.state == HubState::Error && st.request_id.is_none() {
                    st.state = HubState::Ready;
                }
            });
            windows::schedule_hide_player(&hub.inner.app, Duration::from_millis(100));
        });
    }

    /// Synthesize to a 16-bit WAV file without playing. Blocks the caller (IPC thread).
    pub fn render(&self, text: &str, options: SpeakOptions, path: &std::path::Path) -> Result<RenderInfo, HubError> {
        let chunks = self.prepare(text);
        if chunks.is_empty() {
            return Err(HubError::Invalid("nothing to render".into()));
        }
        *self.inner.last_activity.lock().unwrap() = Instant::now();
        let engine = self.engine_or_load()?;
        let (voice, speed) = {
            let st = self.inner.status.lock().unwrap();
            (
                options.voice_id.clone().filter(|v| !v.is_empty()).unwrap_or_else(|| st.voice_id.clone()),
                options.speed.unwrap_or(st.speed),
            )
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let mut rate = 0u32;
        let mut samples: Vec<f32> = Vec::new();
        for chunk in &chunks {
            let pcm = engine.synthesize(chunk, &voice, speed, &cancel)?;
            if rate == 0 {
                rate = pcm.rate;
            }
            samples.extend_from_slice(&pcm.samples);
        }
        let spec = hound::WavSpec { channels: 1, sample_rate: rate, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
        let mut w = hound::WavWriter::create(path, spec).map_err(|e| HubError::Invalid(format!("cannot write {}: {e}", path.display())))?;
        for s in &samples {
            let v = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
            w.write_sample(v).map_err(|e| HubError::Invalid(e.to_string()))?;
        }
        w.finalize().map_err(|e| HubError::Invalid(e.to_string()))?;
        Ok(RenderInfo { path: path.display().to_string(), seconds: samples.len() as f32 / rate.max(1) as f32, sample_rate: rate, voice_id: voice })
    }

    pub fn preview_voice(&self, voice_id: &str) -> Result<String, HubError> {
        self.speak(
            "Hi, I'm one of the voices in Speech Hub. Select some text anywhere and I'll read it to you.",
            SpeakOptions { voice_id: Some(voice_id.to_string()), title: Some("Voice preview".into()), speed: None },
        )
    }
}

/// Convenience for commands: fetch the managed hub.
pub fn get(app: &AppHandle) -> Hub {
    app.state::<Hub>().inner().clone()
}
