//! Cross-process project edits. Barriers are files the child creates while it
//! holds the lock, so the assertions do not depend on a sleep interval.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use pixelforge_kernel::Document;
use serde_json::Value;

fn pixelforge(args: &[&str]) -> Output {
    command(args, &[]).output().expect("spawn pixelforge")
}

fn command(args: &[&str], env: &[(&str, &str)]) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_pixelforge"));
    cmd.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());
    for (key, value) in env {
        cmd.env(key, value);
    }
    cmd
}

fn json_lines(output: &Output) -> Vec<Value> {
    let text = String::from_utf8(output.stdout.clone()).expect("stdout utf-8");
    text.lines()
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_str(line).unwrap_or_else(|err| panic!("{err} in {line}")))
        .collect()
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr utf-8")
}

fn status(output: &Output) -> i32 {
    output.status.code().expect("exited")
}

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
            "pixelforge-concurrent-{nanos}-{n}-{}",
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
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&self.path, fs::Permissions::from_mode(0o755));
        }
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct KillOnDrop(Option<Child>);

impl KillOnDrop {
    fn spawn(cmd: &mut Command) -> Self {
        Self(Some(cmd.spawn().expect("spawn")))
    }

    fn id(&self) -> u32 {
        self.0.as_ref().unwrap().id()
    }

    fn wait(mut self) -> Output {
        self.0.take().unwrap().wait_with_output().expect("wait")
    }
}

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        if let Some(child) = self.0.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn load(path: &Path) -> Document {
    Document::load_project(&fs::read_to_string(path).unwrap()).expect("project")
}

fn new_project(path: &Path) {
    let output = pixelforge(&[
        "new",
        "--width",
        "16",
        "--height",
        "16",
        "--output",
        path.to_str().unwrap(),
    ]);
    assert_eq!(status(&output), 0, "{}", stderr(&output));
}

fn paint(path: &Path, x: u16, env: &[(&str, &str)]) -> Output {
    command(
        &[
            "paint",
            "--project",
            path.to_str().unwrap(),
            "--x",
            &x.to_string(),
            "--y",
            "0",
            "--color-index",
            "1",
        ],
        env,
    )
    .output()
    .expect("paint")
}

fn wait_for(path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        if path.is_file() {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("timed out waiting for {}", path.display());
}

fn names_with_prefix(dir: &Path, prefix: &str) -> Vec<String> {
    fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with(prefix))
        .collect()
}

fn assert_lock_timeout(output: &Output) {
    assert_eq!(status(output), 2, "{}", stderr(output));
    let lines = json_lines(output);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert_eq!(lines[0]["ok"], false);
    assert_eq!(
        lines[0]["error"]["code"],
        "LockTimeout",
        "unexpected refusal: {}; stderr: {}",
        output_text(output),
        stderr(output)
    );
    assert!(lines[0]["error"]["hint"].is_string());
    assert!(
        !output_text(output).contains("\"ok\":true"),
        "timeout must not report a successful edit: {}",
        output_text(output)
    );
}

fn output_text(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

fn temps(dir: &Path) -> Vec<String> {
    fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.contains(".tmp"))
        .collect()
}

#[test]
fn simultaneous_distinct_pixels_are_all_kept_across_repeated_trials() {
    let scratch = Scratch::new();
    for trial in 0..5 {
        let project = scratch.child(&format!("trial-{trial}.json"));
        new_project(&project);
        let mut children = Vec::new();
        for x in 0..8u16 {
            children.push(KillOnDrop::spawn(&mut command(
                &[
                    "paint",
                    "--project",
                    project.to_str().unwrap(),
                    "--x",
                    &x.to_string(),
                    "--y",
                    "0",
                    "--color-index",
                    "1",
                ],
                &[],
            )));
        }
        let mut outputs = Vec::new();
        for child in children {
            outputs.push(child.wait());
        }
        let doc = load(&project);
        for (x, output) in outputs.iter().enumerate() {
            if status(output) == 0 {
                assert_eq!(json_lines(output)[0]["ok"], true, "{}", stderr(output));
                assert_eq!(doc.get(x as u16, 0).unwrap(), 1, "trial {trial} pixel {x}");
            } else {
                assert_lock_timeout(output);
                assert_eq!(doc.get(x as u16, 0).unwrap(), 0, "trial {trial} pixel {x}");
            }
        }
        assert!(
            outputs.iter().all(|output| status(output) == 0),
            "trial {trial} should finish every edit, codes {:?}",
            outputs.iter().map(status).collect::<Vec<_>>()
        );
        serde_json::from_str::<Value>(&fs::read_to_string(&project).unwrap()).unwrap();
        assert!(scratch
            .path
            .join(format!("trial-{trial}.json.pixelforge.lock"))
            .is_file());
    }
}

#[test]
fn overlapping_edits_stay_exclusive_and_every_pixel_lands() {
    let scratch = Scratch::new();
    let project = scratch.child("hero.json");
    let barrier = scratch.child("edit-barrier");
    fs::create_dir_all(&barrier).unwrap();
    new_project(&project);
    let mut children = Vec::new();
    for x in 0..8u16 {
        children.push((
            x,
            KillOnDrop::spawn(&mut command(
                &[
                    "paint",
                    "--project",
                    project.to_str().unwrap(),
                    "--x",
                    &x.to_string(),
                    "--y",
                    "0",
                    "--color-index",
                    "1",
                ],
                &[
                    ("PIXELFORGE_TEST_EDIT_BARRIER", barrier.to_str().unwrap()),
                    ("PIXELFORGE_LOCK_TIMEOUT_MS", "20000"),
                ],
            )),
        ));
    }
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut released = Vec::new();
    while released.len() < 8 {
        if Instant::now() > deadline {
            panic!("editors did not finish; released {released:?}");
        }
        let paused = names_with_prefix(&barrier, "paused-");
        assert!(
            paused.len() <= 1,
            "more than one editor held the project: {paused:?}"
        );
        if let Some(name) = paused.first() {
            let pid = name.trim_start_matches("paused-");
            if !released.iter().any(|seen| seen == pid) {
                fs::write(barrier.join(format!("release-{pid}")), b"1").unwrap();
                released.push(pid.to_string());
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    for (x, child) in children {
        let output = child.wait();
        assert_eq!(status(&output), 0, "pixel {x}: {}", stderr(&output));
    }
    let doc = load(&project);
    for x in 0..8u16 {
        assert_eq!(doc.get(x, 0).unwrap(), 1);
    }
}

#[test]
fn a_waiter_blocks_then_a_timeout_leaves_the_project_unchanged() {
    let scratch = Scratch::new();
    let project = scratch.child("hero.json");
    let barrier = scratch.child("lock-barrier");
    fs::create_dir_all(&barrier).unwrap();
    new_project(&project);
    let before = fs::read(&project).unwrap();
    let holder = KillOnDrop::spawn(&mut command(
        &[
            "paint",
            "--project",
            project.to_str().unwrap(),
            "--x",
            "0",
            "--y",
            "0",
            "--color-index",
            "1",
        ],
        &[("PIXELFORGE_TEST_LOCK_BARRIER", barrier.to_str().unwrap())],
    ));
    let held = barrier.join(format!("held-{}", holder.id()));
    wait_for(&held);
    let timed_out = paint(&project, 1, &[("PIXELFORGE_LOCK_TIMEOUT_MS", "0")]);
    assert_lock_timeout(&timed_out);
    assert_eq!(fs::read(&project).unwrap(), before);
    assert_eq!(load(&project).get(0, 0).unwrap(), 0);
    assert_eq!(load(&project).get(1, 0).unwrap(), 0);

    let inspect = paint_free_inspect(&project);
    assert_eq!(status(&inspect), 0, "{}", stderr(&inspect));
    assert_eq!(json_lines(&inspect)[0]["ok"], true);
    assert_eq!(fs::read(&project).unwrap(), before);

    fs::write(barrier.join(format!("release-{}", holder.id())), b"1").unwrap();
    let output = holder.wait();
    assert_eq!(status(&output), 0, "{}", stderr(&output));
    assert_eq!(load(&project).get(0, 0).unwrap(), 1);
    assert_eq!(load(&project).get(1, 0).unwrap(), 0);
    assert!(scratch.child("hero.json.pixelforge.lock").is_file());
}

fn paint_free_inspect(project: &Path) -> Output {
    command(
        &["inspect", "--project", project.to_str().unwrap()],
        &[("PIXELFORGE_LOCK_TIMEOUT_MS", "0")],
    )
    .output()
    .expect("inspect")
}

#[test]
fn killing_the_holder_releases_the_lock_for_the_next_edit() {
    let scratch = Scratch::new();
    let project = scratch.child("hero.json");
    let barrier = scratch.child("lock-barrier");
    fs::create_dir_all(&barrier).unwrap();
    new_project(&project);
    let mut holder = command(
        &[
            "paint",
            "--project",
            project.to_str().unwrap(),
            "--x",
            "0",
            "--y",
            "0",
            "--color-index",
            "1",
        ],
        &[("PIXELFORGE_TEST_LOCK_BARRIER", barrier.to_str().unwrap())],
    )
    .spawn()
    .unwrap();
    let held = barrier.join(format!("held-{}", holder.id()));
    wait_for(&held);
    holder.kill().unwrap();
    let finished = holder.wait().unwrap();
    assert!(!finished.success());
    let next = paint(&project, 2, &[]);
    assert_eq!(status(&next), 0, "{}", stderr(&next));
    assert_eq!(load(&project).get(2, 0).unwrap(), 1);
    assert_eq!(load(&project).get(0, 0).unwrap(), 0);
    assert!(scratch.child("hero.json.pixelforge.lock").is_file());
}

#[test]
fn independent_projects_hold_their_locks_at_the_same_time() {
    let scratch = Scratch::new();
    let barrier = scratch.child("locks");
    fs::create_dir_all(&barrier).unwrap();
    let mut children = Vec::new();
    for index in 0..8 {
        let project = scratch.child(&format!("p{index}.json"));
        new_project(&project);
        children.push(KillOnDrop::spawn(&mut command(
            &[
                "paint",
                "--project",
                project.to_str().unwrap(),
                "--x",
                "1",
                "--y",
                "1",
                "--color-index",
                "3",
            ],
            &[("PIXELFORGE_TEST_LOCK_BARRIER", barrier.to_str().unwrap())],
        )));
    }
    let deadline = Instant::now() + Duration::from_secs(20);
    while names_with_prefix(&barrier, "held-").len() < 8 {
        if Instant::now() > deadline {
            panic!(
                "independent projects did not all hold locks: {:?}",
                names_with_prefix(&barrier, "held-")
            );
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    fs::write(barrier.join("release-all"), b"1").unwrap();
    for child in children {
        let output = child.wait();
        assert_eq!(status(&output), 0, "{}", stderr(&output));
    }
    for index in 0..8 {
        let project = scratch.child(&format!("p{index}.json"));
        assert_eq!(load(&project).get(1, 1).unwrap(), 3);
    }
}

#[test]
fn a_symlink_uses_the_same_lock_and_a_hard_link_is_refused() {
    let scratch = Scratch::new();
    let project = scratch.child("hero.json");
    let barrier = scratch.child("barrier");
    fs::create_dir_all(&barrier).unwrap();
    new_project(&project);
    let alias = scratch.child("alias.json");
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&project, &alias).unwrap();
        let holder = KillOnDrop::spawn(&mut command(
            &[
                "paint",
                "--project",
                project.to_str().unwrap(),
                "--x",
                "0",
                "--y",
                "0",
                "--color-index",
                "1",
            ],
            &[("PIXELFORGE_TEST_LOCK_BARRIER", barrier.to_str().unwrap())],
        ));
        wait_for(&barrier.join(format!("held-{}", holder.id())));
        let via_alias = paint(&alias, 1, &[("PIXELFORGE_LOCK_TIMEOUT_MS", "0")]);
        assert_lock_timeout(&via_alias);
        assert_eq!(load(&project).get(1, 0).unwrap(), 0);
        fs::write(barrier.join(format!("release-{}", holder.id())), b"1").unwrap();
        assert_eq!(status(&holder.wait()), 0);
        let painted = paint(&alias, 1, &[]);
        assert_eq!(status(&painted), 0, "{}", stderr(&painted));
        assert_eq!(load(&project).get(1, 0).unwrap(), 1);

        let hard = scratch.child("hard.json");
        fs::hard_link(&project, &hard).unwrap();
        let before = fs::read(&project).unwrap();
        let refused = paint(&hard, 3, &[]);
        assert_eq!(status(&refused), 2, "{}", stderr(&refused));
        assert_eq!(json_lines(&refused)[0]["error"]["code"], "UnsupportedPath");
        assert_eq!(fs::read(&project).unwrap(), before);
        fs::remove_file(&hard).unwrap();
        let restored = paint(&project, 3, &[]);
        assert_eq!(status(&restored), 0, "{}", stderr(&restored));
        assert_eq!(load(&project).get(3, 0).unwrap(), 1);
    }
}

#[test]
fn save_and_replace_failures_keep_the_original_bytes() {
    let scratch = Scratch::new();
    let project = scratch.child("hero.json");
    new_project(&project);
    let before = fs::read(&project).unwrap();

    let write_fail = paint(&project, 0, &[("PIXELFORGE_TEST_FAIL_SAVE", "write")]);
    assert_eq!(status(&write_fail), 2, "{}", stderr(&write_fail));
    assert_eq!(fs::read(&project).unwrap(), before);
    assert!(
        temps(&scratch.path).is_empty(),
        "{:?}",
        temps(&scratch.path)
    );

    let replace_fail = paint(&project, 0, &[("PIXELFORGE_TEST_FAIL_SAVE", "replace")]);
    assert_eq!(status(&replace_fail), 2, "{}", stderr(&replace_fail));
    assert_eq!(fs::read(&project).unwrap(), before);
    assert!(
        temps(&scratch.path).is_empty(),
        "{:?}",
        temps(&scratch.path)
    );
    serde_json::from_str::<Value>(&fs::read_to_string(&project).unwrap()).unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let readonly = fs::Permissions::from_mode(0o555);
        fs::set_permissions(&scratch.path, readonly).unwrap();
        let denied = paint(&project, 0, &[]);
        fs::set_permissions(&scratch.path, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(status(&denied), 2, "{}", stderr(&denied));
        assert_eq!(fs::read(&project).unwrap(), before);
        serde_json::from_str::<Value>(&fs::read_to_string(&project).unwrap()).unwrap();
    }
}

#[test]
fn a_checkpoint_failure_after_the_project_write_is_not_one_commit() {
    let scratch = Scratch::new();
    let project = scratch.child("hero.json");
    let checkpoint = scratch.child("hero.ckpt.json");
    new_project(&project);
    let first = command(
        &[
            "paint",
            "--project",
            project.to_str().unwrap(),
            "--checkpoint",
            checkpoint.to_str().unwrap(),
            "--x",
            "0",
            "--y",
            "0",
            "--color-index",
            "1",
        ],
        &[],
    )
    .output()
    .unwrap();
    assert_eq!(status(&first), 0, "{}", stderr(&first));
    let checkpoint_bytes = fs::read(&checkpoint).unwrap();
    let second = command(
        &[
            "paint",
            "--project",
            project.to_str().unwrap(),
            "--checkpoint",
            checkpoint.to_str().unwrap(),
            "--x",
            "1",
            "--y",
            "0",
            "--color-index",
            "2",
        ],
        &[("PIXELFORGE_TEST_FAIL_SAVE", "checkpoint")],
    )
    .output()
    .unwrap();
    assert_eq!(status(&second), 2, "{}", stderr(&second));
    let lines = json_lines(&second);
    assert!(
        lines
            .iter()
            .any(|line| line["error"]["code"] == "CheckpointFile"),
        "{lines:?}"
    );
    assert_eq!(load(&project).get(1, 0).unwrap(), 2);
    assert_eq!(fs::read(&checkpoint).unwrap(), checkpoint_bytes);
    serde_json::from_str::<Value>(&fs::read_to_string(&project).unwrap()).unwrap();
    let undo = command(
        &[
            "undo",
            "--project",
            project.to_str().unwrap(),
            "--checkpoint",
            checkpoint.to_str().unwrap(),
        ],
        &[],
    )
    .output()
    .unwrap();
    assert_eq!(status(&undo), 2, "{}", stderr(&undo));
    assert_eq!(json_lines(&undo)[0]["error"]["code"], "CheckpointMismatch");
    assert_eq!(load(&project).get(1, 0).unwrap(), 2);
}

#[test]
fn checkpoint_undo_waits_for_the_writer_and_then_restores() {
    let scratch = Scratch::new();
    let project = scratch.child("hero.json");
    let checkpoint = scratch.child("hero.ckpt.json");
    let barrier = scratch.child("barrier");
    fs::create_dir_all(&barrier).unwrap();
    new_project(&project);
    let before = fs::read(&project).unwrap();
    let holder = KillOnDrop::spawn(&mut command(
        &[
            "paint",
            "--project",
            project.to_str().unwrap(),
            "--checkpoint",
            checkpoint.to_str().unwrap(),
            "--x",
            "0",
            "--y",
            "0",
            "--color-index",
            "1",
        ],
        &[("PIXELFORGE_TEST_LOCK_BARRIER", barrier.to_str().unwrap())],
    ));
    wait_for(&barrier.join(format!("held-{}", holder.id())));
    let blocked = command(
        &[
            "paint",
            "--project",
            project.to_str().unwrap(),
            "--checkpoint",
            checkpoint.to_str().unwrap(),
            "--x",
            "1",
            "--y",
            "0",
            "--color-index",
            "2",
        ],
        &[("PIXELFORGE_LOCK_TIMEOUT_MS", "0")],
    )
    .output()
    .unwrap();
    assert_lock_timeout(&blocked);
    assert_eq!(fs::read(&project).unwrap(), before);
    assert!(!checkpoint.exists());
    fs::write(barrier.join(format!("release-{}", holder.id())), b"1").unwrap();
    assert_eq!(status(&holder.wait()), 0);
    assert_eq!(load(&project).get(0, 0).unwrap(), 1);
    let undo = command(
        &[
            "undo",
            "--project",
            project.to_str().unwrap(),
            "--checkpoint",
            checkpoint.to_str().unwrap(),
        ],
        &[],
    )
    .output()
    .unwrap();
    assert_eq!(status(&undo), 0, "{}", stderr(&undo));
    assert_eq!(load(&project).get(0, 0).unwrap(), 0);
}

#[test]
fn creating_the_same_project_twice_does_not_publish_a_partial_file() {
    let scratch = Scratch::new();
    let project = scratch.child("hero.json");
    let barrier = scratch.child("barrier");
    fs::create_dir_all(&barrier).unwrap();
    let holder = KillOnDrop::spawn(&mut command(
        &[
            "new",
            "--width",
            "8",
            "--height",
            "8",
            "--output",
            project.to_str().unwrap(),
        ],
        &[("PIXELFORGE_TEST_LOCK_BARRIER", barrier.to_str().unwrap())],
    ));
    wait_for(&barrier.join(format!("held-{}", holder.id())));
    assert!(!project.exists());
    let blocked = command(
        &[
            "new",
            "--width",
            "8",
            "--height",
            "8",
            "--output",
            project.to_str().unwrap(),
        ],
        &[("PIXELFORGE_LOCK_TIMEOUT_MS", "0")],
    )
    .output()
    .unwrap();
    assert_lock_timeout(&blocked);
    assert!(!project.exists());
    fs::write(barrier.join(format!("release-{}", holder.id())), b"1").unwrap();
    let created = holder.wait();
    assert_eq!(status(&created), 0, "{}", stderr(&created));
    assert!(project.is_file());
    let again = pixelforge(&[
        "new",
        "--width",
        "4",
        "--height",
        "4",
        "--output",
        project.to_str().unwrap(),
    ]);
    assert_eq!(status(&again), 2, "{}", stderr(&again));
    assert_eq!(json_lines(&again)[0]["error"]["code"], "Cli");
    assert_eq!(load(&project).width(), 8);
}
