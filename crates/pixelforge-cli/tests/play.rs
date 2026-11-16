//! The `play` command through the packaged binary.

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::Value;

struct Scratch {
    path: PathBuf,
}

impl Scratch {
    fn new() -> Self {
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "pixelforge-play-cli-{nanos}-{n}-{}",
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

struct Player {
    child: Child,
    stderr: String,
}

impl Drop for Player {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn project(path: &std::path::Path) {
    let output = Command::new(env!("CARGO_BIN_EXE_pixelforge"))
        .args(["new", "--width", "4", "--height", "2", "--output"])
        .arg(path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn start(args: &[&str], browser: Option<&str>) -> Player {
    let mut command = Command::new(env!("CARGO_BIN_EXE_pixelforge"));
    command
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(browser) = browser {
        command.env("PIXELFORGE_BROWSER", browser);
    }
    let mut child = command.spawn().unwrap();
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        let mut reader = BufReader::new(stdout);
        let _ = reader.read_line(&mut line);
        let _ = tx.send(line);
    });
    let stderr_buf = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let collected = std::sync::Arc::clone(&stderr_buf);
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stderr);
        let mut line = String::new();
        while reader.read_line(&mut line).unwrap_or(0) > 0 {
            collected.lock().unwrap().push_str(&line);
            line.clear();
        }
    });
    let line = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("listen json");
    let value: Value =
        serde_json::from_str(line.trim()).unwrap_or_else(|err| panic!("{err} in {line}"));
    assert_eq!(value["ok"], true, "{value}");
    let url = value["result"]["url"].as_str().unwrap().to_string();
    let port = value["result"]["port"].as_u64().unwrap() as u16;
    assert!(url.starts_with(&format!("http://127.0.0.1:{port}/")));
    let deadline = Instant::now() + Duration::from_secs(2);
    let stderr = loop {
        let text = stderr_buf.lock().unwrap().clone();
        if text.contains(&url)
            && text.contains("Ctrl+C")
            && (text.contains("not opening a browser") || text.contains("could not open a browser"))
        {
            break text;
        }
        if Instant::now() > deadline {
            panic!("stderr did not announce the player: {text}");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    Player {
        child,
        stderr: format!("{url}\n{port}\n{stderr}"),
    }
}

fn http(port: u16, method: &str, path: &str) -> (u16, String) {
    let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let host = format!("127.0.0.1:{port}");
    write!(
        stream,
        "{method} {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).unwrap();
    let text = String::from_utf8(buf).unwrap();
    let status = text.split_whitespace().nth(1).unwrap().parse().unwrap();
    (status, text)
}

#[test]
fn no_open_serves_without_writing_and_a_bad_browser_stays_up() {
    let scratch = Scratch::new();
    let path = scratch.child("hero.json");
    project(&path);
    let lock = scratch.child("hero.json.pixelforge.lock");
    // `new` leaves the edit lock sidecar behind. `play` must not create one.
    let _ = fs::remove_file(&lock);
    let before = fs::read(&path).unwrap();
    let player = start(
        &["play", "--project", path.to_str().unwrap(), "--no-open"],
        Some("/bin/false"),
    );
    assert!(player.stderr.contains("not opening a browser"));
    assert!(!player.stderr.contains("could not open a browser"));
    let port: u16 = player.stderr.lines().nth(1).unwrap().parse().unwrap();
    let url = player.stderr.lines().next().unwrap();
    let token = url.trim_end_matches('/').rsplit('/').next().unwrap();
    let (status, page) = http(port, "GET", &format!("/{token}/"));
    assert_eq!(status, 200);
    assert!(page.contains("function createPlayback"));
    assert!(!page.contains("https://"));
    let (status, body) = http(port, "GET", &format!("/{token}/snapshot"));
    assert_eq!(status, 200);
    assert!(body.contains("\"readOnly\":true"));
    assert_eq!(fs::read(&path).unwrap(), before);
    assert!(!lock.exists());
    drop(player);

    let mut player = start(
        &["play", "--project", path.to_str().unwrap()],
        Some("/bin/false"),
    );
    assert!(
        player.child.try_wait().unwrap().is_none(),
        "browser failure stopped the player"
    );
    assert!(player.stderr.contains("could not open a browser"));
    assert!(player.stderr.contains("open this URL manually"));
    let port: u16 = player.stderr.lines().nth(1).unwrap().parse().unwrap();
    let (status, _) = http(port, "GET", "/");
    assert_eq!(status, 404);
    assert_eq!(fs::read(&path).unwrap(), before);
    assert!(!lock.exists());
}

#[test]
fn an_occupied_port_exits_and_a_bad_project_does_not_listen() {
    let scratch = Scratch::new();
    let path = scratch.child("hero.json");
    project(&path);
    let hold = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = hold.local_addr().unwrap().port();
    let output = Command::new(env!("CARGO_BIN_EXE_pixelforge"))
        .args([
            "play",
            "--project",
            path.to_str().unwrap(),
            "--no-open",
            "--port",
        ])
        .arg(port.to_string())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let text = String::from_utf8(output.stdout).unwrap();
    let value: Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
    assert_eq!(value["error"]["code"], "AddressInUse");
    assert!(value["error"]["message"]
        .as_str()
        .unwrap()
        .contains(&port.to_string()));
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("pixelforge: AddressInUse"));

    let garbage = scratch.child("bad.json");
    fs::write(&garbage, b"{").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_pixelforge"))
        .args(["play", "--project", garbage.to_str().unwrap(), "--no-open"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let value: Value = serde_json::from_str(
        String::from_utf8(output.stdout)
            .unwrap()
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(value["ok"], false);
    assert_eq!(value["error"]["code"], "ProjectFile");

    let missing = scratch.child("missing.json");
    let output = Command::new(env!("CARGO_BIN_EXE_pixelforge"))
        .args(["play", "--project", missing.to_str().unwrap(), "--no-open"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let value: Value = serde_json::from_str(
        String::from_utf8(output.stdout)
            .unwrap()
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(value["error"]["code"], "Cli");

    let output = Command::new(env!("CARGO_BIN_EXE_pixelforge"))
        .args([
            "play",
            "--project",
            path.to_str().unwrap(),
            "--checkpoint",
            scratch.child("side.json").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr)
        .to_ascii_lowercase()
        .contains("checkpoint"));
    assert_eq!(fs::read(&path).unwrap().len() > 0, true);
}
