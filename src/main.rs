use whalu::config::{Config, config_dir};
#[cfg(target_os = "macos")]
mod macos;
fn main() {
    if let Err(e) = main_result() {
        eprintln!("whalu: {e}");
        std::process::exit(1);
    }
}
fn main_result() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--help" | "-h") => {
            println!(
                "Whalu {} · native macOS window manager\n\n  whalu                    Run menu-bar app\n  --doctor                 Check permissions and platform support\n  --list                   List manageable windows in current Space\n  --performance-probe      Measure discovery and cached navigation checks\n  --native-smoke-test      Test temporary native fixture windows\n  --space-smoke-test       Test native drag-and-follow Space transfers\n  --gesture-probe [secs]    Check three-finger recognition without navigation\n  --preview <output.png> [--hold]  Render sample HUD; keep overview visible\n  --permissions            Open Accessibility settings\n  --print-config           Print default configuration\n  --check-config [path]    Validate configuration\n  --config <path>          Run using configuration file\n\nDefault config: {}",
                env!("CARGO_PKG_VERSION"),
                config_dir().join("config.toml").display()
            );
            return Ok(());
        }
        Some("--print-config") => {
            println!(
                "{}",
                toml::to_string_pretty(&Config::default()).map_err(|e| e.to_string())?
            );
            return Ok(());
        }
        Some("--check-config") => {
            let path = args
                .get(1)
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| config_dir().join("config.toml"));
            if args.len() > 1 && !path.exists() {
                return Err(format!(
                    "Configuration file does not exist: {}",
                    path.display()
                ));
            }
            Config::load(&path)?;
            println!("Configuration valid: {}", path.display());
            return Ok(());
        }
        _ => {}
    }
    #[cfg(target_os = "macos")]
    {
        match args.first().map(String::as_str) {
            Some("--gesture-probe") => {
                let seconds = args
                    .get(1)
                    .map(|s| s.parse::<f64>())
                    .transpose()
                    .map_err(|e| e.to_string())?
                    .unwrap_or(10.);
                if !seconds.is_finite() || !(1. ..=120.).contains(&seconds) {
                    return Err("Probe duration must be between 1 and 120 seconds".into());
                }
                macos::gesture_probe(seconds)
            }
            Some("--native-smoke-test") => macos::probe::smoke(),
            Some("--space-smoke-test") => macos::probe::space_smoke(),
            Some("--performance-probe") => macos::probe::performance(),
            Some("--fixture") => macos::probe::fixture(),
            Some("--fixture-minimum-size") => macos::probe::constrained_fixture(),
            Some("--preview") if args.len() == 2 || (args.len() == 3 && args[2] == "--hold") => {
                macos::probe::preview(&args[1], args.len() == 3)
            }
            Some("--doctor") => {
                macos::doctor();
                Ok(())
            }
            Some("--list") => macos::list(),
            Some("--permissions") => {
                std::process::Command::new("/usr/bin/open").arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility").status().map_err(|e|e.to_string())?;
                Ok(())
            }
            Some("--config") if args.len() == 2 => macos::run(
                Config::load(std::path::Path::new(&args[1]))?,
                std::path::PathBuf::from(&args[1]),
            ),
            None => {
                whalu::config::migrate_legacy_data()?;
                let path = config_dir().join("config.toml");
                macos::run(Config::load(&path)?, path)
            }
            Some(arg) => Err(format!("Unknown or incomplete argument {arg}; see --help")),
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("The native adapter requires macOS. The Rust core and tests are portable.".into())
    }
}
