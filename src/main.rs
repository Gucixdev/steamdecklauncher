//! steamdecklauncher: one program. No arguments opens the window; `protonup ...` and `flatseal ...` (or the
//! program started under one of those names, as the symlinks `setup` makes) are the command lines.
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().collect();
    let name = args
        .first()
        .map(|a| a.rsplit('/').next().unwrap_or(a).to_string())
        .unwrap_or_default();
    args.remove(0);
    let (tool, rest) = match name.as_str() {
        "protonup" => ("protonup".to_string(), args),
        "flatseal" => ("flatseal".to_string(), args),
        _ => match args.first().map(String::as_str) {
            Some("protonup") | Some("flatseal") => (args[0].clone(), args[1..].to_vec()),
            Some("--help") | Some("-h") | Some("help") => {
                println!("steamdecklauncher: ProtonUp, Flatseal and system setup in one window\n\n  steamdecklauncher [storage|steam|apps|health|settings|log]   open the window (on that page)\n  steamdecklauncher protonup ...     the ProtonUp command line (try: steamdecklauncher protonup --help)\n  steamdecklauncher flatseal ...     the Flatseal command line\n\nLog: {}", steamdecklauncher::log::path().display());
                return ExitCode::SUCCESS;
            }
            Some("--version") | Some("-V") => {
                println!("steamdecklauncher {}", env!("CARGO_PKG_VERSION"));
                return ExitCode::SUCCESS;
            }
            other => {
                const PAGES: [&str; 8] = [
                    "home", "steam", "apps", "scripts", "storage", "health", "system", "log",
                ];
                if let Some(o) = other {
                    if o != "settings" && !PAGES.contains(&o) {
                        eprintln!("steamdecklauncher: unknown command '{o}' (try --help)");
                        return ExitCode::from(2);
                    }
                }
                let section = match other {
                    Some("steam") => steamdecklauncher::launcher::Section::Steam,
                    Some("apps") => steamdecklauncher::launcher::Section::Apps,
                    Some("scripts") => steamdecklauncher::launcher::Section::Scripts,
                    Some("storage") => steamdecklauncher::launcher::Section::Storage,
                    Some("health") => steamdecklauncher::launcher::Section::Health,
                    Some("system") | Some("settings") => steamdecklauncher::launcher::Section::System,
                    Some("log") => steamdecklauncher::launcher::Section::Log,
                    _ => steamdecklauncher::launcher::Section::Home,
                };
                return match steamdecklauncher::launcher::run(steamdecklauncher::launcher::Start {
                    section,
                    ..Default::default()
                }) {
                    Ok(()) => ExitCode::SUCCESS,
                    Err(e) => {
                        steamdecklauncher::log::write("launcher", &format!("window failed: {e}"));
                        eprintln!("steamdecklauncher: {e}");
                        ExitCode::FAILURE
                    }
                };
            }
        },
    };
    steamdecklauncher::log::install_panic_hook();
    match tool.as_str() {
        "protonup" => steamdecklauncher::cli_protonup::run(rest),
        _ => steamdecklauncher::cli_flatseal::run(rest),
    }
}
