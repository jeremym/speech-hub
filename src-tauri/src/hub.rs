//! Speech Hub: the single pipeline every entry point goes through.
//! request → validate → normalize → (pronunciation, M5) → segment → engine → player.

use crate::{
    accessibility, engines::{self, Engine, EngineError}, playback::Player, settings::{RepeatBehavior, SettingsStore},
    text, windows,
};
use speechhub_protocol::{EngineInfo, ErrorCode, HubState, Progress, SpeakOptions, Status, VoiceInfo};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
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
    cancel: AtomicBool,
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
    models_dir: PathBuf,
    player: Player,
    engine: Mutex<Option<Arc<dyn Engine>>>,
    status: Mutex<Status>,
    job: Mutex<Option<Arc<Job>>>,
    pending_reader: Mutex<Option<ReaderText>>,
    last_activity: Mutex<Instant>,
}

#[derive(Clone)]
pub struct Hub {
    inner: Arc<Inner>,
}

impl Hub {
    pub fn new(app: AppHandle, settings: SettingsStore, models_dir: PathBuf) -> Result<Self, String> {
        let player = Player::new().map_err(|e| e.to_string())?;
        let s = settings.get();
        let hub = Hub {
            inner: Arc::new(Inner {
                app,
                settings,
                models_dir,
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
            }),
        };
        hub.load_engine(&s.engine_profile);
        Ok(hub)
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
            .spawn(move || match engines::create(&profile, &hub.inner.models_dir) {
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

    fn engine(&self) -> Result<Arc<dyn Engine>, HubError> {
        self.inner.engine.lock().unwrap().clone().ok_or(HubError::EngineNotReady)
    }

    pub fn voices(&self) -> Vec<VoiceInfo> {
        self.inner.engine.lock().unwrap().as_ref().map(|e| e.voices()).unwrap_or_default()
    }

    pub fn models_dir(&self) -> &PathBuf {
        &self.inner.models_dir
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
        let chunks = text::segment(text);
        if chunks.is_empty() {
            return Err(HubError::Invalid("nothing to speak".into()));
        }
        let engine = self.engine()?;
        self.cancel_current();
        self.inner.player.resume();

        let job = Arc::new(Job { id: uuid::Uuid::new_v4().to_string(), cancel: AtomicBool::new(false) });
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

    /// Option+Escape: capture the selection and speak it, honoring the repeat behavior.
    pub fn speak_selection(&self) -> Result<String, HubError> {
        let settings = self.inner.settings.get();
        if self.is_busy() && settings.repeat_shortcut == RepeatBehavior::Stop {
            self.stop();
            return Ok(String::new());
        }
        match accessibility::selected_text() {
            Ok(sel) => {
                let title = Some(match sel.app_name {
                    Some(app) => format!("Selection from {app}"),
                    None => "Selected text".to_string(),
                });
                self.speak(&sel.text, SpeakOptions { title, ..Default::default() })
            }
            Err(e) => {
                // M4: run settings.fallback_source_id here when configured.
                self.notice(e.to_string());
                Err(e.into())
            }
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
