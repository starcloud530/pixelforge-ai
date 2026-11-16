//! Read-only loopback preview.
//!
//! `pixelforge play` reads a project, composites each frame with the same
//! RGBA path as export, and serves those pictures plus the APNG playback
//! order. The page is bundled in this binary. Playback controls stay in the
//! browser and do not write the project, a checkpoint, or the edit lock.
//! Speed scales the preview clock only.
//!
//! The listener is `127.0.0.1`. A request whose `Host` is not that loopback
//! name is refused, so a DNS-rebinding page cannot read the snapshot. The
//! URL carries an unguessable token. There is no write route.

use std::fs;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use pixelforge_kernel::{playback_order, plays_for, Document, LoopMode, TOOL_API_VERSION};
use serde_json::{json, Value};

use crate::failure::CliFailure;
use crate::store;

const HEADER_LIMIT: usize = 8192;
const POLL_EVERY: Duration = Duration::from_millis(200);

struct Stop {
    flag: AtomicBool,
    pair: Mutex<()>,
    wake: Condvar,
}

impl Stop {
    fn new() -> Self {
        Self {
            flag: AtomicBool::new(false),
            pair: Mutex::new(()),
            wake: Condvar::new(),
        }
    }

    fn request(&self) {
        let _guard = self.pair.lock().unwrap_or_else(|err| err.into_inner());
        self.flag.store(true, Ordering::Relaxed);
        self.wake.notify_all();
    }

    fn wait(&self, dur: Duration) -> bool {
        let guard = self.pair.lock().unwrap_or_else(|err| err.into_inner());
        if self.flag.load(Ordering::Relaxed) {
            return true;
        }
        let (guard, _) = self
            .wake
            .wait_timeout(guard, dur)
            .unwrap_or_else(|err| err.into_inner());
        drop(guard);
        self.flag.load(Ordering::Relaxed)
    }
}

struct Loaded {
    hash: u64,
    revision: String,
    body: Arc<[u8]>,
}

struct Shared {
    revision: String,
    raw_hash: u64,
    failed: bool,
    ok_body: Arc<[u8]>,
    err_body: Arc<[u8]>,
}

impl Shared {
    fn ready(loaded: Loaded) -> Self {
        Self {
            revision: loaded.revision,
            raw_hash: loaded.hash,
            failed: false,
            ok_body: loaded.body,
            err_body: Arc::from([]),
        }
    }

    fn fail(&mut self, message: &str) {
        self.failed = true;
        let body = serde_json::to_vec(&json!({
            "ok": false,
            "revision": self.revision,
            "error": message,
        }))
        .expect("error snapshot is JSON");
        self.err_body = Arc::from(body);
    }
}

pub(crate) enum Opener {
    System,
    Off,
    Program(String),
}

pub(crate) struct Launch {
    pub project: PathBuf,
    pub port: Option<u16>,
}

pub(crate) struct Running {
    url: String,
    port: u16,
    local_ip: Ipv4Addr,
    stop: Arc<Stop>,
    accept: Option<JoinHandle<()>>,
    watch: Option<JoinHandle<()>>,
}

impl Running {
    pub(crate) fn url(&self) -> &str {
        &self.url
    }

    pub(crate) fn port(&self) -> u16 {
        self.port
    }

    pub(crate) fn local_ip(&self) -> Ipv4Addr {
        self.local_ip
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        self.stop.request();
        if let Some(handle) = self.accept.take() {
            let _ = handle.join();
        }
        if let Some(handle) = self.watch.take() {
            let _ = handle.join();
        }
    }
}

pub(crate) fn serve_cli(
    project: &Path,
    port: Option<u16>,
    open_browser: bool,
) -> Result<(), CliFailure> {
    let opener = if open_browser {
        opener_from_env()
    } else {
        Opener::Off
    };
    let running = Running::start(Launch {
        project: project.to_path_buf(),
        port,
    })?;
    let body = json!({
        "ok": true,
        "apiVersion": TOOL_API_VERSION,
        "result": {
            "url": running.url(),
            "host": running.local_ip().to_string(),
            "port": running.port(),
            "open": open_browser,
        }
    });
    let _ = print_json(&body);
    eprintln!("pixelforge: player at {}", running.url());
    eprintln!("pixelforge: press Ctrl+C to stop");
    match opener {
        Opener::Off => eprintln!("pixelforge: not opening a browser"),
        ref other => {
            if let Err(err) = launch(other, running.url()) {
                eprintln!("pixelforge: could not open a browser: {err}");
                eprintln!("pixelforge: open this URL manually: {}", running.url());
            }
        }
    }
    let _hold = running;
    loop {
        thread::park();
    }
}

impl Running {
    pub(crate) fn start(launch: Launch) -> Result<Self, CliFailure> {
        let label = file_label(&launch.project);
        let loaded = load_path(&launch.project, &label)?;
        let listener = bind_loopback(launch.port)?;
        let addr = listener.local_addr().map_err(|err| {
            CliFailure::new("Cli", format!("cannot read the preview address: {err}"))
        })?;
        let std::net::IpAddr::V4(local_ip) = addr.ip() else {
            return Err(CliFailure::new(
                "Cli",
                format!("preview listener is {}, not 127.0.0.1", addr.ip()),
            ));
        };
        if local_ip != Ipv4Addr::LOCALHOST {
            return Err(CliFailure::new(
                "Cli",
                format!("preview listener is {local_ip}, not 127.0.0.1"),
            ));
        }
        listener.set_nonblocking(true).map_err(|err| {
            CliFailure::new("Cli", format!("cannot set the preview listener: {err}"))
        })?;
        let token = random_token()?;
        let url = format!("http://127.0.0.1:{}/{token}/", addr.port());
        let shared = Arc::new(Mutex::new(Shared::ready(loaded)));
        let stop = Arc::new(Stop::new());
        let page = page_bytes();
        let ready = Arc::new(AtomicBool::new(false));

        let watch_stop = Arc::clone(&stop);
        let watch_shared = Arc::clone(&shared);
        let watch_path = launch.project.clone();
        let watch_label = label;
        let watch = thread::Builder::new()
            .name("pixelforge-play-watch".into())
            .spawn(move || watch_loop(&watch_path, &watch_label, &watch_shared, &watch_stop))
            .map_err(|err| CliFailure::new("Cli", format!("cannot watch the project: {err}")))?;

        let accept_stop = Arc::clone(&stop);
        let accept_ready = Arc::clone(&ready);
        let accept = thread::Builder::new()
            .name("pixelforge-play-http".into())
            .spawn(move || {
                accept_ready.store(true, Ordering::Relaxed);
                accept_loop(listener, addr.port(), token, page, shared, &accept_stop);
            })
            .map_err(|err| CliFailure::new("Cli", format!("cannot serve the preview: {err}")))?;

        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while !ready.load(Ordering::Relaxed) {
            if std::time::Instant::now() > deadline {
                return Err(CliFailure::new("Cli", "preview listener did not start"));
            }
            thread::sleep(Duration::from_millis(1));
        }

        Ok(Self {
            url,
            port: addr.port(),
            local_ip,
            stop,
            accept: Some(accept),
            watch: Some(watch),
        })
    }
}

fn opener_from_env() -> Opener {
    match std::env::var("PIXELFORGE_BROWSER") {
        Ok(program) if !program.is_empty() => Opener::Program(program),
        _ => Opener::System,
    }
}

pub(crate) fn launch(opener: &Opener, url: &str) -> Result<(), String> {
    match opener {
        Opener::Off => Ok(()),
        Opener::System => system_open(url),
        Opener::Program(program) => run_program(program, url),
    }
}

fn system_open(url: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        run_program("open", url)
    }
    #[cfg(target_os = "windows")]
    {
        let status = Command::new("cmd")
            .args(["/C", "start", "", url])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map_err(|err| format!("cannot run cmd: {err}"))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("cmd exited with {status}"))
        }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        run_program("xdg-open", url)
    }
}

fn run_program(program: &str, url: &str) -> Result<(), String> {
    let status = Command::new(program)
        .arg(url)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|err| format!("cannot run {program}: {err}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} exited with {status}"))
    }
}

fn bind_loopback(port: Option<u16>) -> Result<TcpListener, CliFailure> {
    let port = port.unwrap_or(0);
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    TcpListener::bind(addr).map_err(|err| {
        if err.kind() == std::io::ErrorKind::AddrInUse {
            CliFailure::new(
                "AddressInUse",
                format!("127.0.0.1:{port} is already in use"),
            )
            .with_hint("choose another --port, or omit --port to use a free port")
        } else {
            CliFailure::new("Cli", format!("cannot listen on 127.0.0.1:{port}: {err}"))
        }
    })
}

fn random_token() -> Result<String, CliFailure> {
    let mut bytes = [0u8; 16];
    fill_random(&mut bytes)
        .map_err(|err| CliFailure::new("Cli", format!("cannot generate a preview token: {err}")))?;
    let mut token = String::with_capacity(32);
    for byte in bytes {
        token.push_str(&format!("{byte:02x}"));
    }
    Ok(token)
}

#[cfg(unix)]
fn fill_random(buf: &mut [u8]) -> std::io::Result<()> {
    let mut file = fs::File::open("/dev/urandom")?;
    file.read_exact(buf)
}

#[cfg(windows)]
fn fill_random(buf: &mut [u8]) -> std::io::Result<()> {
    #[link(name = "advapi32")]
    extern "system" {
        fn SystemFunction036(random_buffer: *mut u8, random_buffer_length: u32) -> u8;
    }
    if buf.len() > u32::MAX as usize {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "token too large",
        ));
    }
    // SAFETY: `buf` is a writable byte slice and the length fits in `u32`.
    // `SystemFunction036` writes exactly that many random bytes.
    let ok = unsafe { SystemFunction036(buf.as_mut_ptr(), buf.len() as u32) };
    if ok == 0 {
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "SystemFunction036 failed",
        ))
    } else {
        Ok(())
    }
}

fn watch_loop(path: &Path, label: &str, shared: &Mutex<Shared>, stop: &Stop) {
    while !stop.wait(POLL_EVERY) {
        refresh(path, label, shared);
    }
}

fn refresh(path: &Path, label: &str, shared: &Mutex<Shared>) {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) => {
            let mut guard = lock_shared(shared);
            guard.fail(&format!("cannot read {}: {err}", path.display()));
            return;
        }
    };
    let hash = fnv(&bytes);
    {
        let mut guard = lock_shared(shared);
        if hash == guard.raw_hash {
            guard.failed = false;
            return;
        }
    }
    match load_bytes(&bytes, label) {
        Ok(loaded) => {
            let mut guard = lock_shared(shared);
            guard.raw_hash = loaded.hash;
            guard.revision = loaded.revision;
            guard.ok_body = loaded.body;
            guard.failed = false;
        }
        Err(message) => {
            let mut guard = lock_shared(shared);
            guard.fail(&message);
        }
    }
}

fn lock_shared(shared: &Mutex<Shared>) -> std::sync::MutexGuard<'_, Shared> {
    shared.lock().unwrap_or_else(|err| err.into_inner())
}

fn load_path(path: &Path, label: &str) -> Result<Loaded, CliFailure> {
    let bytes = fs::read(path).map_err(|err| CliFailure::io(path, err))?;
    load_bytes(&bytes, label).map_err(|message| CliFailure::new("ProjectFile", message))
}

fn load_bytes(bytes: &[u8], file_name: &str) -> Result<Loaded, String> {
    if bytes.iter().all(|byte| byte.is_ascii_whitespace()) {
        return Err("project file is empty".to_string());
    }
    let text = std::str::from_utf8(bytes).map_err(|_| "project file is not UTF-8".to_string())?;
    let document = Document::load_project(text).map_err(|err| err.to_string())?;
    let value = snapshot_value(&document, file_name)?;
    let revision = value["revision"]
        .as_str()
        .ok_or_else(|| "snapshot has no revision".to_string())?
        .to_string();
    let body = serde_json::to_vec(&value).map_err(|err| err.to_string())?;
    Ok(Loaded {
        hash: fnv(bytes),
        revision,
        body: Arc::from(body),
    })
}

fn snapshot_value(document: &Document, file_name: &str) -> Result<Value, String> {
    let mut frames = Vec::with_capacity(document.frame_count());
    for index in 0..document.frame_count() {
        let png = document
            .export_frame_png(index)
            .map_err(|err| err.to_string())?;
        let delay_ms = document.frame_delay(index).map_err(|err| err.to_string())?;
        frames.push(json!({
            "index": index,
            "delayMs": delay_ms,
            "png": STANDARD.encode(png),
        }));
    }
    let actions = if document.tags().is_empty() {
        let last = document.frame_count() - 1;
        vec![action_value(
            document,
            "timeline",
            "全部帧",
            "timeline",
            0,
            last,
            LoopMode::Forward,
        )?]
    } else {
        let mut actions = Vec::with_capacity(document.tags().len());
        for tag in document.tags() {
            actions.push(action_value(
                document,
                &tag.name,
                &tag.name,
                "tag",
                tag.from,
                tag.to,
                tag.loop_mode,
            )?);
        }
        actions
    };
    let revision = store::revision_of(&document.save_project());
    Ok(json!({
        "ok": true,
        "revision": revision,
        "fileName": file_name,
        "width": document.width(),
        "height": document.height(),
        "readOnly": true,
        "frames": frames,
        "actions": actions,
    }))
}

fn action_value(
    document: &Document,
    id: &str,
    name: &str,
    kind: &str,
    from: usize,
    to: usize,
    mode: LoopMode,
) -> Result<Value, String> {
    let order = playback_order(from, to, mode);
    let mut steps = Vec::with_capacity(order.len());
    for index in order {
        let delay_ms = document.frame_delay(index).map_err(|err| err.to_string())?;
        steps.push(json!({
            "frame": index,
            "delayMs": delay_ms,
        }));
    }
    Ok(json!({
        "id": id,
        "name": name,
        "kind": kind,
        "loop": mode.as_str(),
        "plays": plays_for(mode),
        "steps": steps,
    }))
}

fn accept_loop(
    listener: TcpListener,
    port: u16,
    token: String,
    page: Arc<[u8]>,
    shared: Arc<Mutex<Shared>>,
    stop: &Stop,
) {
    while !stop.wait(Duration::from_millis(20)) {
        match listener.accept() {
            Ok((stream, _)) => handle_client(stream, port, &token, &page, &shared),
            Err(err)
                if err.kind() == std::io::ErrorKind::WouldBlock
                    || err.kind() == std::io::ErrorKind::TimedOut => {}
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => break,
        }
    }
}

fn handle_client(
    mut stream: TcpStream,
    port: u16,
    token: &str,
    page: &[u8],
    shared: &Mutex<Shared>,
) {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
    let mut buf = [0u8; HEADER_LIMIT];
    let mut filled = 0usize;
    let header_end = loop {
        if filled == buf.len() {
            write_response(
                &mut stream,
                431,
                "Request Header Fields Too Large",
                "text/plain; charset=utf-8",
                b"headers too large\n",
                "",
            );
            return;
        }
        match stream.read(&mut buf[filled..]) {
            Ok(0) => return,
            Ok(count) => {
                filled += count;
                if let Some(end) = find_header_end(&buf[..filled]) {
                    break end;
                }
            }
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return,
        }
    };
    let head = match std::str::from_utf8(&buf[..header_end]) {
        Ok(head) => head,
        Err(_) => {
            write_response(
                &mut stream,
                400,
                "Bad Request",
                "text/plain; charset=utf-8",
                b"bad request\n",
                "",
            );
            return;
        }
    };
    let mut lines = head.split("\r\n");
    let Some(request) = lines.next() else {
        write_response(
            &mut stream,
            400,
            "Bad Request",
            "text/plain; charset=utf-8",
            b"bad request\n",
            "",
        );
        return;
    };
    let mut host = None;
    let mut if_none_match = None;
    for line in lines {
        if let Some(value) = header_value(line, "host") {
            host = Some(value.to_string());
        } else if let Some(value) = header_value(line, "if-none-match") {
            if_none_match = Some(value.to_string());
        }
    }
    let Some(host) = host else {
        write_response(
            &mut stream,
            400,
            "Bad Request",
            "text/plain; charset=utf-8",
            b"missing host\n",
            "",
        );
        return;
    };
    if !host_is_loopback(&host, port) {
        write_response(
            &mut stream,
            403,
            "Forbidden",
            "text/plain; charset=utf-8",
            b"host is not loopback\n",
            "",
        );
        return;
    }
    let mut parts = request.split(' ');
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("");
    let version = parts.next().unwrap_or("");
    if parts.next().is_some() || !(version == "HTTP/1.1" || version == "HTTP/1.0") {
        write_response(
            &mut stream,
            400,
            "Bad Request",
            "text/plain; charset=utf-8",
            b"bad request\n",
            "",
        );
        return;
    }
    if !target.starts_with('/')
        || target.contains('?')
        || target.contains('%')
        || target.contains('\\')
        || target.contains("..")
    {
        write_response(
            &mut stream,
            404,
            "Not Found",
            "text/plain; charset=utf-8",
            b"not found\n",
            "",
        );
        return;
    }
    let route = match route_for(target, token) {
        Some(route) => route,
        None => {
            write_response(
                &mut stream,
                404,
                "Not Found",
                "text/plain; charset=utf-8",
                b"not found\n",
                "",
            );
            return;
        }
    };
    if method != "GET" {
        write_response(
            &mut stream,
            405,
            "Method Not Allowed",
            "text/plain; charset=utf-8",
            b"read only\n",
            "Allow: GET\r\n",
        );
        return;
    }
    match route {
        Route::Page => write_response(&mut stream, 200, "OK", "text/html; charset=utf-8", page, ""),
        Route::Snapshot => {
            let guard = lock_shared(shared);
            if guard.failed {
                let body = Arc::clone(&guard.err_body);
                drop(guard);
                write_response(
                    &mut stream,
                    200,
                    "OK",
                    "application/json; charset=utf-8",
                    &body,
                    "",
                );
                return;
            }
            if etag_matches(if_none_match.as_deref(), &guard.revision) {
                let etag = format!("ETag: \"{}\"\r\n", guard.revision);
                drop(guard);
                write_response(
                    &mut stream,
                    304,
                    "Not Modified",
                    "application/json; charset=utf-8",
                    b"",
                    &etag,
                );
                return;
            }
            let body = Arc::clone(&guard.ok_body);
            let etag = format!("ETag: \"{}\"\r\n", guard.revision);
            drop(guard);
            write_response(
                &mut stream,
                200,
                "OK",
                "application/json; charset=utf-8",
                &body,
                &etag,
            );
        }
    }
}

enum Route {
    Page,
    Snapshot,
}

fn route_for(path: &str, token: &str) -> Option<Route> {
    let mut parts = path.split('/').filter(|part| !part.is_empty());
    let first = parts.next()?;
    let second = parts.next();
    if parts.next().is_some() || first != token {
        return None;
    }
    match second {
        None => Some(Route::Page),
        Some("snapshot") => Some(Route::Snapshot),
        Some(_) => None,
    }
}

fn host_is_loopback(host: &str, port: u16) -> bool {
    let host = host.trim();
    let v4 = format!("127.0.0.1:{port}");
    let name = format!("localhost:{port}");
    host.eq_ignore_ascii_case(&v4) || host.eq_ignore_ascii_case(&name)
}

fn etag_matches(header: Option<&str>, revision: &str) -> bool {
    let Some(header) = header else {
        return false;
    };
    header
        .split(',')
        .any(|part| part.trim().trim_matches('"') == revision)
}

fn header_value<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    let (key, value) = line.split_once(':')?;
    if key.eq_ignore_ascii_case(name) {
        Some(value.trim())
    } else {
        None
    }
}

fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|index| index + 4)
}

fn write_response(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    content_type: &str,
    body: &[u8],
    extra: &str,
) {
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {len}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nCross-Origin-Resource-Policy: same-origin\r\nContent-Security-Policy: default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data:; connect-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'\r\nConnection: close\r\n{extra}\r\n",
        len = body.len(),
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body);
}

fn page_bytes() -> Arc<[u8]> {
    const SHELL: &str = include_str!("page.html");
    const SCRIPT: &str = include_str!("playback.js");
    Arc::from(SHELL.replace("/*__PLAYBACK_JS__*/", SCRIPT).into_bytes())
}

fn file_label(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "project".to_string())
}

fn fnv(bytes: &[u8]) -> u64 {
    const OFFSET: u64 = 14_695_981_039_346_656_037;
    const PRIME: u64 = 1_099_511_628_211;
    let mut hash = OFFSET;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

fn print_json(value: &Value) -> bool {
    let line = match serde_json::to_string(value) {
        Ok(line) => line,
        Err(_) => return false,
    };
    let mut out = std::io::stdout().lock();
    writeln!(out, "{line}").is_ok() && out.flush().is_ok()
}

#[cfg(test)]
mod tests {
    use super::{launch, snapshot_value, Launch, Opener, Running};
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine;
    use pixelforge_kernel::{playback_order, plays_for, ApngSource, Document, LoopMode};
    use serde_json::Value;
    use std::fs;
    use std::io::{Read, Write};
    use std::net::{Ipv4Addr, TcpListener, TcpStream};
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
    use std::thread;
    use std::time::Duration;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct Scratch {
        path: PathBuf,
    }

    impl Scratch {
        fn new() -> Self {
            static N: AtomicU64 = AtomicU64::new(0);
            let n = N.fetch_add(1, AtomicOrdering::Relaxed);
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "pixelforge-play-{nanos}-{n}-{}",
                std::process::id()
            ));
            fs::create_dir_all(&path).unwrap();
            Self { path }
        }

        fn child(&self, name: &str) -> PathBuf {
            self.path.join(name)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn tagged_clip() -> Document {
        let mut doc = Document::new(4, 2).unwrap();
        for _ in 0..3 {
            doc.add_frame();
        }
        let delays = [40, 200, 80, 120];
        for index in 0..4 {
            doc.set_frame_delay(index, delays[index]).unwrap();
            doc.set_active_frame(index).unwrap();
            doc.paint(index as u16, 0, (index as u8) + 1).unwrap();
        }
        doc.add_tag("go", 0, 2, LoopMode::Forward).unwrap();
        doc.add_tag("back", 0, 2, LoopMode::Reverse).unwrap();
        doc.add_tag("bounce", 0, 3, LoopMode::PingPong).unwrap();
        doc.add_tag("hit", 1, 2, LoopMode::Once).unwrap();
        doc
    }

    fn write_project(path: &Path, document: &Document) {
        fs::write(path, document.save_project()).unwrap();
    }

    fn replace_atomic(path: &Path, bytes: &[u8]) {
        let tmp = path.with_extension(format!(
            "tmp-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::write(&tmp, bytes).unwrap();
        fs::rename(&tmp, path).unwrap();
    }

    fn lock_path(path: &Path) -> PathBuf {
        let mut name = path.file_name().unwrap().to_os_string();
        name.push(".pixelforge.lock");
        path.parent().unwrap().join(name)
    }

    struct Response {
        status: u16,
        headers: String,
        body: Vec<u8>,
    }

    fn request(port: u16, method: &str, path: &str, host: &str, extra: &str) -> Response {
        let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let head =
            format!("{method} {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n{extra}\r\n");
        stream.write_all(head.as_bytes()).unwrap();
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).unwrap();
        let split = buf
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .expect("headers");
        let headers = String::from_utf8(buf[..split].to_vec()).unwrap();
        let status: u16 = headers.split_whitespace().nth(1).unwrap().parse().unwrap();
        Response {
            status,
            headers,
            body: buf[split + 4..].to_vec(),
        }
    }

    fn snapshot(port: u16, url_path: &str) -> Value {
        let response = request(port, "GET", url_path, &format!("127.0.0.1:{port}"), "");
        assert_eq!(
            response.status,
            200,
            "{}",
            String::from_utf8_lossy(&response.body)
        );
        serde_json::from_slice(&response.body).unwrap()
    }

    fn token_of(url: &str) -> String {
        url.trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap()
            .to_string()
    }

    #[test]
    fn snapshot_matches_export_playback_and_composites_layers() {
        let mut doc = tagged_clip();
        let before = doc.save_project();
        let value = snapshot_value(&doc, "clip.json").unwrap();
        assert_eq!(doc.save_project(), before);
        assert_eq!(value["readOnly"], true);
        assert!(value["frames"].as_array().unwrap().len() == 4);
        for frame in value["frames"].as_array().unwrap() {
            let index = frame["index"].as_u64().unwrap() as usize;
            let png = STANDARD.decode(frame["png"].as_str().unwrap()).unwrap();
            assert_eq!(png, doc.export_frame_png(index).unwrap());
            assert_eq!(
                frame["delayMs"].as_u64().unwrap(),
                u64::from(doc.frame_delay(index).unwrap())
            );
        }
        let modes = [
            ("go", LoopMode::Forward, 0),
            ("back", LoopMode::Reverse, 0),
            ("bounce", LoopMode::PingPong, 0),
            ("hit", LoopMode::Once, 1),
        ];
        for (name, mode, plays) in modes {
            let action = value["actions"]
                .as_array()
                .unwrap()
                .iter()
                .find(|action| action["id"] == name)
                .unwrap();
            let exported = doc.export_apng(1, ApngSource::Tag(name.into())).unwrap();
            assert_eq!(exported.plays, plays_for(mode));
            assert_eq!(exported.plays, plays);
            assert_eq!(action["plays"].as_u64().unwrap(), u64::from(plays));
            assert_eq!(action["loop"], mode.as_str());
            let steps = action["steps"].as_array().unwrap();
            assert_eq!(steps.len(), exported.frames.len());
            let order = playback_order(
                doc.tags().iter().find(|tag| tag.name == name).unwrap().from,
                doc.tags().iter().find(|tag| tag.name == name).unwrap().to,
                mode,
            );
            for ((step, exported_frame), index) in steps.iter().zip(&exported.frames).zip(&order) {
                assert_eq!(
                    step["frame"].as_u64().unwrap(),
                    u64::try_from(*index).unwrap()
                );
                assert_eq!(exported_frame.index as usize, *index);
                assert_eq!(
                    step["delayMs"].as_u64().unwrap(),
                    u64::from(exported_frame.delay_ms)
                );
            }
        }
        let bounce = value["actions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|action| action["id"] == "bounce")
            .unwrap();
        let bounce_steps: Vec<u64> = bounce["steps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|step| step["frame"].as_u64().unwrap())
            .collect();
        assert_eq!(bounce_steps, vec![0, 1, 2, 3, 2, 1]);

        doc.set_active_frame(0).unwrap();
        doc.add_layer(Some("top")).unwrap();
        doc.paint(0, 0, 2).unwrap();
        let covered = doc.export_frame_png(0).unwrap();
        doc.set_layer_visible(1, false).unwrap();
        let hidden = doc.export_frame_png(0).unwrap();
        assert_ne!(covered, hidden);
        let layered = snapshot_value(&doc, "layered.json").unwrap();
        let png = STANDARD
            .decode(layered["frames"][0]["png"].as_str().unwrap())
            .unwrap();
        assert_eq!(png, hidden);
        assert_ne!(png, covered);
    }

    #[test]
    fn timeline_without_tags_and_a_single_frame_use_the_export_order() {
        let mut bare = Document::new(2, 2).unwrap();
        bare.add_frame();
        bare.set_frame_delay(0, 30).unwrap();
        bare.set_frame_delay(1, 90).unwrap();
        let value = snapshot_value(&bare, "bare.json").unwrap();
        assert_eq!(value["actions"].as_array().unwrap().len(), 1);
        assert_eq!(value["actions"][0]["id"], "timeline");
        assert_eq!(value["actions"][0]["kind"], "timeline");
        assert_eq!(value["actions"][0]["loop"], "forward");
        assert_eq!(value["actions"][0]["plays"], 0);
        let frames: Vec<u64> = value["actions"][0]["steps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|step| step["frame"].as_u64().unwrap())
            .collect();
        assert_eq!(frames, vec![0, 1]);
        assert_eq!(value["actions"][0]["steps"][0]["delayMs"], 30);
        assert_eq!(value["actions"][0]["steps"][1]["delayMs"], 90);

        let one = Document::new(2, 2).unwrap();
        let single = snapshot_value(&one, "one.json").unwrap();
        assert_eq!(single["frames"].as_array().unwrap().len(), 1);
        assert_eq!(single["actions"][0]["steps"].as_array().unwrap().len(), 1);
        assert_eq!(single["actions"][0]["plays"], 0);
    }

    #[test]
    fn server_is_loopback_read_only_and_reload_is_atomic() {
        let scratch = Scratch::new();
        let path = scratch.child("hero.json");
        let original = tagged_clip();
        write_project(&path, &original);
        let bytes = fs::read(&path).unwrap();
        let running = Running::start(Launch {
            project: path.clone(),
            port: None,
        })
        .unwrap();
        assert_eq!(running.local_ip(), Ipv4Addr::LOCALHOST);
        assert_ne!(running.port(), 0);
        let token = token_of(running.url());
        let page_path = format!("/{token}/");
        let snap_path = format!("/{token}/snapshot");
        let host = format!("127.0.0.1:{}", running.port());

        let page = request(running.port(), "GET", &page_path, &host, "");
        assert_eq!(page.status, 200);
        let html = String::from_utf8(page.body).unwrap();
        assert!(html.contains("function createPlayback"));
        assert!(!html.contains("/*__PLAYBACK_JS__*/"));
        assert!(!html.contains("http://"));
        assert!(!html.contains("https://"));
        for banned in ["unpkg", "jsdelivr", "googleapis", "cdnjs", "cdn."] {
            assert!(!html.to_ascii_lowercase().contains(banned), "{banned}");
        }
        assert!(page
            .headers
            .to_ascii_lowercase()
            .contains("cross-origin-resource-policy: same-origin"));
        assert!(page
            .headers
            .to_ascii_lowercase()
            .contains("default-src 'none'"));
        assert!(!page
            .headers
            .to_ascii_lowercase()
            .contains("access-control-allow-origin"));

        let first = snapshot(running.port(), &snap_path);
        assert_eq!(first["actions"][0]["id"], "go");
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert!(!lock_path(&path).exists());
        for _ in 0..8 {
            let again = request(running.port(), "GET", &snap_path, &host, "");
            assert_eq!(again.status, 200);
        }
        assert_eq!(fs::read(&path).unwrap(), bytes);
        let names: Vec<_> = fs::read_dir(&scratch.path)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(names, vec![std::ffi::OsString::from("hero.json")]);

        let etag = format!(
            "If-None-Match: \"{}\"\r\n",
            first["revision"].as_str().unwrap()
        );
        let not_modified = request(running.port(), "GET", &snap_path, &host, &etag);
        assert_eq!(not_modified.status, 304);
        assert!(not_modified.body.is_empty());

        let posted = request(running.port(), "POST", &snap_path, &host, "");
        assert_eq!(posted.status, 405);
        assert_eq!(String::from_utf8(posted.body).unwrap(), "read only\n");
        let foreign = request(running.port(), "GET", &snap_path, "evil.example:80", "");
        assert_eq!(foreign.status, 403);
        let traversal = request(
            running.port(),
            "GET",
            &format!("/{token}/../snapshot"),
            &host,
            "",
        );
        assert_eq!(traversal.status, 404);
        let encoded = request(
            running.port(),
            "GET",
            &format!("/{token}/%2e%2e/snapshot"),
            &host,
            "",
        );
        assert_eq!(encoded.status, 404);
        let missing = request(running.port(), "GET", "/no-token/snapshot", &host, "");
        assert_eq!(missing.status, 404);
        assert_eq!(fs::read(&path).unwrap(), bytes);

        fs::write(&path, b"").unwrap();
        let empty = wait_error(running.port(), &snap_path, "empty");
        assert!(empty.get("frames").is_none());
        assert!(empty.get("actions").is_none());
        assert_eq!(empty["revision"], first["revision"]);
        let stale = request(
            running.port(),
            "GET",
            &snap_path,
            &host,
            &format!(
                "If-None-Match: \"{}\"\r\n",
                first["revision"].as_str().unwrap()
            ),
        );
        assert_eq!(stale.status, 200);
        assert_eq!(
            serde_json::from_slice::<Value>(&stale.body).unwrap()["ok"],
            false
        );

        fs::write(&path, b"{").unwrap();
        let invalid = wait_error(running.port(), &snap_path, "not valid");
        assert!(invalid.get("frames").is_none());

        let mut next = Document::new(4, 2).unwrap();
        next.add_frame();
        next.set_active_frame(1).unwrap();
        next.paint(1, 1, 4).unwrap();
        next.add_tag("attack", 1, 1, LoopMode::Once).unwrap();
        replace_atomic(&path, next.save_project().as_bytes());
        let updated = wait_ok(running.port(), &snap_path, "attack");
        assert_ne!(updated["revision"], first["revision"]);
        let ids: Vec<_> = updated["actions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|action| action["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, vec!["attack"]);
        assert_eq!(updated["actions"][0]["plays"], 1);
        assert_eq!(updated["actions"][0]["steps"][0]["frame"], 1);
        let png = STANDARD
            .decode(updated["frames"][1]["png"].as_str().unwrap())
            .unwrap();
        assert_eq!(png, next.export_frame_png(1).unwrap());
        assert_ne!(png, original.export_frame_png(1).unwrap());
        assert!(!updated["frames"]
            .as_array()
            .unwrap()
            .iter()
            .any(|frame| frame["png"] == first["frames"][0]["png"]));

        replace_atomic(&path, &bytes);
        let restored = wait_revision(
            running.port(),
            &snap_path,
            first["revision"].as_str().unwrap(),
        );
        assert_eq!(restored["actions"][0]["id"], "go");
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert!(!lock_path(&path).exists());
    }

    #[test]
    fn occupied_port_is_reported_and_auto_port_does_not_reuse_it() {
        let scratch = Scratch::new();
        let path = scratch.child("hero.json");
        write_project(&path, &Document::new(2, 2).unwrap());
        let hold = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = hold.local_addr().unwrap().port();
        let err = match Running::start(Launch {
            project: path.clone(),
            port: Some(port),
        }) {
            Err(err) => err,
            Ok(running) => panic!("occupied port accepted {}", running.port()),
        };
        assert_eq!(err.code(), "AddressInUse");
        assert!(err.message().contains(&format!("127.0.0.1:{port}")));
        assert!(err.hint().unwrap().contains("omit --port"));
        let running = Running::start(Launch {
            project: path,
            port: None,
        })
        .unwrap();
        assert_ne!(running.port(), port);
        assert_eq!(running.local_ip(), Ipv4Addr::LOCALHOST);
    }

    #[test]
    fn a_missing_browser_program_fails_without_stopping_the_listener() {
        let err = launch(
            &Opener::Program("pixelforge-browser-missing".into()),
            "http://127.0.0.1:9/token/",
        )
        .unwrap_err();
        assert!(err.contains("pixelforge-browser-missing"), "{err}");
        let scratch = Scratch::new();
        let path = scratch.child("hero.json");
        write_project(&path, &Document::new(2, 2).unwrap());
        let running = Running::start(Launch {
            project: path,
            port: None,
        })
        .unwrap();
        let failure = launch(
            &Opener::Program("pixelforge-browser-missing".into()),
            running.url(),
        )
        .unwrap_err();
        assert!(failure.contains("pixelforge-browser-missing"), "{failure}");
        let token = token_of(running.url());
        let response = request(
            running.port(),
            "GET",
            &format!("/{token}/snapshot"),
            &format!("127.0.0.1:{}", running.port()),
            "",
        );
        assert_eq!(response.status, 200);
    }

    #[test]
    fn node_checks_a_real_snapshot_clock_and_the_script_suite() {
        let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/player/playback.test.js");
        let output = Command::new("node")
            .arg("--test")
            .arg(&script)
            .output()
            .expect("node is required to test the bundled player");
        assert!(
            output.status.success(),
            "player script tests failed\n{}",
            String::from_utf8_lossy(&output.stderr)
        );

        let value = snapshot_value(&tagged_clip(), "clip.json").unwrap();
        let scratch = Scratch::new();
        let file = scratch.child("snapshot.json");
        fs::write(&file, serde_json::to_vec(&value).unwrap()).unwrap();
        let playback = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/player/playback.js");
        let driver = r#"
const assert = require("node:assert/strict");
const { createPlayback } = require(process.env.PLAYBACK);
const snap = JSON.parse(require("node:fs").readFileSync(process.env.SNAPSHOT, "utf8"));
const hit = snap.actions.find((action) => action.id === "hit");
const player = createPlayback(snap);
player.selectAction("hit", 0);
player.play(0);
const total = hit.steps.reduce((sum, step) => sum + step.delayMs, 0);
player.tick(total - 1);
assert.equal(player.state().finished, false);
player.tick(total);
assert.equal(player.state().finished, true);
assert.equal(player.state().playing, false);
assert.equal(player.state().frame, hit.steps[hit.steps.length - 1].frame);
player.play(total);
assert.equal(player.state().step, 0);
assert.equal(player.state().frame, hit.steps[0].frame);
const delays = player.state().steps.map((step) => step.delayMs).join(",");
player.setSpeed(4, total);
assert.equal(player.state().steps.map((step) => step.delayMs).join(","), delays);
assert.equal(player.state().previewMs, hit.steps[0].delayMs / 4);
"#;
        let driven = Command::new("node")
            .arg("-e")
            .arg(driver)
            .env("PLAYBACK", &playback)
            .env("SNAPSHOT", &file)
            .output()
            .unwrap();
        assert!(
            driven.status.success(),
            "{}",
            String::from_utf8_lossy(&driven.stderr)
        );
    }

    fn wait_error(port: u16, path: &str, needle: &str) -> Value {
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        loop {
            let value = snapshot_or_error(port, path);
            if value["ok"] == false && value["error"].as_str().unwrap_or("").contains(needle) {
                return value;
            }
            if std::time::Instant::now() > deadline {
                panic!("timed out waiting for {needle}: {value}");
            }
            thread::sleep(Duration::from_millis(40));
        }
    }

    fn wait_ok(port: u16, path: &str, action: &str) -> Value {
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        loop {
            let value = snapshot_or_error(port, path);
            let ids = value["actions"]
                .as_array()
                .map(|actions| actions.iter().any(|item| item["id"] == action));
            if value["ok"] == true && ids == Some(true) {
                return value;
            }
            if std::time::Instant::now() > deadline {
                panic!("timed out waiting for {action}: {value}");
            }
            thread::sleep(Duration::from_millis(40));
        }
    }

    fn wait_revision(port: u16, path: &str, revision: &str) -> Value {
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        loop {
            let value = snapshot_or_error(port, path);
            if value["ok"] == true && value["revision"] == revision {
                return value;
            }
            if std::time::Instant::now() > deadline {
                panic!("timed out waiting for {revision}: {value}");
            }
            thread::sleep(Duration::from_millis(40));
        }
    }

    fn snapshot_or_error(port: u16, path: &str) -> Value {
        let response = request(port, "GET", path, &format!("127.0.0.1:{port}"), "");
        assert_eq!(
            response.status,
            200,
            "{}",
            String::from_utf8_lossy(&response.body)
        );
        serde_json::from_slice(&response.body).unwrap()
    }
}
