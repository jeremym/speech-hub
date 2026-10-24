//! Fetch text for each Source kind. Hard limits everywhere: size, time, no shell by default,
//! scrubbed environment, no network except the Http kind.

use super::{SourceError, SourceKind};
use std::{
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

/// Cap on text produced by any Source.
pub const MAX_OUTPUT_BYTES: usize = 1024 * 1024;

pub fn fetch_text(kind: &SourceKind, timeout_secs: u32) -> Result<String, SourceError> {
    let timeout = Duration::from_secs(timeout_secs.clamp(1, 120) as u64);
    match kind {
        SourceKind::Text { text } => Ok(text.clone()),
        SourceKind::File { path } => read_file(path),
        SourceKind::Command { program, args, cwd, shell } => run_command(program, args, cwd.as_deref(), *shell, timeout),
        SourceKind::Shortcut { name, input } => run_shortcut(name, input.as_deref(), timeout),
        SourceKind::Http { url, json_path } => fetch_http(url, json_path.as_deref(), timeout),
    }
}

pub fn expand_home(p: &str) -> PathBuf {
    if let Some(rest) = p.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return Path::new(&home).join(rest);
        }
    }
    PathBuf::from(p)
}

fn read_file(path: &str) -> Result<String, SourceError> {
    if path.trim().is_empty() {
        return Err(SourceError::Failed("no file chosen".into()));
    }
    let p = expand_home(path);
    if !p.is_absolute() {
        return Err(SourceError::Failed("file path must be absolute".into()));
    }
    let meta = std::fs::metadata(&p).map_err(|e| SourceError::Failed(format!("{}: {e}", p.display())))?;
    if !meta.is_file() {
        return Err(SourceError::Failed(format!("{} is not a file", p.display())));
    }
    let mut f = std::fs::File::open(&p).map_err(|e| SourceError::Failed(e.to_string()))?;
    let mut buf = Vec::new();
    f.by_ref().take(MAX_OUTPUT_BYTES as u64 + 1).read_to_end(&mut buf).map_err(|e| SourceError::Failed(e.to_string()))?;
    let truncated = buf.len() > MAX_OUTPUT_BYTES;
    buf.truncate(MAX_OUTPUT_BYTES);
    let mut text = String::from_utf8_lossy(&buf).into_owned();
    if truncated {
        text.push_str("\n\n[File truncated at 1 MB]");
    }
    Ok(text)
}

/// Minimal, predictable environment for child processes. No secrets leak from the app's env.
fn scrubbed_env(cmd: &mut Command) {
    cmd.env_clear();
    cmd.env("PATH", "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin");
    for key in ["HOME", "USER", "LOGNAME", "TMPDIR", "LANG", "LC_ALL", "SHELL"] {
        if let Ok(v) = std::env::var(key) {
            cmd.env(key, v);
        }
    }
    cmd.env("SPEECH_HUB", "1");
}

fn run_command(program: &str, args: &[String], cwd: Option<&str>, shell: bool, timeout: Duration) -> Result<String, SourceError> {
    if program.trim().is_empty() {
        return Err(SourceError::Failed("no program given".into()));
    }
    let mut cmd = if shell {
        let mut c = Command::new("/bin/zsh");
        c.arg("-c").arg(program);
        c
    } else {
        let mut c = Command::new(expand_home(program));
        c.args(args);
        c
    };
    scrubbed_env(&mut cmd);
    if let Some(dir) = cwd.filter(|d| !d.trim().is_empty()) {
        cmd.current_dir(expand_home(dir));
    }
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| SourceError::Failed(format!("cannot run {program}: {e}")))?;
    let mut stdout = child.stdout.take().expect("piped");
    let mut stderr = child.stderr.take().expect("piped");
    // Read stdout on a helper thread so a chatty command can't deadlock us; enforce the timeout here.
    let reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = (&mut stdout).take(MAX_OUTPUT_BYTES as u64 + 1).read_to_end(&mut buf);
        buf
    });
    let err_reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = (&mut stderr).take(16 * 1024).read_to_end(&mut buf);
        buf
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if start.elapsed() > timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(SourceError::Failed(format!("{program} took longer than {}s and was stopped", timeout.as_secs())));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(e) => return Err(SourceError::Failed(e.to_string())),
        }
    };
    let out = reader.join().unwrap_or_default();
    let err = err_reader.join().unwrap_or_default();
    if !status.success() {
        let msg = String::from_utf8_lossy(&err).trim().to_string();
        return Err(SourceError::Failed(if msg.is_empty() { format!("{program} exited with {status}") } else { msg.chars().take(300).collect() }));
    }
    Ok(bytes_to_text(out))
}

fn bytes_to_text(mut out: Vec<u8>) -> String {
    let truncated = out.len() > MAX_OUTPUT_BYTES;
    out.truncate(MAX_OUTPUT_BYTES);
    let mut text = String::from_utf8_lossy(&out).into_owned();
    if truncated {
        text.push_str("\n\n[Output truncated at 1 MB]");
    }
    text
}

fn run_shortcut(name: &str, input: Option<&str>, timeout: Duration) -> Result<String, SourceError> {
    if name.trim().is_empty() {
        return Err(SourceError::Failed("no Shortcut chosen".into()));
    }
    let out_path = std::env::temp_dir().join(format!("speechhub-shortcut-{}.txt", uuid::Uuid::new_v4()));
    let mut args: Vec<String> = vec!["run".into(), name.to_string(), "--output-path".into(), out_path.display().to_string(), "--output-type".into(), "public.plain-text".into()];
    let in_path = if let Some(text) = input.filter(|t| !t.is_empty()) {
        let p = std::env::temp_dir().join(format!("speechhub-shortcut-in-{}.txt", uuid::Uuid::new_v4()));
        std::fs::write(&p, text).map_err(|e| SourceError::Failed(e.to_string()))?;
        args.push("--input-path".into());
        args.push(p.display().to_string());
        Some(p)
    } else {
        None
    };
    let result = run_command("/usr/bin/shortcuts", &args, None, false, timeout);
    if let Some(p) = in_path {
        let _ = std::fs::remove_file(p);
    }
    let stdout = result?;
    let from_file = std::fs::read(&out_path).ok().map(bytes_to_text);
    let _ = std::fs::remove_file(&out_path);
    match from_file {
        Some(t) if !t.trim().is_empty() => Ok(t),
        _ if !stdout.trim().is_empty() => Ok(stdout),
        _ => Err(SourceError::Failed(format!("Shortcut “{name}” returned no text. End it with a text result (e.g. a Text or Get Text action)."))),
    }
}

/// Uses the system curl: no TLS stack to bundle, honors proxies and keychain roots.
fn fetch_http(url: &str, json_path: Option<&str>, timeout: Duration) -> Result<String, SourceError> {
    let url = url.trim();
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(SourceError::Failed("URL must start with http:// or https://".into()));
    }
    let args: Vec<String> = vec![
        "-fsSL".into(),
        "--max-time".into(), timeout.as_secs().to_string(),
        "--max-filesize".into(), MAX_OUTPUT_BYTES.to_string(),
        "--proto".into(), "=http,https".into(),
        "-A".into(), "SpeechHub/0.1 (+https://github.com/jeremym/speech-hub)".into(),
        "-H".into(), "Accept: text/plain, application/json;q=0.9, text/*;q=0.8, */*;q=0.1".into(),
        "-w".into(), "\n\u{1}%{content_type}".into(),
        url.to_string(),
    ];
    let raw = run_command("/usr/bin/curl", &args, None, false, timeout + Duration::from_secs(2))?;
    let (body, content_type) = match raw.rsplit_once('\u{1}') {
        Some((b, ct)) => (b.trim_end_matches('\n').to_string(), ct.trim().to_lowercase()),
        None => (raw, String::new()),
    };
    if let Some(path) = json_path.map(str::trim).filter(|p| !p.is_empty()) {
        return extract_json_path(&body, path);
    }
    if content_type.contains("json") {
        // No path given: speak the JSON's string values in order, not the braces.
        return Ok(json_strings(&body));
    }
    if content_type.contains("html") || body.trim_start().starts_with("<!") || body.trim_start().starts_with("<html") {
        return Ok(strip_html(&body));
    }
    Ok(body)
}

/// `items.0.title` → value as text. Arrays/objects at the end are rendered as their string values.
pub fn extract_json_path(body: &str, path: &str) -> Result<String, SourceError> {
    let mut v: serde_json::Value = serde_json::from_str(body).map_err(|e| SourceError::Failed(format!("response is not JSON: {e}")))?;
    let normalized = path.replace('[', ".").replace(']', "");
    for part in normalized.split('.').filter(|p| !p.is_empty()) {
        v = match (&v, part.parse::<usize>()) {
            (serde_json::Value::Array(a), Ok(i)) => a.get(i).cloned().unwrap_or(serde_json::Value::Null),
            (serde_json::Value::Object(o), _) => o.get(part).cloned().unwrap_or(serde_json::Value::Null),
            _ => serde_json::Value::Null,
        };
        if v.is_null() {
            return Err(SourceError::Failed(format!("nothing at JSON path “{path}” (stopped at “{part}”)")));
        }
    }
    Ok(match v {
        serde_json::Value::String(s) => s,
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        other => json_strings(&other.to_string()),
    })
}

fn json_strings(body: &str) -> String {
    fn walk(v: &serde_json::Value, out: &mut Vec<String>) {
        match v {
            serde_json::Value::String(s) => out.push(s.clone()),
            serde_json::Value::Number(n) => out.push(n.to_string()),
            serde_json::Value::Array(a) => a.iter().for_each(|x| walk(x, out)),
            serde_json::Value::Object(o) => o.values().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(body) {
        walk(&v, &mut out);
    }
    if out.is_empty() { body.to_string() } else { out.join(". ") }
}

/// Good-enough HTML to text: drop script/style, turn block tags into newlines, strip the rest,
/// decode the common entities. Not a browser; fine for articles and status pages.
pub fn strip_html(html: &str) -> String {
    let mut out = String::with_capacity(html.len() / 2);
    let lower = html.to_lowercase();
    let mut i = 0;
    let bytes = html.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'<' {
            let end = match html[i..].find('>') { Some(e) => i + e + 1, None => break };
            let tag = &lower[i..end];
            if tag.starts_with("<script") || tag.starts_with("<style") {
                let close = if tag.starts_with("<script") { "</script>" } else { "</style>" };
                i = lower[end..].find(close).map(|c| end + c + close.len()).unwrap_or(bytes.len());
                continue;
            }
            let name: String = tag.trim_start_matches('<').trim_start_matches('/').chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
            if matches!(name.as_str(), "p" | "div" | "br" | "li" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "tr" | "section" | "article" | "blockquote" | "pre") {
                out.push('\n');
            }
            i = end;
            continue;
        }
        // Copy a run of text until the next tag.
        let next = html[i..].find('<').map(|n| i + n).unwrap_or(bytes.len());
        out.push_str(&html[i..next]);
        i = next;
    }
    let decoded = out
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&mdash;", "—")
        .replace("&ndash;", "–")
        .replace("&hellip;", "…");
    // Collapse runs of blank lines / spaces.
    let mut lines: Vec<&str> = Vec::new();
    for line in decoded.lines() {
        let t = line.trim();
        if !t.is_empty() {
            lines.push(t);
        }
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_runs_without_shell() {
        let out = run_command("/bin/echo", &["hello".into(), "$HOME".into()], None, false, Duration::from_secs(5)).unwrap();
        assert_eq!(out.trim(), "hello $HOME"); // literally, no expansion
    }

    #[test]
    fn shell_opt_in_expands() {
        let out = run_command("echo a b | tr a-z A-Z", &[], None, true, Duration::from_secs(5)).unwrap();
        assert_eq!(out.trim(), "A B");
    }

    #[test]
    fn command_timeout_kills() {
        let err = run_command("/bin/sleep", &["5".into()], None, false, Duration::from_secs(1)).unwrap_err();
        assert!(err.to_string().contains("longer than"));
    }

    #[test]
    fn command_failure_reports_stderr() {
        let err = run_command("/bin/ls", &["/definitely/not/here".into()], None, false, Duration::from_secs(5)).unwrap_err();
        assert!(err.to_string().contains("No such file"), "{err}");
    }

    #[test]
    fn env_is_scrubbed() {
        unsafe { std::env::set_var("SPEECHHUB_TEST_SECRET", "leak") };
        let out = run_command("/usr/bin/env", &[], None, false, Duration::from_secs(5)).unwrap();
        assert!(!out.contains("SPEECHHUB_TEST_SECRET"));
        assert!(out.contains("SPEECH_HUB=1"));
    }

    #[test]
    fn file_reads_and_rejects_relative() {
        let p = std::env::temp_dir().join(format!("speechhub-{}.md", uuid::Uuid::new_v4()));
        std::fs::write(&p, "# Title\n\nBody").unwrap();
        assert_eq!(read_file(p.to_str().unwrap()).unwrap(), "# Title\n\nBody");
        assert!(read_file("relative.md").is_err());
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn json_path_and_strings() {
        let body = r#"{"items":[{"title":"First","n":2},{"title":"Second"}],"ok":true}"#;
        assert_eq!(extract_json_path(body, "items.0.title").unwrap(), "First");
        assert_eq!(extract_json_path(body, "items[1].title").unwrap(), "Second");
        assert!(extract_json_path(body, "items.5.title").is_err());
        assert_eq!(json_strings(r#"{"a":"x","b":["y","z"]}"#), "x. y. z");
    }

    #[test]
    fn html_stripping() {
        let html = "<html><head><style>p{}</style><script>var x=1;</script></head><body><h1>Hi &amp; bye</h1><p>One</p><p>Two&nbsp;words</p></body></html>";
        assert_eq!(strip_html(html), "Hi & bye\nOne\nTwo words");
    }
}
