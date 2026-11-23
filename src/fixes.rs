//! The Fixes tab: one-off tweaks, each with the problem it solves and exactly what it changes written next to
//! its button. Firefox ones write a marked block into the profile's `user.js` (see `firefox.rs`); the system
//! ones run the same `setup --system` actions the bash wizard has (OOM guard, memory settings, swap file, ssh),
//! output shown and logged. The script is embedded in the program, so one binary is enough on the Deck; root
//! comes from pkexec (or sudo in a terminal) and only for the system actions.
use crate::firefox;
use crate::log;
use crate::ui::{heading, segment, ACCENT, DIM, SMALL_SIZE, SPACING, TEXT_SIZE};
use iced::widget::{column, row, scrollable, text, text_input};
use iced::{Element, Fill, Task};
use std::path::PathBuf;
use std::process::Command;

const SETUP: &str = include_str!("../setup");

#[derive(Debug, Clone)]
pub enum Msg {
    /// `setup --system ARGS` (root)
    Run(Vec<String>),
    /// `setup ARGS`, no root (the Firefox step downloads uBlock Origin)
    Setup(Vec<String>),
    Done(String, String),
    Swappiness(String),
    SwapSize(String),
    Fullscreen(bool),
    PrivacyOff,
    Refresh,
}

pub struct State {
    output: String,
    busy: bool,
    swappiness: String,
    swap: String,
    ff: Option<PathBuf>,
    fullscreen: bool,
    privacy: bool,
}

impl State {
    pub fn new() -> (Self, Task<Msg>) {
        let mut s = State {
            output: String::new(),
            busy: false,
            swappiness: "100".into(),
            swap: "8G".into(),
            ff: None,
            fullscreen: false,
            privacy: false,
        };
        s.look();
        (
            s,
            Task::perform(
                crate::ui::blocking(move || run_setup(vec!["--system".into(), "status".into()], false)),
                |(a, o)| Msg::Done(a, o),
            ),
        )
    }

    pub fn last_error(&self) -> Option<&str> {
        None
    }

    /// Read what the Firefox profile says now.
    fn look(&mut self) {
        self.ff = firefox::profile();
        self.fullscreen = self.ff.as_deref().is_some_and(|p| firefox::has_block(p, firefox::FULLSCREEN));
        self.privacy = self.ff.as_deref().is_some_and(|p| firefox::has_block(p, firefox::PRIVACY));
    }

    fn firefox_change(&mut self, what: &str, r: std::io::Result<String>) {
        self.output = match r {
            Ok(m) => format!("{what}: {m}\nRestart Firefox for it to apply."),
            Err(e) => format!("{what}: could not write the Firefox profile: {e}"),
        };
        log::write("fixes", &self.output);
        self.look();
    }

    pub fn update(&mut self, m: Msg) -> Task<Msg> {
        match m {
            Msg::Swappiness(s) => self.swappiness = s,
            Msg::SwapSize(s) => self.swap = s,
            Msg::Refresh => {
                self.look();
                return self.update(Msg::Run(vec!["status".into()]));
            }
            Msg::Run(a) => {
                let mut args = vec!["--system".to_string()];
                args.extend(a);
                return self.start(args, true);
            }
            Msg::Setup(a) => return self.start(a, false),
            Msg::Done(a, o) => {
                self.busy = false;
                log::write("fixes", &format!("{a}\n{o}"));
                self.output = o;
                self.look();
            }
            Msg::Fullscreen(on) => {
                if let Some(p) = self.ff.clone() {
                    let r = if on {
                        firefox::set_block(&p, firefox::FULLSCREEN, firefox::FULLSCREEN_PREFS)
                            .map(|_| "full-screen-api.ignore-widgets is now true".to_string())
                    } else {
                        firefox::remove_block(&p, firefox::FULLSCREEN)
                            .map(|_| "the preference block is removed".to_string())
                    };
                    self.firefox_change("fullscreen fix", r);
                }
            }
            Msg::PrivacyOff => {
                if let Some(p) = self.ff.clone() {
                    let r = firefox::remove_block(&p, firefox::PRIVACY).map(|_| {
                        "the privacy preferences are removed (uBlock Origin stays: remove it under Add-ons)".to_string()
                    });
                    self.firefox_change("privacy", r);
                }
            }
        }
        Task::none()
    }

    fn start(&mut self, args: Vec<String>, root: bool) -> Task<Msg> {
        self.busy = true;
        self.output = format!("running: {}\n", args.join(" "));
        Task::perform(crate::ui::blocking(move || run_setup(args, root)), |(a, o)| {
            Msg::Done(a, o)
        })
    }

    pub fn view(&self) -> Element<'_, Msg> {
        let go = |label: &'static str, args: Vec<String>| segment(label, false, false, Msg::Run(args));
        let ok_swappiness = self.swappiness.parse::<u32>().map(|n| n <= 200).unwrap_or(false);
        let ok_swap = {
            let t = self.swap.trim_end_matches(['G', 'g', 'M', 'm']);
            !t.is_empty() && t.chars().all(|c| c.is_ascii_digit()) && self.swap.len() > t.len()
        };
        let have_ff = self.ff.is_some();
        let ff_note = match &self.ff {
            Some(p) => format!("Profile: {}", shorten(p)),
            None => "No Firefox profile found. Start Firefox once, then press Refresh.".to_string(),
        };
        // the buttons of a Firefox fix are only there when there is a profile to write to
        let ff_buttons = |on: bool, apply: Element<'static, Msg>, undo: Element<'static, Msg>| -> Element<'static, Msg> {
            if !have_ff {
                text("").into()
            } else if on {
                undo
            } else {
                apply
            }
        };
        let mut col = column![
            heading("Fixes"),
            text("Small one-off changes. Each says what is wrong, what it changes, and how to take it back.")
                .size(SMALL_SIZE)
                .color(DIM),
            heading("Firefox"),
            text(ff_note).size(SMALL_SIZE).color(DIM),
            card(
                "Video fullscreen in Gaming Mode",
                have_ff.then_some(self.fullscreen),
                "Started from Gaming Mode, Firefox puts a fullscreen video at the wrong size (Desktop Mode is fine). This is the fix Deck users report, not an official Mozilla one.",
                "Sets full-screen-api.ignore-widgets to true. That is the about:config switch, kept in the profile's user.js so it is applied at every start. Restart Firefox. Undo removes it.",
                ff_buttons(
                    self.fullscreen,
                    segment("Fix fullscreen", false, false, Msg::Fullscreen(true)),
                    segment("Undo fullscreen", false, true, Msg::Fullscreen(false)),
                ),
            ),
            card(
                "Privacy and clutter",
                have_ff.then_some(self.privacy),
                "Firefox sends usage data, shows sponsored tiles and suggestions, and WebRTC can show websites more of your network addresses than they need.",
                "Telemetry, studies, sponsored content and Pocket off; HTTPS-only mode on (it asks before opening a plain http site); tracking protection on; WebRTC limited to the default address; uBlock Origin downloaded from addons.mozilla.org into the profile. Undo removes the preferences; uBlock Origin stays.",
                ff_buttons(
                    self.privacy,
                    segment("Apply privacy prefs", false, false, Msg::Setup(vec!["--firefox".into()])),
                    segment("Undo privacy prefs", false, true, Msg::PrivacyOff),
                ),
            ),
            heading("System"),
            text("These change files in /etc, which survive SteamOS updates. Each asks for your password.")
                .size(SMALL_SIZE)
                .color(DIM),
            card(
                "OOM guard",
                None,
                "When a game uses all the memory the Deck can freeze for a long time instead of closing the game.",
                "Configures systemd-oomd to kill the biggest offender in the user session (the game) at 60% memory pressure or 90% swap use, and keeps the Gaming Mode compositor out of reach. Remove deletes only the files this program wrote.",
                row![
                    go("set up", vec!["oom".into()]),
                    go("remove", vec!["undo".into()])
                ]
                .spacing(8)
                .into(),
            ),
            card(
                "Memory settings",
                None,
                "Memory-hungry games on a Deck that swaps into zram (compressed memory).",
                "Sets vm.swappiness (the value below), vm.vfs_cache_pressure = 50 and vm.page-cluster = 0 in a file under /etc, values that suit zram.",
                row![
                    text_input("100", &self.swappiness)
                        .on_input(Msg::Swappiness)
                        .width(80)
                        .padding(8)
                        .size(SMALL_SIZE),
                    if ok_swappiness {
                        go("apply", vec!["--swappiness".into(), self.swappiness.clone(), "sysctl".into()])
                    } else {
                        text("0-200").size(SMALL_SIZE).color(DIM).into()
                    },
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center)
                .into(),
            ),
            card(
                "Swap file",
                None,
                "The swap file can be too small for the biggest games.",
                "Resizes /home/swapfile to the size you type, after checking that the free space is enough.",
                row![
                    text_input("8G", &self.swap)
                        .on_input(Msg::SwapSize)
                        .width(80)
                        .padding(8)
                        .size(SMALL_SIZE),
                    if ok_swap {
                        go("resize", vec!["swapfile".into(), self.swap.clone()])
                    } else {
                        text("e.g. 8G").size(SMALL_SIZE).color(DIM).into()
                    },
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center)
                .into(),
            ),
            card(
                "SSH",
                None,
                "You cannot copy files to the Deck or log in from your PC: SSH is off.",
                "Enables sshd. Set a password first (run passwd in a terminal).",
                go("enable", vec!["ssh".into()]),
            ),
            card(
                "State",
                None,
                "Is any of the system changes active?",
                "Reads only; changes nothing.",
                segment("Refresh", false, false, Msg::Refresh),
            ),
        ]
        .spacing(SPACING);
        if self.busy {
            col = col.push(text("working...").size(TEXT_SIZE).color(DIM));
        }
        col = col.push(text(self.output.clone()).size(SMALL_SIZE));
        scrollable(col.padding(iced::Padding {
            right: 16.0,
            ..iced::Padding::ZERO
        }))
        .height(Fill)
        .into()
    }
}

/// One fix: its name and whether it is on, the problem in plain words, exactly what it changes, and the buttons.
fn card<'a>(
    title: &'static str,
    on: Option<bool>,
    problem: &'static str,
    does: &'static str,
    controls: Element<'a, Msg>,
) -> Element<'a, Msg> {
    let state: Element<'a, Msg> = match on {
        Some(true) => text("on").size(SMALL_SIZE).color(ACCENT).into(),
        Some(false) => text("off").size(SMALL_SIZE).color(DIM).into(),
        None => text("").into(),
    };
    column![
        row![text(title).size(TEXT_SIZE), state]
            .spacing(SPACING)
            .align_y(iced::Alignment::Center),
        text(format!("Problem: {problem}")).size(SMALL_SIZE),
        text(format!("What it does: {does}")).size(SMALL_SIZE).color(DIM),
        controls,
    ]
    .spacing(4)
    .into()
}

/// `/home/you/...` shown as `~/...`: a window that gets screenshotted should not carry the login name.
fn shorten(p: &std::path::Path) -> String {
    match dirs::home_dir().and_then(|h| p.strip_prefix(h).ok().map(|r| r.to_path_buf())) {
        Some(r) => format!("~/{}", r.display()),
        None => p.display().to_string(),
    }
}

/// Run the embedded setup script with ARGS and return (what was run, its output). `status` needs no root.
fn run_setup(args: Vec<String>, root: bool) -> (String, String) {
    let label = args.join(" ");
    let script = std::env::temp_dir().join(format!("steamdecklauncher-setup-{}.sh", std::process::id()));
    if let Err(e) = std::fs::write(&script, SETUP) {
        return (label, format!("could not write {}: {e}", script.display()));
    }
    let needs_root = root && args.iter().any(|a| a != "status" && a != "--system");
    let mut cmd;
    if needs_root && std::env::var("USER").map(|u| u != "root").unwrap_or(true) {
        if which("pkexec") {
            cmd = Command::new("pkexec");
            cmd.arg("bash");
        } else {
            cmd = Command::new("sudo");
            cmd.arg("bash");
        }
    } else {
        cmd = Command::new("bash");
    }
    cmd.arg(&script).args(&args);
    let out = match cmd.output() {
        Ok(o) => {
            let mut t = String::from_utf8_lossy(&o.stdout).to_string();
            t.push_str(&String::from_utf8_lossy(&o.stderr));
            if !o.status.success() {
                t.push_str(&format!("\n(exit status {})", o.status));
            }
            t
        }
        Err(e) => format!("could not start the script: {e}"),
    };
    let _ = std::fs::remove_file(&script);
    (label, out)
}

fn which(name: &str) -> bool {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| d.join(name).is_file()))
        .unwrap_or(false)
}
