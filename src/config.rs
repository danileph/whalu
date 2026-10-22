use crate::{core::Command, geometry::Direction, input::Modifiers};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, path::PathBuf};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub gap: f64,
    pub overlay: bool,
    /// Legacy setting, accepted for existing configuration files but no longer used.
    #[serde(skip_serializing)]
    pub overlay_brightness: f64,
    pub overlay_delay_ms: u64,
    pub overlay_modifiers: Modifiers,
    pub magnetic: bool,
    pub gestures: bool,
    pub ignored_apps: Vec<String>,
    pub bindings: Vec<Binding>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub keycode: u16,
    pub modifiers: Modifiers,
    pub action: Action,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Next,
    Previous,
    AppNext,
    AppPrevious,
    ToggleMode,
    SnapLeft,
    SnapRight,
    SnapFull,
    Restore,
}
impl Action {
    pub fn command(self) -> Command {
        match self {
            Self::Next => Command::FocusNext,
            Self::Previous => Command::FocusPrevious,
            Self::AppNext => Command::AppNext,
            Self::AppPrevious => Command::AppPrevious,
            Self::ToggleMode => Command::ToggleMode,
            Self::SnapLeft => Command::Snap(Direction::Left),
            Self::SnapRight => Command::Snap(Direction::Right),
            Self::SnapFull => Command::Snap(Direction::Up),
            Self::Restore => Command::Snap(Direction::Down),
        }
    }
}
impl Default for Config {
    fn default() -> Self {
        let base = Modifiers {
            control: true,
            option: true,
            ..Default::default()
        };
        let mut bindings = vec![];
        for (key, action) in [
            (126, Action::Previous),
            (125, Action::Next),
            (123, Action::AppPrevious),
            (124, Action::AppNext),
            (49, Action::ToggleMode),
        ] {
            bindings.push(Binding {
                keycode: key,
                modifiers: base,
                action,
            });
        }
        for (key, action) in [
            (126, Action::SnapFull),
            (125, Action::Restore),
            (123, Action::SnapLeft),
            (124, Action::SnapRight),
        ] {
            bindings.push(Binding {
                keycode: key,
                modifiers: Modifiers {
                    command: true,
                    ..base
                },
                action,
            });
        }
        Self {
            gap: 8.,
            overlay: true,
            overlay_brightness: 0.10,
            overlay_delay_ms: 120,
            overlay_modifiers: base,
            magnetic: true,
            gestures: true,
            ignored_apps: vec![],
            bindings,
        }
    }
}
impl Config {
    pub fn validate(&self) -> Result<(), String> {
        if !self.gap.is_finite() || !(0. ..=100.).contains(&self.gap) {
            return Err("gap must be finite and between 0 and 100".into());
        }
        if self.overlay_delay_ms > 2000 {
            return Err("overlay_delay_ms must be at most 2000".into());
        }
        if self.overlay_modifiers == Modifiers::default() {
            return Err("overlay_modifiers must contain at least one modifier".into());
        }
        let mut seen = BTreeSet::new();
        for b in &self.bindings {
            let m = b.modifiers;
            if b.keycode > 127 {
                return Err("keycode must be in 0..=127".into());
            }
            if !(m.control || m.option || m.command) {
                return Err("bindings require Control, Option, or Command".into());
            }
            if (m.control && !m.option && !m.command && [123, 124, 125, 126].contains(&b.keycode))
                || (m.command && !m.control && !m.option && b.keycode == 48)
                || (m.option && !m.control && !m.command && b.keycode == 46)
            {
                return Err("binding uses a reserved macOS/spec shortcut".into());
            }
            if !seen.insert((b.keycode, m.control, m.option, m.shift, m.command)) {
                return Err("duplicate shortcut".into());
            }
        }
        Ok(())
    }
    pub fn load(path: &std::path::Path) -> Result<Self, String> {
        let cfg = if path.exists() {
            toml::from_str(&std::fs::read_to_string(path).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?
        } else {
            Self::default()
        };
        cfg.validate()?;
        Ok(cfg)
    }
}
pub fn config_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Library/Application Support/Magnetic")
}
