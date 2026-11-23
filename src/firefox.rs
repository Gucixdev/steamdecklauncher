//! Firefox preferences the Fixes page sets. Each fix is a block of `user_pref` lines in the profile's `user.js`
//! between two marker lines, so applying it twice changes nothing, undoing it removes exactly that block and
//! every other line of the file stays as it was. `user.js` is read by Firefox at every start, which is why
//! the setting survives updates; Firefox has to be restarted once for a change to apply.
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Fullscreen video from Gaming Mode: the fix people report (a Steam Deck blog post, forum threads), not an
/// official Mozilla one. The bash `setup` writes the privacy block under the id [`PRIVACY`].
pub const FULLSCREEN: &str = "fullscreen";
pub const FULLSCREEN_PREFS: &str = "user_pref(\"full-screen-api.ignore-widgets\", true);";
/// The block the `setup --firefox` step writes (its markers say just `steamdecklauncher setup`).
pub const PRIVACY: &str = "setup";

fn markers(id: &str) -> (String, String) {
    (
        format!("// >>> steamdecklauncher {id}"),
        format!("// <<< steamdecklauncher {id}"),
    )
}

/// The profile Firefox uses: `$FIREFOX_PROFILE` when set, else the one `installs.ini` / `profiles.ini` name
/// (native and Flatpak installs), else the first `*.default*` folder.
pub fn profile() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("FIREFOX_PROFILE") {
        let p = PathBuf::from(p);
        return p.is_dir().then_some(p);
    }
    profile_under(&dirs::home_dir()?)
}

pub fn profile_under(home: &Path) -> Option<PathBuf> {
    let bases = [
        ".var/app/org.mozilla.firefox/config/mozilla/firefox",
        ".var/app/org.mozilla.firefox/.mozilla/firefox",
        ".config/mozilla/firefox",
        ".mozilla/firefox",
    ];
    for b in bases {
        let base = home.join(b);
        if !base.is_dir() {
            continue;
        }
        // the profile this install really uses is in installs.ini; profiles.ini's Default=1 can be stale
        if let Some(d) = ini_value(&base.join("installs.ini"), "Default").map(|d| base.join(d)) {
            if d.is_dir() {
                return Some(d);
            }
        }
        if let Some(d) = default_from_profiles(&base.join("profiles.ini")).map(|d| base.join(d)) {
            if d.is_dir() {
                return Some(d);
            }
        }
        let mut found: Vec<PathBuf> = fs::read_dir(&base)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.is_dir()
                    && p.file_name()
                        .map(|n| n.to_string_lossy().contains(".default"))
                        .unwrap_or(false)
            })
            .collect();
        found.sort();
        if let Some(d) = found.into_iter().next() {
            return Some(d);
        }
    }
    None
}

/// First `KEY=value` line of an ini file.
fn ini_value(file: &Path, key: &str) -> Option<String> {
    let t = fs::read_to_string(file).ok()?;
    t.lines()
        .find_map(|l| l.strip_prefix(&format!("{key}=")).map(|v| v.trim().to_string()))
        .filter(|v| !v.is_empty())
}

/// The `Path=` of the section that has `Default=1`.
fn default_from_profiles(file: &Path) -> Option<String> {
    let t = fs::read_to_string(file).ok()?;
    let mut path = None;
    for l in t.lines() {
        if l.starts_with('[') {
            path = None;
        } else if let Some(p) = l.strip_prefix("Path=") {
            path = Some(p.trim().to_string());
        } else if l.trim() == "Default=1" {
            return path.filter(|p| !p.is_empty());
        }
    }
    None
}

fn user_js(profile: &Path) -> PathBuf {
    profile.join("user.js")
}

/// Everything except the block `id` (from its start marker to its end marker, both included).
fn strip(text: &str, id: &str) -> String {
    let (s, e) = markers(id);
    let mut out = String::new();
    let mut skip = false;
    for l in text.lines() {
        if l == s {
            skip = true;
        }
        if !skip {
            out.push_str(l);
            out.push('\n');
        }
        if l == e {
            skip = false;
        }
    }
    out
}

pub fn has_block(profile: &Path, id: &str) -> bool {
    let (s, _) = markers(id);
    fs::read_to_string(user_js(profile))
        .map(|t| t.lines().any(|l| l == s))
        .unwrap_or(false)
}

/// Write (or replace) the block `id`. The first time, the old `user.js` is kept as `user.js.steamdecklauncher-bak`.
pub fn set_block(profile: &Path, id: &str, prefs: &str) -> io::Result<()> {
    let js = user_js(profile);
    let old = fs::read_to_string(&js).unwrap_or_default();
    let bak = profile.join("user.js.steamdecklauncher-bak");
    if js.is_file() && !bak.exists() {
        fs::copy(&js, &bak)?;
    }
    let (s, e) = markers(id);
    let mut text = strip(&old, id);
    text.push_str(&format!("{s}\n{}\n{e}\n", prefs.trim_end()));
    replace(&js, &text)
}

/// Remove the block `id`; false when it was not there. Nothing else in the file changes.
pub fn remove_block(profile: &Path, id: &str) -> io::Result<bool> {
    if !has_block(profile, id) {
        return Ok(false);
    }
    let js = user_js(profile);
    let text = strip(&fs::read_to_string(&js)?, id);
    replace(&js, &text)?;
    Ok(true)
}

/// Written whole and renamed into place, so Firefox never reads a half file.
fn replace(js: &Path, text: &str) -> io::Result<()> {
    let tmp = js.with_extension("js.steamdecklauncher-tmp");
    fs::write(&tmp, text)?;
    fs::rename(&tmp, js)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("sdl-ff-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_block_is_written_once_and_replaced_not_repeated() {
        let p = tmp("twice");
        set_block(&p, FULLSCREEN, FULLSCREEN_PREFS).unwrap();
        set_block(&p, FULLSCREEN, FULLSCREEN_PREFS).unwrap();
        let t = fs::read_to_string(p.join("user.js")).unwrap();
        assert_eq!(t.matches("full-screen-api.ignore-widgets").count(), 1, "{t}");
        assert!(has_block(&p, FULLSCREEN));
    }

    #[test]
    fn undo_leaves_every_other_line_and_other_blocks_alone() {
        let p = tmp("undo");
        fs::write(p.join("user.js"), "user_pref(\"mine\", 1);\n").unwrap();
        set_block(&p, PRIVACY, "user_pref(\"a\", false);").unwrap();
        set_block(&p, FULLSCREEN, FULLSCREEN_PREFS).unwrap();
        assert!(remove_block(&p, FULLSCREEN).unwrap());
        let t = fs::read_to_string(p.join("user.js")).unwrap();
        assert!(t.contains("user_pref(\"mine\", 1);") && t.contains("user_pref(\"a\", false);"), "{t}");
        assert!(!t.contains("ignore-widgets") && !has_block(&p, FULLSCREEN) && has_block(&p, PRIVACY));
        assert!(!remove_block(&p, FULLSCREEN).unwrap(), "second undo is a no-op");
        assert!(remove_block(&p, PRIVACY).unwrap());
        assert_eq!(fs::read_to_string(p.join("user.js")).unwrap(), "user_pref(\"mine\", 1);\n");
    }

    #[test]
    fn the_first_write_keeps_the_old_file_once() {
        let p = tmp("bak");
        fs::write(p.join("user.js"), "original\n").unwrap();
        set_block(&p, FULLSCREEN, FULLSCREEN_PREFS).unwrap();
        set_block(&p, PRIVACY, "user_pref(\"x\", 1);").unwrap();
        assert_eq!(fs::read_to_string(p.join("user.js.steamdecklauncher-bak")).unwrap(), "original\n");
    }

    #[test]
    fn the_profile_comes_from_installs_ini_then_profiles_ini_then_a_default_folder() {
        let h = tmp("home");
        let base = h.join(".mozilla/firefox");
        fs::create_dir_all(base.join("aaa.stale")).unwrap();
        fs::create_dir_all(base.join("bbb.real")).unwrap();
        fs::write(base.join("profiles.ini"), "[Profile0]\nName=x\nPath=aaa.stale\nDefault=1\n").unwrap();
        assert_eq!(profile_under(&h), Some(base.join("aaa.stale")));
        fs::write(base.join("installs.ini"), "[ABC]\nDefault=bbb.real\nLocked=1\n").unwrap();
        assert_eq!(profile_under(&h), Some(base.join("bbb.real")));
        let h2 = tmp("home2");
        fs::create_dir_all(h2.join(".mozilla/firefox/zzz.default-release")).unwrap();
        assert_eq!(profile_under(&h2), Some(h2.join(".mozilla/firefox/zzz.default-release")));
        assert_eq!(profile_under(&tmp("empty")), None);
    }
}
