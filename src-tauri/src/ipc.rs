//! Unix-domain-socket server: the public interface for the `speak` CLI and other local apps.
//! One request per connection, length-prefixed JSON (speechhub-protocol). Same-user only.

use crate::{hub::{self, Hub}, windows};
use speechhub_protocol::{self as proto, Envelope, ErrorCode, Request, Response, frame};
use std::{
    fs,
    io::{self, Read, Write},
    os::unix::{fs::{FileTypeExt, PermissionsExt}, io::AsRawFd, net::{UnixListener, UnixStream}},
    path::{Path, PathBuf},
    time::Duration,
};
use tauri::AppHandle;

/// Start listening. Returns the socket path. Errors are fatal for the IPC feature only.
pub fn start(app: AppHandle, data_dir: &Path) -> io::Result<PathBuf> {
    let run_dir = data_dir.join("run");
    fs::create_dir_all(&run_dir)?;
    fs::set_permissions(&run_dir, fs::Permissions::from_mode(0o700))?;
    let sock = run_dir.join("hub.sock");
    // Never follow a symlink someone planted; remove whatever is there (single-instance plugin
    // guarantees we're the only live server).
    if let Ok(meta) = fs::symlink_metadata(&sock) {
        if meta.file_type().is_symlink() || meta.file_type().is_socket() {
            let _ = fs::remove_file(&sock);
        } else {
            return Err(io::Error::other("socket path is occupied by a regular file"));
        }
    }
    let listener = UnixListener::bind(&sock)?;
    fs::set_permissions(&sock, fs::Permissions::from_mode(0o600))?;
    log::info!("ipc listening on {}", sock.display());
    std::thread::Builder::new().name("speechhub-ipc".into()).spawn(move || {
        for conn in listener.incoming() {
            match conn {
                Ok(stream) => {
                    let app = app.clone();
                    let _ = std::thread::Builder::new()
                        .name("speechhub-ipc-conn".into())
                        .spawn(move || handle(app, stream));
                }
                Err(e) => log::warn!("ipc accept: {e}"),
            }
        }
    })?;
    Ok(sock)
}

fn same_user(stream: &UnixStream) -> bool {
    let mut uid: libc::uid_t = 0;
    let mut gid: libc::gid_t = 0;
    let rc = unsafe { libc::getpeereid(stream.as_raw_fd(), &mut uid, &mut gid) };
    rc == 0 && uid == unsafe { libc::getuid() }
}

fn handle(app: AppHandle, mut stream: UnixStream) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(15)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(15)));
    if !same_user(&stream) {
        log::warn!("ipc: rejected connection from another user");
        return;
    }
    let payload = match frame::read(&mut stream) {
        Ok(p) => p,
        Err(e) => {
            let _ = reply(&mut stream, &Response::error("unknown", ErrorCode::InvalidRequest, format!("bad frame: {e}")));
            return;
        }
    };
    let env: Envelope = match serde_json::from_slice(&payload) {
        Ok(e) => e,
        Err(e) => {
            let _ = reply(&mut stream, &Response::error("unknown", ErrorCode::InvalidRequest, format!("bad json: {e}")));
            return;
        }
    };
    if let Err((code, msg)) = proto::validate(&env) {
        let _ = reply(&mut stream, &Response::error(env.request_id, code, msg));
        return;
    }
    let response = dispatch(&app, env);
    let _ = reply(&mut stream, &response);
}

fn reply(stream: &mut UnixStream, r: &Response) -> io::Result<()> {
    let bytes = serde_json::to_vec(r).map_err(io::Error::other)?;
    frame::write(stream, &bytes)
}

/// Route a validated request to the hub. Also used by deep links (with a restricted subset).
pub fn dispatch(app: &AppHandle, env: Envelope) -> Response {
    let hub: Hub = hub::get(app);
    let id = env.request_id;
    let hub_err = |e: hub::HubError| Response::error(id.clone(), e.code(), e.to_string());
    match env.request {
        Request::Speak { text, options } => match hub.speak(&text, options) {
            Ok(rid) => Response::ok_with(id, serde_json::json!({ "playback_id": rid })),
            Err(e) => hub_err(e),
        },
        Request::Open { text, options } => {
            hub.open(&text, options.title);
            Response::ok(id)
        }
        Request::Render { text, options, path } => match hub.render(&text, options, Path::new(&path)) {
            Ok(info) => Response::ok_with(id, info),
            Err(e) => hub_err(e),
        },
        Request::RunSource { source_id } => match crate::sources::run_by_ref(app, &source_id) {
            Ok(v) => Response::ok_with(id, v),
            Err(e) => Response::error(id, e.code(), e.to_string()),
        },
        Request::Stop => {
            hub.stop();
            Response::ok(id)
        }
        Request::Pause => {
            hub.pause();
            Response::ok(id)
        }
        Request::Resume => {
            hub.resume();
            Response::ok(id)
        }
        Request::Status => Response::ok_with(id, hub.status()),
        Request::ListVoices => Response::ok_with(id, hub.voices()),
        Request::ListSources => Response::ok_with(id, crate::sources::list_public(app)),
        Request::Palette => {
            windows::show(app, windows::PALETTE);
            Response::ok(id)
        }
        Request::Version => Response::ok_with(
            id,
            serde_json::json!({ "app": app.package_info().version.to_string(), "protocol": proto::PROTOCOL_VERSION }),
        ),
    }
}

/// Read everything from a stream (used by tests).
#[allow(dead_code)]
pub fn read_all(mut s: impl Read) -> Vec<u8> {
    let mut v = Vec::new();
    let _ = s.read_to_end(&mut v);
    v
}

#[allow(dead_code)]
pub fn write_all(mut s: impl Write, b: &[u8]) {
    let _ = s.write_all(b);
}
