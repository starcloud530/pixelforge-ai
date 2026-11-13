//! A packaged `pixelforge` binary must list and read bundled skills without
//! the git checkout or a private bot filesystem.

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const CANONICAL: &[&str] = &[
    "pixelforge",
    "pixelforge-critique",
    "pixelforge-draw",
    "pixelforge-layers",
    "pixelforge-sprite-4dir",
    "pixel-art-craft",
];

struct Isolated {
    root: PathBuf,
    bin: PathBuf,
    cwd: PathBuf,
}

impl Isolated {
    fn new() -> Self {
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "pixelforge-skills-standalone-{nanos}-{n}-{}",
            std::process::id()
        ));
        let bin_dir = root.join("bin");
        let cwd = root.join("cwd");
        fs::create_dir_all(&bin_dir).unwrap();
        fs::create_dir_all(&cwd).unwrap();
        let bin = bin_dir.join("pixelforge");
        fs::copy(env!("CARGO_BIN_EXE_pixelforge"), &bin).unwrap();
        // The empty cwd is not the repository, and HOME/XDG dirs here cannot
        // supply `.cursor/skills` or a bot skill tree.
        Self { root, bin, cwd }
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(&self.bin)
            .args(args)
            .current_dir(&self.cwd)
            .env_clear()
            .env("HOME", self.root.join("home"))
            .env("XDG_CONFIG_HOME", self.root.join("config"))
            .env("XDG_DATA_HOME", self.root.join("data"))
            .env("PATH", self.bin.parent().unwrap())
            .env("TERM", "dumb")
            .env("LC_ALL", "C")
            .output()
            .expect("spawn isolated pixelforge")
    }
}

impl Drop for Isolated {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout is UTF-8")
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr is UTF-8")
}

fn status(output: &Output) -> i32 {
    output.status.code().expect("process exited")
}

fn json_stdout(output: &Output) -> Value {
    let text = stdout(output);
    let line = text
        .lines()
        .find(|line| !line.is_empty())
        .unwrap_or_else(|| panic!("expected JSON on stdout, got {text:?}"));
    serde_json::from_str(line).unwrap_or_else(|err| panic!("{err} in {line}"))
}

fn repo_absent(root: &Path) {
    for path in [
        root.join(".cursor"),
        root.join("cwd").join(".cursor"),
        root.join("home").join(".cursor"),
        root.join("home").join("agent-data"),
    ] {
        assert!(
            !path.exists(),
            "isolated tree leaked skill sources at {}",
            path.display()
        );
    }
}

#[test]
fn standalone_binary_lists_canonical_and_tutorial_skills() {
    let cli = Isolated::new();
    repo_absent(&cli.root);

    let output = cli.run(&["skills", "list"]);
    assert_eq!(status(&output), 0, "{}", stderr(&output));
    let body = json_stdout(&output);
    assert_eq!(body["ok"], true);
    assert_eq!(body["apiVersion"], 1);
    let names: Vec<String> = body["skills"]
        .as_array()
        .expect("skills array")
        .iter()
        .map(|skill| skill["name"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(body["count"], names.len());
    let mut expected = names.clone();
    expected.sort();
    assert_eq!(names, expected, "skill names should be sorted");
    for name in CANONICAL {
        assert!(
            names.contains(&name.to_string()),
            "missing {name} in {names:?}"
        );
    }
    for skill in body["skills"].as_array().unwrap() {
        assert_eq!(skill["version"], env!("CARGO_PKG_VERSION"));
        assert!(
            skill["description"].as_str().unwrap().len() > 8,
            "empty description for {}",
            skill["name"]
        );
        assert_eq!(skill["metadata"]["cliHelp"], "pixelforge --help");
        assert_eq!(skill["metadata"]["requires"]["bins"][0], "pixelforge");
    }

    let listed = cli.run(&["skills", "list", "--json"]);
    assert_eq!(status(&listed), 0, "{}", stderr(&listed));
    assert_eq!(json_stdout(&listed)["count"], body["count"]);
    repo_absent(&cli.root);
}

#[test]
fn standalone_binary_reads_canonical_skill_and_tutorial_references() {
    let cli = Isolated::new();

    let raw = cli.run(&["skills", "read", "pixelforge"]);
    assert_eq!(status(&raw), 0, "{}", stderr(&raw));
    let markdown = stdout(&raw);
    assert!(
        markdown.starts_with("---"),
        "raw SKILL.md should start at frontmatter:\n{markdown}"
    );
    assert!(
        markdown.contains("index-color")
            || markdown.contains("索引色")
            || markdown.contains("pixelforge"),
        "canonical pixelforge skill body missing:\n{markdown}"
    );
    assert!(
        stderr(&raw).contains("pixelforge skills read pixelforge"),
        "SKILL.md tip should go to stderr: {}",
        stderr(&raw)
    );

    let wrapped = cli.run(&["skills", "read", "pixelforge", "--json"]);
    assert_eq!(status(&wrapped), 0, "{}", stderr(&wrapped));
    let body = json_stdout(&wrapped);
    assert_eq!(body["ok"], true);
    assert_eq!(body["apiVersion"], 1);
    assert_eq!(body["skill"], "pixelforge");
    assert_eq!(body["path"], "SKILL.md");
    assert_eq!(body["content"].as_str().unwrap(), markdown);
    assert!(body["guidance"]
        .as_str()
        .unwrap()
        .contains("pixelforge skills read"));

    let listing = cli.run(&["skills", "list", "pixel-art-craft"]);
    assert_eq!(status(&listing), 0, "{}", stderr(&listing));
    let listed = json_stdout(&listing);
    assert_eq!(listed["path"], "pixel-art-craft");
    let entries = listed["entries"].as_array().unwrap();
    let paths: Vec<&str> = entries
        .iter()
        .map(|e| e["path"].as_str().unwrap())
        .collect();
    assert!(paths.contains(&"SKILL.md"), "{paths:?}");
    assert!(paths.contains(&"references"), "{paths:?}");
    let references = entries.iter().find(|e| e["path"] == "references").unwrap();
    assert_eq!(references["is_dir"], true);

    let refs = cli.run(&["skills", "list", "pixel-art-craft/references"]);
    assert_eq!(status(&refs), 0, "{}", stderr(&refs));
    let ref_paths: Vec<String> = json_stdout(&refs)["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["path"].as_str().unwrap().to_string())
        .collect();
    for name in ["beginner.md", "intermediate.md", "advanced.md", "master.md"] {
        assert!(
            ref_paths.contains(&format!("references/{name}")),
            "missing {name} in {ref_paths:?}"
        );
    }
    assert!(!ref_paths.iter().any(|path| path.ends_with("REPORT.md")));

    let beginner = cli.run(&[
        "skills",
        "read",
        "pixel-art-craft",
        "references/beginner.md",
    ]);
    assert_eq!(status(&beginner), 0, "{}", stderr(&beginner));
    let beginner_md = stdout(&beginner);
    assert!(!beginner_md.is_empty());
    assert!(
        beginner_md.contains("beginner")
            || beginner_md.contains("Beginner")
            || beginner_md.contains("入门")
            || beginner_md.contains("#"),
        "tutorial reference looked empty:\n{beginner_md}"
    );

    let alt = cli.run(&["skills", "read", "pixel-art-craft/references/beginner.md"]);
    assert_eq!(status(&alt), 0, "{}", stderr(&alt));
    assert_eq!(stdout(&alt), beginner_md);

    let as_json = cli.run(&[
        "skills",
        "read",
        "pixel-art-craft/references/beginner.md",
        "--json",
    ]);
    assert_eq!(status(&as_json), 0, "{}", stderr(&as_json));
    let body = json_stdout(&as_json);
    assert_eq!(body["skill"], "pixel-art-craft");
    assert_eq!(body["path"], "references/beginner.md");
    assert_eq!(body["content"].as_str().unwrap(), beginner_md);
    assert!(body.get("guidance").is_none());

    repo_absent(&cli.root);
}

#[test]
fn standalone_binary_reports_actionable_skill_errors() {
    let cli = Isolated::new();

    let unknown = cli.run(&["skills", "list", "not-a-skill"]);
    assert_eq!(status(&unknown), 2, "{}", stderr(&unknown));
    let body = json_stdout(&unknown);
    assert_eq!(body["ok"], false);
    assert_eq!(body["apiVersion"], 1);
    assert_eq!(body["error"]["code"], "not_found");
    assert!(body["error"]["message"]
        .as_str()
        .unwrap()
        .contains("not-a-skill"));
    assert!(body["error"]["hint"]
        .as_str()
        .unwrap()
        .contains("pixelforge skills list"));
    assert!(stderr(&unknown).contains("pixelforge: not_found"));

    let parent = cli.run(&["skills", "read", "pixelforge/../secret"]);
    assert_eq!(status(&parent), 2, "{}", stderr(&parent));
    assert_eq!(json_stdout(&parent)["error"]["code"], "invalid_argument");

    let absolute = cli.run(&["skills", "read", "pixelforge", "/etc/passwd"]);
    assert_eq!(status(&absolute), 2, "{}", stderr(&absolute));
    assert_eq!(json_stdout(&absolute)["error"]["code"], "invalid_argument");

    let file_as_dir = cli.run(&["skills", "list", "pixelforge/SKILL.md"]);
    assert_eq!(status(&file_as_dir), 2, "{}", stderr(&file_as_dir));
    assert_eq!(
        json_stdout(&file_as_dir)["error"]["code"],
        "invalid_argument"
    );

    let dir_as_file = cli.run(&["skills", "read", "pixel-art-craft/references"]);
    assert_eq!(status(&dir_as_file), 2, "{}", stderr(&dir_as_file));
    assert_eq!(
        json_stdout(&dir_as_file)["error"]["code"],
        "invalid_argument"
    );

    let missing = cli.run(&["skills", "read", "pixel-art-craft", "references/missing.md"]);
    assert_eq!(status(&missing), 2, "{}", stderr(&missing));
    assert_eq!(json_stdout(&missing)["error"]["code"], "not_found");

    let two_paths = cli.run(&[
        "skills",
        "read",
        "pixel-art-craft/SKILL.md",
        "references/beginner.md",
    ]);
    assert_eq!(status(&two_paths), 2, "{}", stderr(&two_paths));
    assert_eq!(json_stdout(&two_paths)["error"]["code"], "invalid_argument");

    repo_absent(&cli.root);
}
