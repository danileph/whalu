use crate::{
    core::Command,
    input::{Modifiers, NativeShortcut},
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, path::PathBuf};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Legacy layout setting, accepted but no longer used.
    #[serde(skip_serializing)]
    pub gap: f64,
    pub overlay: bool,
    /// Legacy setting, accepted for existing configuration files but no longer used.
    #[serde(skip_serializing)]
    pub overlay_brightness: f64,
    pub overlay_delay_ms: u64,
    pub overlay_modifiers: Modifiers,
    /// Legacy layout setting, accepted but no longer used.
    #[serde(skip_serializing)]
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
    StackUp,
    StackDown,
    SpacePrevious,
    SpaceNext,
    SnapLeft,
    SnapRight,
    SnapFull,
    SnapTop,
    SnapBottom,
    Center,
    ArrangeLeftRight,
    ArrangeRightLeft,
    ArrangeTopBottom,
    ArrangeBottomTop,
    Restore,
}
impl Action {
    pub fn command(self) -> Command {
        match self {
            Self::Next => Command::FocusNext,
            Self::Previous => Command::FocusPrevious,
            Self::AppNext => Command::AppNext,
            Self::AppPrevious => Command::AppPrevious,
            Self::StackUp => Command::StackUp,
            Self::StackDown => Command::StackDown,
            Self::SpacePrevious => Command::MoveSpace(false),
            Self::SpaceNext => Command::MoveSpace(true),
            Self::ToggleMode => Command::ToggleMode,
            Self::SnapLeft => Command::Native(NativeShortcut::tiling(123, false)),
            Self::SnapRight => Command::Native(NativeShortcut::tiling(124, false)),
            Self::SnapFull => Command::Native(NativeShortcut::tiling(3, false)),
            Self::SnapTop => Command::Native(NativeShortcut::tiling(126, false)),
            Self::SnapBottom => Command::Native(NativeShortcut::tiling(125, false)),
            Self::Center => Command::Native(NativeShortcut::tiling(8, false)),
            Self::ArrangeLeftRight => Command::Native(NativeShortcut::tiling(123, true)),
            Self::ArrangeRightLeft => Command::Native(NativeShortcut::tiling(124, true)),
            Self::ArrangeTopBottom => Command::Native(NativeShortcut::tiling(126, true)),
            Self::ArrangeBottomTop => Command::Native(NativeShortcut::tiling(125, true)),
            Self::Restore => Command::Native(NativeShortcut::tiling(15, false)),
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
        let navigation = Modifiers {
            command: true,
            ..base
        };
        let positioning = Modifiers {
            control: true,
            command: true,
            ..Default::default()
        };
        let arrangements = Modifiers {
            shift: true,
            ..positioning
        };
        let mut bindings = vec![];
        for (keys, action, modifiers) in [
            (
                &[48][..],
                Action::Next,
                Modifiers {
                    command: true,
                    ..Default::default()
                },
            ),
            (
                &[50][..],
                Action::Previous,
                Modifiers {
                    command: true,
                    ..Default::default()
                },
            ),
            (&[126, 40][..], Action::Previous, base),
            (&[125, 38][..], Action::Next, base),
            (&[123, 4][..], Action::AppPrevious, base),
            (&[124, 37][..], Action::AppNext, base),
            (&[126, 40][..], Action::StackUp, navigation),
            (&[125, 38][..], Action::StackDown, navigation),
            (&[123, 4][..], Action::SpacePrevious, navigation),
            (&[124, 37][..], Action::SpaceNext, navigation),
            (&[123, 4][..], Action::SnapLeft, positioning),
            (&[124, 37][..], Action::SnapRight, positioning),
            (&[126, 40][..], Action::SnapTop, positioning),
            (&[125, 38][..], Action::SnapBottom, positioning),
            (&[46][..], Action::SnapFull, positioning),
            (&[34][..], Action::Center, positioning),
            (&[45][..], Action::Restore, positioning),
            (&[123, 4][..], Action::ArrangeLeftRight, arrangements),
            (&[124, 37][..], Action::ArrangeRightLeft, arrangements),
            (&[126, 40][..], Action::ArrangeTopBottom, arrangements),
            (&[125, 38][..], Action::ArrangeBottomTop, arrangements),
        ] {
            bindings.extend(keys.iter().map(|&keycode| Binding {
                keycode,
                action,
                modifiers,
            }));
        }
        Self {
            gap: 8.,
            overlay: true,
            overlay_brightness: 0.10,
            overlay_delay_ms: 120,
            overlay_modifiers: base,
            magnetic: false,
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
        .join("Library/Application Support/Whalu")
}

/// Import only missing files; retain the former app's data for rollback.
/// Explicit configuration paths and read-only CLI commands do not migrate data.
pub fn migrate_legacy_data() -> Result<(), String> {
    let destination = config_dir();
    let legacy = destination.parent().unwrap().join("Magnetic");
    migrate_files(&legacy, &destination).map_err(|error| {
        format!(
            "Cannot import Magnetic settings into {}: {error}",
            destination.display()
        )
    })
}

fn migrate_files(legacy: &std::path::Path, destination: &std::path::Path) -> std::io::Result<()> {
    for name in ["config.toml", "state.json"] {
        let source = legacy.join(name);
        let target = destination.join(name);
        if target.try_exists()? || !source.try_exists()? {
            continue;
        }
        std::fs::create_dir_all(destination)?;
        std::fs::copy(source, target)?;
    }
    Ok(())
}

#[cfg(test)]
mod migration_tests {
    use super::migrate_files;

    #[test]
    fn rebrand_imports_settings_and_recovery_without_overwriting_either_app() {
        let root = std::env::temp_dir().join(format!("whalu-migration-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let legacy = root.join("Magnetic");
        let destination = root.join("Whalu");

        migrate_files(&legacy, &destination).unwrap();
        assert!(!destination.exists());

        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::write(legacy.join("config.toml"), "gap = 12.0\n").unwrap();
        std::fs::write(legacy.join("state.json"), r#"{"version":1,"windows":[]}"#).unwrap();
        std::fs::write(legacy.join("instance.lock"), "legacy lock").unwrap();
        migrate_files(&legacy, &destination).unwrap();
        for name in ["config.toml", "state.json"] {
            assert_eq!(
                std::fs::read(legacy.join(name)).unwrap(),
                std::fs::read(destination.join(name)).unwrap()
            );
        }
        assert!(!destination.join("instance.lock").exists());

        std::fs::write(destination.join("config.toml"), "gap = 4.0\n").unwrap();
        std::fs::remove_file(destination.join("state.json")).unwrap();
        migrate_files(&legacy, &destination).unwrap();
        assert_eq!(
            std::fs::read_to_string(destination.join("config.toml")).unwrap(),
            "gap = 4.0\n"
        );
        assert_eq!(
            std::fs::read_to_string(legacy.join("config.toml")).unwrap(),
            "gap = 12.0\n"
        );
        assert!(destination.join("state.json").exists());
        std::fs::remove_dir_all(root).unwrap();
    }
}
