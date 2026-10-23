//! Recovery stores metadata, never reapplies saved geometry on launch.
use crate::core::{Engine, Mode, Window};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex},
    thread::{self, JoinHandle},
};
#[derive(Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedState {
    pub version: u32,
    pub windows: Vec<SavedWindow>,
}
#[derive(PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedWindow {
    pub app_id: String,
    pub title: String,
    pub mode: Mode,
}

#[derive(Default)]
struct PendingSave {
    state: Option<SavedState>,
    stopping: bool,
}

/// A single writer with one pending snapshot. Bursts replace pending work;
/// shutdown waits for the latest snapshot to reach the atomic state file.
pub struct StateWriter {
    pending: Arc<(Mutex<PendingSave>, Condvar)>,
    worker: Option<JoinHandle<()>>,
}
impl StateWriter {
    pub fn new(path: PathBuf) -> std::io::Result<Self> {
        let pending = Arc::new((Mutex::new(PendingSave::default()), Condvar::new()));
        let shared = Arc::clone(&pending);
        let worker = thread::Builder::new()
            .name("magnetic-state".into())
            .spawn(move || {
                let (lock, wake) = &*shared;
                let mut saved = None;
                loop {
                    let state = {
                        let mut pending = lock.lock().unwrap();
                        while pending.state.is_none() && !pending.stopping {
                            pending = wake.wait(pending).unwrap();
                        }
                        match pending.state.take() {
                            Some(state) => state,
                            None => break,
                        }
                    };
                    if saved.as_ref() == Some(&state) {
                        continue;
                    }
                    match state.write(&path) {
                        Ok(()) => saved = Some(state),
                        Err(error) => eprintln!("Recovery write failed: {error}"),
                    }
                }
            })?;
        Ok(Self {
            pending,
            worker: Some(worker),
        })
    }

    pub fn save(&self, state: SavedState) {
        let (lock, wake) = &*self.pending;
        lock.lock().unwrap().state = Some(state);
        wake.notify_one();
    }
}
impl Drop for StateWriter {
    fn drop(&mut self) {
        let (lock, wake) = &*self.pending;
        lock.lock().unwrap().stopping = true;
        wake.notify_one();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writer_flushes_the_latest_snapshot_on_shutdown() {
        let dir = std::env::temp_dir().join(format!("magnetic-state-test-{}", std::process::id()));
        let path = dir.join("state.json");
        let writer = StateWriter::new(path.clone()).unwrap();
        for index in 0..100 {
            writer.save(SavedState {
                version: 1,
                windows: vec![SavedWindow {
                    app_id: "test.app".into(),
                    title: format!("Window {index}"),
                    mode: Mode::Floating,
                }],
            });
        }
        drop(writer);
        let saved = SavedState::load(&path);
        assert_eq!(saved.windows.len(), 1);
        assert_eq!(saved.windows[0].title, "Window 99");
        assert_eq!(saved.windows[0].mode, Mode::Floating);
        assert!(!path.with_extension("json.tmp").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
