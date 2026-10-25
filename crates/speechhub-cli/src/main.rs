//! `speak` — thin client for Speech Hub. Sends one JSON request over the app's Unix socket.
//! Launches the app if it isn't running. Never loads a model itself.
//!
//! Accepts the common macOS `say` flags (-v, -r, -f, -o) so existing scripts can switch.

use speechhub_protocol::{self as proto, Envelope, Request, Response, SpeakOptions, frame};
use std::{
    io::{self, IsTerminal, Read},
    os::unix::net::UnixStream,
    path::PathBuf,
    process::ExitCode,
    time::{Duration, Instant},
};

const EXIT_OK: u8 = 0;
const EXIT_REQUEST: u8 = 1;
const EXIT_UNAVAILABLE: u8 = 2;
const EXIT_NOT_READY: u8 = 3;
const EXIT_USAGE: u8 = 64;

const HELP: &str = "speak — give anything on your Mac a voice (Speech Hub CLI)

USAGE
  speak [TEXT...]                 speak text now (reads stdin if no text and stdin is piped)
  speak open [TEXT...]            open the Reader with the text instead of speaking
  speak run <source>              run a saved Source by name or id
  speak palette                   open Quick Speak
  speak stop | pause | resume
  speak status [--json]
  speak voices [--json]
  speak sources [list] [--json]
  speak version

OPTIONS
  -v, --voice <id>       voice id (see `speak voices`)
  -r, --rate <wpm>       words per minute, like `say` (175 = normal)
      --speed <x>        speed multiplier 0.5–3.0 (overrides --rate)
  -f, --file <path>      read text from a file ('-' = stdin)
      --stdin            read text from stdin
  -o, --output <file.wav> render to a WAV file instead of playing
  -t, --title <text>     label shown in the mini player
      --open             same as the `open` subcommand
      --json             machine-readable output
      --no-launch        fail instead of launching Speech Hub if it isn't running
      --timeout <secs>   how long to wait for the app (default 10)
  -h, --help

EXIT CODES
  0 ok · 1 request rejected · 2 Speech Hub unavailable · 3 engine not ready / permission · 64 usage
";

#[derive(Default, Debug)]
struct Args {
    command: String,
    text_parts: Vec<String>,
    voice: Option<String>,
    rate_wpm: Option<f32>,
    speed: Option<f32>,
    file: Option<String>,
    stdin: bool,
    output: Option<String>,
    title: Option<String>,
    open: bool,
    json: bool,
    no_launch: bool,
    timeout: f32,
}

fn parse(argv: &[String]) -> Result<Args, String> {
    let mut a = Args { command: "speak".into(), timeout: 10.0, ..Default::default() };
    let mut it = argv.iter().peekable();
    // Subcommand = first arg if it names one.
    if let Some(first) = it.peek() {
        let f = first.as_str();
        if ["speak", "open", "run", "palette", "stop", "pause", "resume", "status", "voices", "sources", "version", "help"].contains(&f) {
            a.command = f.to_string();
            it.next();
            if a.command == "sources" {
                if let Some(sub) = it.peek() {
                    if sub.as_str() == "list" {
                        it.next();
                    }
                }
            }
        }
    }
    let need = |it: &mut std::iter::Peekable<std::slice::Iter<String>>, flag: &str| -> Result<String, String> {
        it.next().cloned().ok_or_else(|| format!("{flag} needs a value"))
    };
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "-v" | "--voice" => a.voice = Some(need(&mut it, "--voice")?),
            "-r" | "--rate" => a.rate_wpm = Some(need(&mut it, "--rate")?.parse().map_err(|_| "--rate must be a number")?),
            "--speed" => a.speed = Some(need(&mut it, "--speed")?.parse().map_err(|_| "--speed must be a number")?),
            "-f" | "--file" => a.file = Some(need(&mut it, "--file")?),
            "--stdin" => a.stdin = true,
            "-o" | "--output" => a.output = Some(need(&mut it, "--output")?),
            "-t" | "--title" => a.title = Some(need(&mut it, "--title")?),
            "--open" => a.open = true,
            "--json" => a.json = true,
            "--no-launch" => a.no_launch = true,
            "--timeout" => a.timeout = need(&mut it, "--timeout")?.parse().map_err(|_| "--timeout must be a number")?,
            "-h" | "--help" => a.command = "help".into(),
            "--" => {
                a.text_parts.extend(it.by_ref().cloned());
            }
            s if s.starts_with('-') && s.len() > 1 => return Err(format!("unknown option {s}")),
            s => a.text_parts.push(s.to_string()),
        }
    }
    if a.command == "open" {
        a.open = true;
        a.command = "speak".into();
    }
    Ok(a)
}

fn read_text(a: &Args) -> Result<String, String> {
    if let Some(f) = &a.file {
        if f == "-" {
            return read_stdin();
        }
        return std::fs::read_to_string(f).map_err(|e| format!("cannot read {f}: {e}"));
    }
    if a.stdin {
        return read_stdin();
    }
    if !a.text_parts.is_empty() {
        return Ok(a.text_parts.join(" "));
    }
    if !io::stdin().is_terminal() {
        return read_stdin();
    }
    Err("no text given. Pass text, use -f <file>, or pipe something in.".into())
}

fn read_stdin() -> Result<String, String> {
    let mut buf = Vec::new();
    io::stdin().read_to_end(&mut buf).map_err(|e| e.to_string())?;
    if buf.len() > proto::MAX_TEXT_BYTES {
        return Err(format!("input is {} bytes; the limit is {} bytes", buf.len(), proto::MAX_TEXT_BYTES));
    }
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

fn socket_path() -> Option<PathBuf> {
    std::env::var_os("SPEECH_HUB_SOCKET").map(PathBuf::from).or_else(proto::default_socket_path)
}

fn connect(path: &PathBuf, launch: bool, timeout: Duration) -> Result<UnixStream, String> {
    if let Ok(s) = UnixStream::connect(path) {
        return Ok(s);
    }
    if !launch {
        return Err("Speech Hub is not running (and --no-launch was given)".into());
    }
    // Launch the app in the background without stealing focus, then wait for the socket.
    let status = std::process::Command::new("/usr/bin/open").args(["-g", "-b", proto::BUNDLE_ID]).status();
    if !matches!(status, Ok(s) if s.success()) {
        return Err("could not launch Speech Hub. Is it installed in /Applications?".into());
    }
    let start = Instant::now();
    while start.elapsed() < timeout {
        std::thread::sleep(Duration::from_millis(200));
        if let Ok(s) = UnixStream::connect(path) {
            return Ok(s);
        }
    }
    Err(format!("Speech Hub did not start within {:.0}s", timeout.as_secs_f32()))
}

fn send(stream: &mut UnixStream, env: &Envelope) -> Result<Response, String> {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(600)));
    let bytes = serde_json::to_vec(env).map_err(|e| e.to_string())?;
    frame::write(stream, &bytes).map_err(|e| format!("send failed: {e}"))?;
    let reply = frame::read(stream).map_err(|e| format!("no reply: {e}"))?;
    serde_json::from_slice(&reply).map_err(|e| format!("bad reply: {e}"))
}

fn request_id() -> String {
    let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    format!("cli-{}-{n:x}", std::process::id())
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let a = match parse(&argv) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("speak: {e}\n\nRun `speak --help` for usage.");
            return ExitCode::from(EXIT_USAGE);
        }
    };
    if a.command == "help" {
        print!("{HELP}");
        return ExitCode::from(EXIT_OK);
    }
    let options = SpeakOptions {
        voice_id: a.voice.clone(),
        speed: a.speed.or(a.rate_wpm.map(|w| (w / 175.0).clamp(0.5, 3.0))),
        title: a.title.clone(),
    };
    let request = match a.command.as_str() {
        "speak" => {
            let text = match read_text(&a) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("speak: {e}");
                    return ExitCode::from(EXIT_USAGE);
                }
            };
            if let Some(out) = &a.output {
                let abs = std::env::current_dir().map(|d| d.join(out)).unwrap_or_else(|_| PathBuf::from(out));
                Request::Render { text, options, path: abs.display().to_string() }
            } else if a.open {
                Request::Open { text, options }
            } else {
                Request::Speak { text, options }
            }
        }
        "run" => match a.text_parts.first() {
            Some(s) => Request::RunSource { source_id: s.clone() },
            None => {
                eprintln!("speak: run needs a Source name or id");
                return ExitCode::from(EXIT_USAGE);
            }
        },
        "palette" => Request::Palette,
        "stop" => Request::Stop,
        "pause" => Request::Pause,
        "resume" => Request::Resume,
        "status" => Request::Status,
        "voices" => Request::ListVoices,
        "sources" => Request::ListSources,
        "version" => Request::Version,
        _ => unreachable!(),
    };
    let Some(path) = socket_path() else {
        eprintln!("speak: cannot determine the socket path (HOME unset?)");
        return ExitCode::from(EXIT_UNAVAILABLE);
    };
    // Pure queries shouldn't boot the app just to say "not running".
    let launch = !a.no_launch && !matches!(request, Request::Status | Request::Stop | Request::Pause | Request::Resume | Request::Version);
    let mut stream = match connect(&path, launch, Duration::from_secs_f32(a.timeout)) {
        Ok(s) => s,
        Err(e) => {
            if a.json {
                println!("{}", serde_json::json!({ "ok": false, "code": "unavailable", "message": e }));
            } else if matches!(request, Request::Status) && !a.no_launch {
                println!("Speech Hub is not running.");
                return ExitCode::from(EXIT_OK);
            } else {
                eprintln!("speak: {e}");
            }
            return ExitCode::from(EXIT_UNAVAILABLE);
        }
    };
    let env = Envelope { version: proto::PROTOCOL_VERSION, request_id: request_id(), request: request.clone() };
    match send(&mut stream, &env) {
        Ok(Response::Ok { data, .. }) => {
            if a.json {
                println!("{}", serde_json::to_string_pretty(&data.unwrap_or(serde_json::Value::Null)).unwrap_or_default());
            } else {
                print_human(&request, data.as_ref());
            }
            ExitCode::from(EXIT_OK)
        }
        Ok(Response::Error { code, message, .. }) => {
            if a.json {
                println!("{}", serde_json::json!({ "ok": false, "code": code, "message": message }));
            } else {
                eprintln!("speak: {message}");
            }
            ExitCode::from(match code {
                proto::ErrorCode::EngineNotReady | proto::ErrorCode::PermissionRequired => EXIT_NOT_READY,
                _ => EXIT_REQUEST,
            })
        }
        Err(e) => {
            eprintln!("speak: {e}");
            ExitCode::from(EXIT_UNAVAILABLE)
        }
    }
}

fn print_human(request: &Request, data: Option<&serde_json::Value>) {
    let Some(d) = data else { return };
    match request {
        Request::Status => {
            let state = d["state"].as_str().unwrap_or("?");
            let engine = d["engine"]["name"].as_str().unwrap_or("?");
            let voice = d["voice_id"].as_str().unwrap_or("?");
            match d["title"].as_str() {
                Some(t) => println!("{state}: {t}  ({engine}, {voice})"),
                None => println!("{state}  ({engine}, {voice}, {}×)", d["speed"].as_f64().unwrap_or(1.0)),
            }
        }
        Request::ListVoices => {
            if let Some(arr) = d.as_array() {
                for v in arr {
                    println!("{:<16} {:<6} {}", v["id"].as_str().unwrap_or(""), v["language"].as_str().unwrap_or(""), v["name"].as_str().unwrap_or(""));
                }
            }
        }
        Request::ListSources => {
            match d.as_array() {
                Some(arr) if !arr.is_empty() => {
                    for s in arr {
                        println!("{:<28} {:<10} {}", s["name"].as_str().unwrap_or(""), s["type"].as_str().unwrap_or(""), s["id"].as_str().unwrap_or(""));
                    }
                }
                _ => println!("No Sources yet. Create one in Speech Hub → Preferences → Sources."),
            }
        }
        Request::Version => println!("Speech Hub {} (protocol v{})", d["app"].as_str().unwrap_or("?"), d["protocol"]),
        Request::Render { .. } => println!("Wrote {} ({:.1}s)", d["path"].as_str().unwrap_or(""), d["seconds"].as_f64().unwrap_or(0.0)),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> Args {
        parse(&s.split_whitespace().map(String::from).collect::<Vec<_>>()).unwrap()
    }

    #[test]
    fn default_is_speak_with_text() {
        let a = p("hello there world");
        assert_eq!(a.command, "speak");
        assert_eq!(a.text_parts.join(" "), "hello there world");
    }

    #[test]
    fn say_compatible_flags() {
        let a = p("-v af_heart -r 210 -f notes.txt -o out.wav");
        assert_eq!(a.voice.as_deref(), Some("af_heart"));
        assert_eq!(a.rate_wpm, Some(210.0));
        assert_eq!(a.file.as_deref(), Some("notes.txt"));
        assert_eq!(a.output.as_deref(), Some("out.wav"));
    }

    #[test]
    fn subcommands() {
        assert_eq!(p("run morning-briefing").command, "run");
        assert_eq!(p("sources list --json").command, "sources");
        let a = p("open some text");
        assert!(a.open);
        assert_eq!(a.text_parts.join(" "), "some text");
    }

    #[test]
    fn rejects_unknown_option() {
        assert!(parse(&["--bogus".to_string()]).is_err());
    }
}
