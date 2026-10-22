//! Recovery stores metadata, never reapplies saved geometry on launch.
use crate::core::{Engine, Mode, Window};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};
#[derive(Default, Serialize, Deserialize)]
pub struct SavedState {
    pub version: u32,
    pub windows: Vec<SavedWindow>,
}
#[derive(Serialize, Deserialize)]
pub struct SavedWindow {
    pub app_id: String,
    pub title: String,
    pub mode: Mode,
}
impl SavedState {
    pub fn capture(engine: &Engine) -> Self {
        Self {
            version: 1,
            windows: engine
                .windows
                .values()
                .map(|w| SavedWindow {
                    app_id: w.app_id.clone(),
                    title: w.title.clone(),
                    mode: w.mode,
                })
                .collect(),
        }
    }
    pub fn load(path: &Path) -> Self {
        let Ok(data) = std::fs::read(path) else {
            return Self::default();
        };
        match serde_json::from_slice::<Self>(&data) {
            Ok(s) if s.version == 1 => s,
            _ => {
                eprintln!(
                    "Ignoring invalid or unsupported recovery state: {}",
                    path.display()
                );
                Self::default()
            }
        }
    }
    /// Exact, unique identity only. Ambiguous/untitled windows use safe defaults.
    pub fn restore_modes(&self, windows: &mut BTreeMap<u64, Window>) {
        let mut counts = BTreeMap::new();
        for w in windows.values() {
            *counts
                .entry((w.app_id.clone(), w.title.clone()))
                .or_insert(0) += 1;
        }
        for w in windows.values_mut() {
            if w.title.is_empty() || counts.get(&(w.app_id.clone(), w.title.clone())) != Some(&1) {
                continue;
            }
            let mut saved = self
                .windows
                .iter()
                .filter(|s| s.app_id == w.app_id && s.title == w.title);
            if let (Some(s), None) = (saved.next(), saved.next()) {
                w.mode = s.mode;
                if w.mode == Mode::Floating {
                    w.last_floating_frame = Some(w.frame);
                }
            }
        }
    }

    pub fn write(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_vec_pretty(self)?;
        let tmp = path.with_extension("json.tmp");
        {
            use std::io::Write;
            let mut f = std::fs::File::create(&tmp)?;
            f.write_all(&data)?;
            f.sync_all()?;
        }
        std::fs::rename(tmp, path)?;
        Ok(())
    }
}
