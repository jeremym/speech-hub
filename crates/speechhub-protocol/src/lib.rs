//! Speech Hub protocol v1.
//!
//! Shared by the desktop app (socket server + Tauri commands) and the `speak` CLI.
//! Everything here is plain data: no I/O, no Tauri, no engine details.
//!
//! Design rules (see PLAN.md §5):
//! - Every request carries a `version` and a `request_id`.
//! - There is no verb that executes arbitrary commands. `run_source` references a Source the
//!   user already created and approved, by id only.
//! - Text payloads are capped at [`MAX_TEXT_BYTES`].

use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u32 = 1;
/// Hard cap on submitted text per request (UTF-8 bytes). Documented in docs/public-api.md.
pub const MAX_TEXT_BYTES: usize = 1024 * 1024;
/// Hard cap on a whole JSON frame on the wire.
pub const MAX_FRAME_BYTES: usize = MAX_TEXT_BYTES + 64 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Envelope {
    pub version: u32,
    pub request_id: String,
    #[serde(flatten)]
    pub request: Request,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Request {
    /// Speak text now. Replaces whatever is currently playing.
    Speak {
        text: String,
        #[serde(default)]
        options: SpeakOptions,
    },
    /// Open the Reader pre-filled with text. Does not start speaking.
    Open {
        text: String,
        #[serde(default)]
        options: SpeakOptions,
    },
    /// Render text to a WAV file at `path` without playing it (CLI `speak -o`).
    Render {
        text: String,
        #[serde(default)]
        options: SpeakOptions,
        path: String,
    },
    /// Run a user-defined Source by id or name (the only privileged verb).
    RunSource { source_id: String },
    Stop,
    Pause,
    Resume,
    Status,
    ListVoices,
    ListSources,
    /// Open the Quick Speak palette.
    Palette,
    /// App + protocol version.
    Version,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SpeakOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voice_id: Option<String>,
    /// Playback speed multiplier, 0.5 – 3.0. `None` = user default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed: Option<f32>,
    /// Short label shown in the mini player ("Morning briefing", "Selected text", ...).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "ok")]
pub enum Response {
    #[serde(rename = "true")]
    Ok {
        request_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        data: Option<serde_json::Value>,
    },
    #[serde(rename = "false")]
    Error {
        request_id: String,
        code: ErrorCode,
        message: String,
    },
}

impl Response {
    pub fn ok(request_id: impl Into<String>) -> Self {
        Response::Ok { request_id: request_id.into(), data: None }
    }
    pub fn ok_with(request_id: impl Into<String>, data: impl Serialize) -> Self {
        Response::Ok {
            request_id: request_id.into(),
            data: serde_json::to_value(data).ok(),
        }
    }
    pub fn error(request_id: impl Into<String>, code: ErrorCode, message: impl Into<String>) -> Self {
        Response::Error { request_id: request_id.into(), code, message: message.into() }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InvalidRequest,
    UnsupportedVersion,
    TextTooLarge,
    EngineNotReady,
    PermissionRequired,
    SelectionUnavailable,
    SourceNotFound,
    SourceNotTrusted,
    SourceFailed,
    Internal,
}

/// Application state machine (PLAN.md §3). Single source of truth lives in the Rust hub.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum HubState {
    #[default]
    Idle,
    LoadingModel,
    LoadingSource,
    Ready,
    Synthesizing,
    Playing,
    Paused,
    Cancelling,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Status {
    pub state: HubState,
    /// Request currently being synthesized/played, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub engine: EngineInfo,
    pub voice_id: String,
    pub speed: f32,
    /// Chunks synthesized / total for the active request (progress hint for the UI).
    #[serde(default)]
    pub progress: Progress,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
pub struct Progress {
    pub synthesized: u32,
    pub total: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct EngineInfo {
    pub id: String,
    pub name: String,
    pub loaded: bool,
    #[serde(default)]
    pub capabilities: EngineCapabilities,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
pub struct EngineCapabilities {
    pub speed_control: bool,
    pub voice_cloning: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VoiceInfo {
    pub id: String,
    pub name: String,
    pub language: String,
    pub engine_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gender: Option<String>,
    #[serde(default)]
    pub custom: bool,
}

/// Socket location relative to the app data dir (`~/Library/Application Support/<bundle id>`).
pub const BUNDLE_ID: &str = "com.jeremym.speechhub";
pub const SOCKET_RELATIVE_PATH: &str = "run/hub.sock";

/// Default socket path for the current user.
pub fn default_socket_path() -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(std::path::PathBuf::from(home).join("Library/Application Support").join(BUNDLE_ID).join(SOCKET_RELATIVE_PATH))
}

/// Length-prefixed JSON framing shared by server and client.
pub mod frame {
    use super::MAX_FRAME_BYTES;
    use std::io::{self, Read, Write};

    pub fn write<W: Write>(w: &mut W, payload: &[u8]) -> io::Result<()> {
        if payload.len() > MAX_FRAME_BYTES {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "frame too large"));
        }
        w.write_all(&(payload.len() as u32).to_be_bytes())?;
        w.write_all(payload)?;
        w.flush()
    }

    pub fn read<R: Read>(r: &mut R) -> io::Result<Vec<u8>> {
        let mut len = [0u8; 4];
        r.read_exact(&mut len)?;
        let len = u32::from_be_bytes(len) as usize;
        if len > MAX_FRAME_BYTES {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "frame too large"));
        }
        let mut buf = vec![0u8; len];
        r.read_exact(&mut buf)?;
        Ok(buf)
    }
}

/// Validate an envelope before it reaches the hub. Returns the error code to answer with.
pub fn validate(env: &Envelope) -> Result<(), (ErrorCode, String)> {
    if env.version != PROTOCOL_VERSION {
        return Err((
            ErrorCode::UnsupportedVersion,
            format!("protocol version {} not supported (want {PROTOCOL_VERSION})", env.version),
        ));
    }
    if env.request_id.is_empty() || env.request_id.len() > 128 {
        return Err((ErrorCode::InvalidRequest, "request_id must be 1–128 chars".into()));
    }
    let check_text = |t: &str| -> Result<(), (ErrorCode, String)> {
        if t.len() > MAX_TEXT_BYTES {
            return Err((
                ErrorCode::TextTooLarge,
                format!("text is {} bytes; limit is {MAX_TEXT_BYTES}", t.len()),
            ));
        }
        if t.trim().is_empty() {
            return Err((ErrorCode::InvalidRequest, "text is empty".into()));
        }
        Ok(())
    };
    let check_opts = |o: &SpeakOptions| -> Result<(), (ErrorCode, String)> {
        if let Some(s) = o.speed {
            if !(0.5..=3.0).contains(&s) {
                return Err((ErrorCode::InvalidRequest, "speed must be between 0.5 and 3.0".into()));
            }
        }
        if let Some(t) = &o.title {
            if t.len() > 200 {
                return Err((ErrorCode::InvalidRequest, "title too long".into()));
            }
        }
        Ok(())
    };
    match &env.request {
        Request::Speak { text, options } | Request::Open { text, options } => {
            check_text(text)?;
            check_opts(options)
        }
        Request::Render { text, options, path } => {
            check_text(text)?;
            check_opts(options)?;
            if path.is_empty() || path.len() > 4096 || !path.ends_with(".wav") {
                return Err((ErrorCode::InvalidRequest, "output path must end in .wav".into()));
            }
            Ok(())
        }
        Request::RunSource { source_id } => {
            if source_id.is_empty() || source_id.len() > 128 {
                return Err((ErrorCode::InvalidRequest, "source_id must be 1–128 chars".into()));
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(req: Request) -> Envelope {
        Envelope { version: PROTOCOL_VERSION, request_id: "r1".into(), request: req }
    }

    #[test]
    fn speak_roundtrip() {
        let e = env(Request::Speak {
            text: "Hello".into(),
            options: SpeakOptions { voice_id: Some("af_heart".into()), speed: Some(1.2), title: None },
        });
        let json = serde_json::to_string(&e).unwrap();
        assert!(json.contains(r#""action":"speak""#));
        let back: Envelope = serde_json::from_str(&json).unwrap();
        assert_eq!(back, e);
        assert!(validate(&back).is_ok());
    }

    #[test]
    fn stop_has_no_payload() {
        let e: Envelope = serde_json::from_str(r#"{"version":1,"request_id":"x","action":"stop"}"#).unwrap();
        assert_eq!(e.request, Request::Stop);
    }

    #[test]
    fn rejects_wrong_version() {
        let mut e = env(Request::Stop);
        e.version = 2;
        assert_eq!(validate(&e).unwrap_err().0, ErrorCode::UnsupportedVersion);
    }

    #[test]
    fn rejects_huge_text() {
        let e = env(Request::Speak { text: "a".repeat(MAX_TEXT_BYTES + 1), options: Default::default() });
        assert_eq!(validate(&e).unwrap_err().0, ErrorCode::TextTooLarge);
    }

    #[test]
    fn rejects_bad_speed() {
        let e = env(Request::Speak {
            text: "hi".into(),
            options: SpeakOptions { speed: Some(9.0), ..Default::default() },
        });
        assert_eq!(validate(&e).unwrap_err().0, ErrorCode::InvalidRequest);
    }

    #[test]
    fn frame_roundtrip() {
        let mut buf = Vec::new();
        frame::write(&mut buf, b"{\"a\":1}").unwrap();
        assert_eq!(&buf[..4], &[0, 0, 0, 7]);
        let back = frame::read(&mut &buf[..]).unwrap();
        assert_eq!(back, b"{\"a\":1}");
    }

    #[test]
    fn render_requires_wav() {
        let e = env(Request::Render { text: "hi".into(), options: Default::default(), path: "/tmp/x.mp3".into() });
        assert_eq!(validate(&e).unwrap_err().0, ErrorCode::InvalidRequest);
    }

    #[test]
    fn response_shape() {
        let r = Response::error("r1", ErrorCode::TextTooLarge, "too big");
        let v: serde_json::Value = serde_json::to_value(&r).unwrap();
        assert_eq!(v["ok"], "false");
        assert_eq!(v["code"], "text_too_large");
        let r = Response::ok("r1");
        let v: serde_json::Value = serde_json::to_value(&r).unwrap();
        assert_eq!(v["ok"], "true");
    }
}
