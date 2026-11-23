//! The Firefox settings, versioned. The files that make up "my Firefox set-up" (the `user.js` the Fixes tab
//! writes, the container list, `userChrome.css` / `userContent.css`) are copied into a small git repository,
//! `~/.config/steamdecklauncher/firefox-settings`, and committed every time one of them changes: through the
//! Fixes buttons at once, and through a check every few seconds for changes made any other way. Nothing needs
//! setting up: the repository is created on the first save. Push it by typing a remote once (a private repo of
//! yours); on a new Deck, Restore brings the settings back. Without `git` everything else still works, only the
//! history is missing, and the screen says so.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// What is saved, relative to the profile folder. No cookies, no logins, no history: only settings you wrote.
pub const FILES: [&str; 4] = ["user.js", "containers.json", "chrome/userChrome.css", "chrome/userContent.css"];

pub fn store() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| dirs::home_dir().map(|h| h.join(".config")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("steamdecklauncher/firefox-settings")
}

/// `git` in the store. The author is fixed, so a commit needs no git configuration and carries no personal
/// data; a push never waits for a password prompt that nobody could answer.
fn git(args: &[&str]) -> Result<String, String> {
    let dir = store();
    fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let o = Command::new("git")
        .arg("-C")
        .arg(&dir)
        .args(["-c", "user.name=steamdecklauncher", "-c", "user.email=steamdecklauncher@localhost"])
        .args(["-c", "commit.gpgsign=false", "-c", "core.autocrlf=false"])
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_SSH_COMMAND", "ssh -o BatchMode=yes -o ConnectTimeout=15")
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                "git is not installed, so there is no history (the settings themselves are fine)".to_string()
            } else {
                format!("git: {e}")
            }
        })?;
    let mut t = String::from_utf8_lossy(&o.stdout).to_string();
    t.push_str(&String::from_utf8_lossy(&o.stderr));
    if o.status.success() {
        Ok(t.trim().to_string())
    } else {
        Err(t.trim().to_string())
    }
}

fn init() -> Result<(), String> {
    if store().join(".git").is_dir() {
        return Ok(());
    }
    git(&["init", "-q", "-b", "main"]).or_else(|_| git(&["init", "-q"]))?;
    Ok(())
}

/// Copy the tracked files of `profile` into the store; a file that is gone from the profile is removed there,
/// so a deletion is versioned too.
fn copy_in(profile: &Path) -> Result<(), String> {
    let dir = store();
    for f in FILES {
        let (from, to) = (profile.join(f), dir.join(f));
        if from.is_file() {
            if let Some(d) = to.parent() {
                fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
            }
            fs::copy(&from, &to).map_err(|e| format!("{}: {e}", from.display()))?;
        } else if to.is_file() {
            fs::remove_file(&to).map_err(|e| format!("{}: {e}", to.display()))?;
        }
    }
    Ok(())
}

/// Save the profile's settings as a new version when they differ from the last one. Returns the short id of
/// the new version, or None when nothing had changed.
pub fn snapshot(profile: &Path, why: &str) -> Result<Option<String>, String> {
    init()?;
    copy_in(profile)?;
    git(&["add", "-A"])?;
    if git(&["status", "--porcelain"])?.is_empty() {
        return Ok(None);
    }
    git(&["commit", "-q", "-m", &format!("firefox settings: {why}")])?;
    git(&["rev-parse", "--short", "HEAD"]).map(Some)
}

/// Put the saved settings (the newest, or an older version `rev`) into the profile. What the profile had is
/// saved as a version first, so a restore can itself be taken back.
pub fn restore(profile: &Path, rev: Option<&str>) -> Result<String, String> {
    snapshot(profile, "before restore")?;
    let rev = rev.unwrap_or("HEAD");
    let listed = git(&["ls-tree", "-r", "--name-only", rev])?;
    let have: Vec<&str> = listed.lines().filter(|l| FILES.contains(l)).collect();
    for f in FILES {
        let to = profile.join(f);
        if have.contains(&f) {
            let o = Command::new("git")
                .arg("-C")
                .arg(store())
                .args(["show", &format!("{rev}:{f}")])
                .output()
                .map_err(|e| format!("git: {e}"))?;
            if !o.status.success() {
                return Err(String::from_utf8_lossy(&o.stderr).trim().to_string());
            }
            if let Some(d) = to.parent() {
                fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
            }
            fs::write(&to, o.stdout).map_err(|e| format!("{}: {e}", to.display()))?;
        } else if to.is_file() {
            fs::remove_file(&to).map_err(|e| format!("{}: {e}", to.display()))?;
        }
    }
    snapshot(profile, &format!("restored {rev}"))?;
    Ok(format!("{} file(s) restored from {rev}", have.len()))
}

/// The last `n` versions, newest first: "abc1234 firefox settings: fullscreen fix on".
pub fn history(n: usize) -> Vec<String> {
    git(&["log", &format!("-{n}"), "--format=%h  %ar  %s"])
        .map(|t| t.lines().map(str::to_string).collect())
        .unwrap_or_default()
}

pub fn remote() -> Option<String> {
    git(&["remote", "get-url", "origin"]).ok().filter(|u| !u.is_empty())
}

/// Set the push address (`origin`). Anything git accepts: an ssh or https address, or a folder.
pub fn set_remote(url: &str) -> Result<(), String> {
    let url = url.trim();
    if url.is_empty() {
        return Err("type a repository address first".into());
    }
    init()?;
    if remote().is_some() {
        git(&["remote", "set-url", "origin", url])?;
    } else {
        git(&["remote", "add", "origin", url])?;
    }
    Ok(())
}

/// Push the history. The branch is whatever the store has; nothing is rewritten or forced.
pub fn push() -> Result<String, String> {
    if remote().is_none() {
        return Err("no repository address yet: type one and press Save address".into());
    }
    if git(&["rev-parse", "--verify", "-q", "HEAD"]).is_err() {
        return Err("nothing saved yet".into());
    }
    git(&["push", "-u", "origin", "HEAD"]).map(|t| if t.is_empty() { "pushed".into() } else { t })
}

/// Size and modification time of every tracked file: when it differs from the last look, something changed
/// the settings. A few `stat` calls, cheap enough to do every few seconds.
pub fn fingerprint(profile: &Path) -> String {
    FILES
        .iter()
        .map(|f| match fs::metadata(profile.join(f)) {
            Ok(m) => format!(
                "{f}:{}:{}",
                m.len(),
                m.modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_nanos())
                    .unwrap_or(0)
            ),
            Err(_) => format!("{f}:-"),
        })
        .collect::<Vec<_>>()
        .join("|")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static LOCK: Mutex<()> = Mutex::new(());

    /// A profile and a store of their own; tests share the process environment, so they run one at a time.
    fn fake(name: &str) -> (std::sync::MutexGuard<'static, ()>, PathBuf, PathBuf) {
        let g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let root = std::env::temp_dir().join(format!("sdl-ffsync-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let prof = root.join("profile");
        fs::create_dir_all(prof.join("chrome")).unwrap();
        std::env::set_var("XDG_CONFIG_HOME", root.join("config"));
        (g, prof, root)
    }

    #[test]
    fn a_change_is_a_new_version_and_no_change_is_none() {
        let (_g, p, _r) = fake("versions");
        fs::write(p.join("user.js"), "user_pref(\"a\", 1);\n").unwrap();
        let first = snapshot(&p, "first").unwrap();
        assert!(first.is_some());
        assert_eq!(snapshot(&p, "again").unwrap(), None, "same files, same version");
        fs::write(p.join("user.js"), "user_pref(\"a\", 2);\n").unwrap();
        assert!(snapshot(&p, "second").unwrap().is_some());
        let h = history(10);
        assert_eq!(h.len(), 2, "{h:?}");
        assert!(h[0].contains("second") && h[1].contains("first"), "{h:?}");
    }

    #[test]
    fn restore_brings_back_an_old_version_and_can_itself_be_undone() {
        let (_g, p, _r) = fake("restore");
        fs::write(p.join("user.js"), "one\n").unwrap();
        let v1 = snapshot(&p, "one").unwrap().unwrap();
        fs::write(p.join("user.js"), "two\n").unwrap();
        fs::write(p.join("chrome/userChrome.css"), "x{}\n").unwrap();
        snapshot(&p, "two").unwrap();
        restore(&p, Some(&v1)).unwrap();
        assert_eq!(fs::read_to_string(p.join("user.js")).unwrap(), "one\n");
        assert!(!p.join("chrome/userChrome.css").exists(), "a file that was not in that version goes");
        // the state before the restore is still in the history
        assert!(history(10).iter().any(|l| l.contains("before restore") || l.contains("restored")), "{:?}", history(10));
        let two = history(10).into_iter().find(|l| l.contains("two")).unwrap();
        restore(&p, Some(two.split_whitespace().next().unwrap())).unwrap();
        assert_eq!(fs::read_to_string(p.join("user.js")).unwrap(), "two\n");
    }

    #[test]
    fn push_goes_to_a_remote_and_says_so_when_there_is_none() {
        let (_g, p, r) = fake("push");
        fs::write(p.join("user.js"), "one\n").unwrap();
        snapshot(&p, "one").unwrap();
        assert!(push().unwrap_err().contains("no repository address"));
        assert!(set_remote("  ").is_err());
        let bare = r.join("remote.git");
        Command::new("git").args(["init", "-q", "--bare"]).arg(&bare).status().unwrap();
        set_remote(bare.to_str().unwrap()).unwrap();
        assert_eq!(remote().as_deref(), bare.to_str());
        push().unwrap();
        let log = Command::new("git").arg("-C").arg(&bare).args(["log", "--oneline", "--all"]).output().unwrap();
        assert!(String::from_utf8_lossy(&log.stdout).contains("firefox settings: one"));
        // saying the address again replaces it, it does not add a second one
        set_remote(bare.to_str().unwrap()).unwrap();
    }

    #[test]
    fn the_fingerprint_changes_when_a_tracked_file_changes_and_not_otherwise() {
        let (_g, p, _r) = fake("print");
        let a = fingerprint(&p);
        assert_eq!(a, fingerprint(&p));
        fs::write(p.join("prefs.js"), "not tracked\n").unwrap();
        assert_eq!(a, fingerprint(&p), "prefs.js is Firefox's own and is not saved");
        fs::write(p.join("user.js"), "x\n").unwrap();
        assert_ne!(a, fingerprint(&p));
    }
}
