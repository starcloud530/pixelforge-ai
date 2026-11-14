//! Cross-process exclusion and atomic replacement for one project path.
//!
//! The lock is a sidecar next to the canonical project file
//! (`<name>.pixelforge.lock`). [`fs4`] takes an exclusive `flock(2)` on Unix
//! and `LockFileEx` on Windows. Both are released by the OS when the last
//! handle closes, including when the process is killed. The sidecar is not
//! removed: unlinking it lets a waiter keep the old inode while a new locker
//! creates another file at the same path.
//!
//! The project file itself is not the lock. Atomic replace points the name at
//! a new inode, so a lock on the data inode would not cover the next writer.
//!
//! This is phase 1 of #132. It does not store a revision or a content
//! fingerprint, and it does not merge concurrent edits.

use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use fs4::FileExt;

use crate::failure::CliFailure;

/// Wait bound when `PIXELFORGE_LOCK_TIMEOUT_MS` is unset.
pub const DEFAULT_LOCK_TIMEOUT_MS: u64 = 30_000;

/// Exclusive lock for one canonical project path.
///
/// Drop unlocks and closes the sidecar. It does not delete the sidecar.
#[derive(Debug)]
pub struct ProjectLock {
    file: File,
    target: PathBuf,
    user_path: PathBuf,
}

impl ProjectLock {
    /// Locks `user_path` before the caller reads it. The wait is bounded and
    /// is not a queue.
    pub fn acquire(user_path: &Path) -> Result<Self, CliFailure> {
        Self::acquire_for(user_path, lock_timeout()?)
    }

    pub fn acquire_for(user_path: &Path, timeout: Duration) -> Result<Self, CliFailure> {
        let target = canonical_target(user_path)?;
        let lock_path = lock_sidecar(&target)?;
        reject_symlink(&lock_path)?;
        let file = open_lock_file(&lock_path)?;
        // Unix open uses `O_NOFOLLOW`. Windows checks the path again because
        // the open itself may follow a reparse point.
        #[cfg(windows)]
        reject_symlink(&lock_path)?;
        wait_exclusive(&file, timeout, user_path)?;
        // The count is taken while this process holds the sidecar, so a
        // cooperating writer cannot add a second name underneath the check.
        ensure_supported_file(&target, user_path)?;
        test_barrier("PIXELFORGE_TEST_LOCK_BARRIER", "held");
        Ok(Self {
            file,
            target,
            user_path: user_path.to_path_buf(),
        })
    }

    /// Canonical file this lock covers. Writes go here, not through a symlink
    /// the caller happened to pass.
    pub fn target(&self) -> &Path {
        &self.target
    }

    pub fn user_path(&self) -> &Path {
        &self.user_path
    }
}

impl Drop for ProjectLock {
    fn drop(&mut self) {
        // Closing the handle also releases the OS lock. Unlock first so a
        // later open in this process does not observe a leftover exclusive
        // range on Windows.
        // Call the trait method by name. Rust 1.89 adds `File::unlock`, and
        // that inherent method would hide `fs4`'s. CI stable is past 1.89;
        // the repo floor is 1.83, which does not have the inherent method.
        let _ = fs4::FileExt::unlock(&self.file);
    }
}

/// Replace `path` with `bytes` by writing a unique temp file in the same
/// directory, syncing it, and renaming it over the destination.
///
/// A crash before the rename leaves the original path untouched. A leftover
/// `.<name>.<pid>.<nanos>.tmp` file is not the project. Success means the
/// rename has published one complete file. On Unix the parent directory is
/// then synced when the filesystem allows it. That is not a power-loss
/// guarantee on network drives, and Windows has no equivalent directory sync
/// here. See `docs/known-limits.md`.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), CliFailure> {
    if path.file_name().is_none() {
        return Err(CliFailure::new(
            "Cli",
            format!("path has no file name: {}", path.display()),
        ));
    }
    if test_fail("write") {
        return Err(CliFailure::new(
            "Cli",
            format!(
                "refusing to write {} (test hook); the original file was left in place",
                path.display()
            ),
        ));
    }
    let parent = parent_dir(path);
    if parent.as_os_str() != "." && !parent.as_os_str().is_empty() {
        fs::create_dir_all(&parent).map_err(|err| CliFailure::io(&parent, err))?;
    }
    let tmp = unique_temp(path)?;
    let published = (|| -> Result<bool, CliFailure> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
            .map_err(|err| CliFailure::io(&tmp, err))?;
        file.write_all(bytes)
            .map_err(|err| CliFailure::io(&tmp, err))?;
        #[cfg(unix)]
        copy_mode(path, &tmp)?;
        file.sync_all().map_err(|err| CliFailure::io(&tmp, err))?;
        if test_fail("replace") {
            return Err(CliFailure::new(
                "Cli",
                format!(
                    "refusing to replace {} (test hook); the original file was left in place",
                    path.display()
                ),
            ));
        }
        fs::rename(&tmp, path).map_err(|err| {
            CliFailure::new(
                "Cli",
                format!(
                    "cannot replace {} ({err}); the original file was left in place",
                    path.display()
                ),
            )
        })?;
        // The name now points at the new file. A directory-sync failure must
        // not delete that file.
        sync_parent(&parent)?;
        Ok(true)
    })();
    if published.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    published.map(|_| ())
}

/// Debug builds stall inside the lock so tests can release one holder at a
/// time. Release builds ignore the variable.
pub(crate) fn test_edit_barrier() {
    test_barrier("PIXELFORGE_TEST_EDIT_BARRIER", "paused");
}

pub(crate) fn test_fail(kind: &str) -> bool {
    #[cfg(debug_assertions)]
    {
        matches!(
            std::env::var("PIXELFORGE_TEST_FAIL_SAVE").as_deref(),
            Ok(value) if value == kind
        )
    }
    #[cfg(not(debug_assertions))]
    {
        let _ = kind;
        false
    }
}

fn lock_timeout() -> Result<Duration, CliFailure> {
    match std::env::var("PIXELFORGE_LOCK_TIMEOUT_MS") {
        Err(std::env::VarError::NotPresent) => Ok(Duration::from_millis(DEFAULT_LOCK_TIMEOUT_MS)),
        Err(err) => Err(CliFailure::new(
            "Cli",
            format!("cannot read PIXELFORGE_LOCK_TIMEOUT_MS: {err}"),
        )),
        Ok(text) => {
            let ms: u64 = text.trim().parse().map_err(|_| {
                CliFailure::new(
                    "Cli",
                    "PIXELFORGE_LOCK_TIMEOUT_MS must be an integer number of milliseconds",
                )
            })?;
            Ok(Duration::from_millis(ms))
        }
    }
}

fn wait_exclusive(file: &File, timeout: Duration, user: &Path) -> Result<(), CliFailure> {
    let start = Instant::now();
    loop {
        // Same reason as `unlock`: do not call inherent `File::try_lock`.
        // That method returns `std::fs::TryLockError` on Rust 1.89+.
        match fs4::FileExt::try_lock(file) {
            Ok(()) => return Ok(()),
            Err(fs4::TryLockError::WouldBlock) => {
                if start.elapsed() >= timeout {
                    return Err(CliFailure::new(
                        "LockTimeout",
                        format!(
                            "timed out after {}ms waiting for the exclusive lock on {}; this process did not read or change it",
                            timeout.as_millis(),
                            user.display()
                        ),
                    )
                    .with_hint(
                        "another pixelforge process may be editing this project. The wait is not a queue and is not first-in-first-out. PIXELFORGE_LOCK_TIMEOUT_MS sets the bound in milliseconds. This process does not keep running after the error",
                    ));
                }
                let rest = timeout.saturating_sub(start.elapsed());
                thread::sleep(Duration::from_millis(20).min(rest));
            }
            Err(fs4::TryLockError::Error(err)) => return Err(CliFailure::io(user, err)),
        }
    }
}

fn canonical_target(user: &Path) -> Result<PathBuf, CliFailure> {
    if user.as_os_str().is_empty() {
        return Err(CliFailure::new("Cli", "project path is empty"));
    }
    match fs::symlink_metadata(user) {
        Ok(meta) if meta.file_type().is_symlink() => {
            if !user.exists() {
                return Err(unsupported(format!(
                    "project path {} is a symbolic link with no target; pass the path to create",
                    user.display()
                )));
            }
            fs::canonicalize(user).map_err(|err| CliFailure::io(user, err))
        }
        Ok(_) => fs::canonicalize(user).map_err(|err| CliFailure::io(user, err)),
        Err(err) if err.kind() == ErrorKind::NotFound => {
            let name = user.file_name().ok_or_else(|| {
                CliFailure::new("Cli", format!("path has no file name: {}", user.display()))
            })?;
            let parent = match user.parent() {
                Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
                _ => PathBuf::from("."),
            };
            if !parent.exists() {
                fs::create_dir_all(&parent).map_err(|err| CliFailure::io(&parent, err))?;
            }
            let parent = fs::canonicalize(&parent).map_err(|err| CliFailure::io(&parent, err))?;
            if !parent.is_dir() {
                return Err(CliFailure::new(
                    "Cli",
                    format!("parent is not a directory: {}", parent.display()),
                ));
            }
            Ok(parent.join(name))
        }
        Err(err) => Err(CliFailure::io(user, err)),
    }
}

fn lock_sidecar(target: &Path) -> Result<PathBuf, CliFailure> {
    let name = target.file_name().ok_or_else(|| {
        CliFailure::new(
            "Cli",
            format!("path has no file name: {}", target.display()),
        )
    })?;
    let mut lock_name = name.to_os_string();
    lock_name.push(".pixelforge.lock");
    Ok(parent_dir(target).join(lock_name))
}

fn reject_symlink(path: &Path) -> Result<(), CliFailure> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => Err(unsupported(format!(
            "lock file {} is a symbolic link; refusing to follow it",
            path.display()
        ))),
        Ok(_) => Ok(()),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
        Err(err) => Err(CliFailure::io(path, err)),
    }
}

fn open_lock_file(path: &Path) -> Result<File, CliFailure> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // `std` already sets close-on-exec. `O_NOFOLLOW` refuses a symlink
        // at the final component, including one that appears at create time.
        options.mode(0o644);
        options.custom_flags(libc::O_NOFOLLOW);
    }
    options.open(path).map_err(|err| {
        #[cfg(unix)]
        if err.raw_os_error() == Some(libc::ELOOP) {
            return unsupported(format!(
                "lock file {} is a symbolic link; refusing to follow it",
                path.display()
            ));
        }
        CliFailure::io(path, err)
    })
}

fn ensure_supported_file(target: &Path, user: &Path) -> Result<(), CliFailure> {
    if !target.exists() {
        return Ok(());
    }
    let meta = fs::metadata(target).map_err(|err| CliFailure::io(target, err))?;
    if !meta.is_file() {
        return Err(CliFailure::new(
            "Cli",
            format!("project path is not a file: {}", user.display()),
        ));
    }
    let links = link_count(target)?;
    if links > 1 {
        return Err(unsupported(format!(
            "project file {} has {links} names; hard links are not a supported alias, so nothing was read or written",
            user.display()
        )));
    }
    Ok(())
}

#[cfg(unix)]
fn link_count(path: &Path) -> Result<u64, CliFailure> {
    use std::os::unix::fs::MetadataExt;
    let meta = fs::metadata(path).map_err(|err| CliFailure::io(path, err))?;
    Ok(meta.nlink())
}

#[cfg(windows)]
fn link_count(path: &Path) -> Result<u64, CliFailure> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
    };

    let file = File::open(path).map_err(|err| CliFailure::io(path, err))?;
    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
    let ok = unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) };
    if ok == 0 {
        return Err(CliFailure::io(path, std::io::Error::last_os_error()));
    }
    Ok(u64::from(info.nNumberOfLinks))
}

#[cfg(unix)]
fn copy_mode(from: &Path, to: &Path) -> Result<(), CliFailure> {
    match fs::metadata(from) {
        Ok(meta) => {
            fs::set_permissions(to, meta.permissions()).map_err(|err| CliFailure::io(to, err))
        }
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
        Err(err) => Err(CliFailure::io(from, err)),
    }
}

fn sync_parent(dir: &Path) -> Result<(), CliFailure> {
    #[cfg(unix)]
    {
        let file = File::open(dir).map_err(|err| CliFailure::io(dir, err))?;
        match file.sync_all() {
            Ok(()) => Ok(()),
            // Overlay and some network filesystems reject directory sync.
            // The rename has already published the bytes. Do not claim the
            // directory entry is power-loss durable in that case, and do not
            // fail a save the reader can already see.
            Err(err) if directory_sync_unsupported(&err) => Ok(()),
            Err(err) => Err(CliFailure::new(
                "DirectorySync",
                format!(
                    "replaced the file but could not sync directory {} ({err}); the new contents are already in place",
                    dir.display()
                ),
            )),
        }
    }
    #[cfg(not(unix))]
    {
        let _ = dir;
        Ok(())
    }
}

#[cfg(unix)]
fn directory_sync_unsupported(err: &std::io::Error) -> bool {
    matches!(
        err.raw_os_error(),
        Some(libc::EINVAL) | Some(libc::ENOTSUP) | Some(libc::ENOSYS)
    )
}

fn unique_temp(path: &Path) -> Result<PathBuf, CliFailure> {
    let name = path.file_name().ok_or_else(|| {
        CliFailure::new("Cli", format!("path has no file name: {}", path.display()))
    })?;
    let parent = parent_dir(path);
    for salt in 0..8 {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let mut tmp_name = std::ffi::OsString::from(".");
        tmp_name.push(name);
        tmp_name.push(format!(".{}.{nanos}.{salt}.tmp", std::process::id()));
        let candidate = parent.join(&tmp_name);
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(CliFailure::new(
        "Cli",
        format!("cannot choose a temp file next to {}", path.display()),
    ))
}

fn parent_dir(path: &Path) -> PathBuf {
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => PathBuf::from("."),
    }
}

fn unsupported(message: String) -> CliFailure {
    CliFailure::new("UnsupportedPath", message)
}

fn test_barrier(var: &str, prefix: &str) {
    #[cfg(debug_assertions)]
    {
        let Ok(dir) = std::env::var(var) else {
            return;
        };
        if dir.is_empty() {
            return;
        }
        let dir = PathBuf::from(dir);
        let pid = std::process::id();
        let flag = dir.join(format!("{prefix}-{pid}"));
        if fs::write(&flag, b"1").is_err() {
            return;
        }
        let release = dir.join(format!("release-{pid}"));
        let all = dir.join("release-all");
        let deadline = Instant::now() + Duration::from_secs(60);
        while Instant::now() < deadline {
            if release.exists() || all.exists() {
                let _ = fs::remove_file(&flag);
                return;
            }
            thread::sleep(Duration::from_millis(10));
        }
        let _ = fs::remove_file(&flag);
    }
    #[cfg(not(debug_assertions))]
    {
        let _ = (var, prefix);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "pixelforge-store-{}-{}-{name}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn atomic_write_replaces_a_complete_file_and_removes_the_temp() {
        let dir = scratch("atomic");
        let path = dir.join("hero.json");
        fs::write(&path, b"{\"old\":true}").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&path).unwrap().permissions();
            perms.set_mode(0o640);
            fs::set_permissions(&path, perms).unwrap();
        }
        write_atomic(&path, b"{\"new\":true}").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"{\"new\":true}");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o640
            );
        }
        let leftovers: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .filter(|name| name.to_string_lossy().contains(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_second_open_cannot_take_the_lock_until_the_holder_drops_it() {
        let dir = scratch("lock");
        let path = dir.join("hero.json");
        fs::write(&path, b"v0").unwrap();
        let held = ProjectLock::acquire_for(&path, Duration::ZERO).unwrap();
        let blocked = ProjectLock::acquire_for(&path, Duration::ZERO).unwrap_err();
        assert_eq!(blocked.code(), "LockTimeout");
        assert!(blocked.message().contains("did not read or change"));
        drop(held);
        let again = ProjectLock::acquire_for(&path, Duration::ZERO).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"v0");
        drop(again);
        assert!(dir.join("hero.json.pixelforge.lock").is_file());
        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_and_its_target_share_one_lock_and_a_hard_link_is_refused() {
        let dir = scratch("alias");
        let path = dir.join("hero.json");
        fs::write(&path, b"v0").unwrap();
        let link = dir.join("alias.json");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        let held = ProjectLock::acquire_for(&path, Duration::ZERO).unwrap();
        let via_link = ProjectLock::acquire_for(&link, Duration::ZERO).unwrap_err();
        assert_eq!(via_link.code(), "LockTimeout");
        drop(held);

        let hard = dir.join("hard.json");
        fs::hard_link(&path, &hard).unwrap();
        let refused = ProjectLock::acquire_for(&path, Duration::ZERO).unwrap_err();
        assert_eq!(refused.code(), "UnsupportedPath");
        assert_eq!(fs::read(&path).unwrap(), b"v0");
        fs::remove_file(&hard).unwrap();
        assert!(ProjectLock::acquire_for(&path, Duration::ZERO).is_ok());

        let dangling = dir.join("dangling.json");
        std::os::unix::fs::symlink(dir.join("missing.json"), &dangling).unwrap();
        let dangling_err = ProjectLock::acquire_for(&dangling, Duration::ZERO).unwrap_err();
        assert_eq!(dangling_err.code(), "UnsupportedPath");

        let lock_link = dir.join("other.json.pixelforge.lock");
        std::os::unix::fs::symlink(&path, &lock_link).unwrap();
        let other = dir.join("other.json");
        fs::write(&other, b"v1").unwrap();
        let followed = ProjectLock::acquire_for(&other, Duration::ZERO).unwrap_err();
        assert_eq!(followed.code(), "UnsupportedPath");
        let _ = fs::remove_dir_all(&dir);
    }
}
