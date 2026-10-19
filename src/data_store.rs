use crate::models::{TrainingPlan, TrainingSession};
use gtk4::glib;
use std::fs;
use std::path::{Path, PathBuf};

fn data_dir() -> PathBuf {
    let dir = glib::user_data_dir().join("training-flatpak");
    let _ = fs::create_dir_all(&dir);
    dir
}

fn plans_path() -> PathBuf {
    data_dir().join("plans.json")
}

fn history_path() -> PathBuf {
    data_dir().join("history.json")
}

fn read_json_file<T: serde::de::DeserializeOwned>(path: &PathBuf) -> Vec<T> {
    if !path.exists() {
        return Vec::new();
    }
    match fs::read_to_string(path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

/// Write `data` to `path` atomically: write to a temp file in the same
/// directory, then rename over the target. A crash mid-write can no longer
/// leave a truncated/corrupt data file behind.
pub fn atomic_write(path: &Path, data: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".to_string());
    let tmp = path.with_file_name(format!(".{}.tmp", file_name));
    fs::write(&tmp, data)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

fn write_json_file<T: serde::Serialize>(path: &PathBuf, data: &[T]) {
    if let Ok(json) = serde_json::to_string_pretty(data) {
        let _ = atomic_write(path, json.as_bytes());
    }
}

pub struct DataStore;

impl DataStore {
    pub fn new() -> Self {
        Self
    }

    pub fn load_plans(&self) -> Vec<TrainingPlan> {
        read_json_file(&plans_path())
    }

    pub fn save_plans(&self, plans: &[TrainingPlan]) {
        write_json_file(&plans_path(), plans);
    }

    pub fn save_plan(&self, plan: &TrainingPlan) {
        let mut plans = self.load_plans();
        if let Some(p) = plans.iter_mut().find(|p| p.id == plan.id) {
            *p = plan.clone();
        } else {
            plans.push(plan.clone());
        }
        self.save_plans(&plans);
    }

    pub fn delete_plan(&self, plan_id: &str) {
        let mut plans = self.load_plans();
        plans.retain(|p| p.id != plan_id);
        self.save_plans(&plans);
    }

    pub fn get_plan(&self, plan_id: &str) -> Option<TrainingPlan> {
        self.load_plans().into_iter().find(|p| p.id == plan_id)
    }

    pub fn load_sessions(&self) -> Vec<TrainingSession> {
        let mut sessions: Vec<TrainingSession> = read_json_file(&history_path());
        sessions.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        sessions
    }

    pub fn save_sessions(&self, sessions: &[TrainingSession]) {
        write_json_file(&history_path(), sessions);
    }

    pub fn save_session(&self, session: &TrainingSession) {
        let mut sessions = self.load_sessions();
        if let Some(s) = sessions.iter_mut().find(|s| s.id == session.id) {
            *s = session.clone();
        } else {
            sessions.push(session.clone());
        }
        self.save_sessions(&sessions);
    }

    #[allow(dead_code)]
    pub fn get_session(&self, session_id: &str) -> Option<TrainingSession> {
        self.load_sessions().into_iter().find(|s| s.id == session_id)
    }

    #[allow(dead_code)]
    pub fn get_sessions_for_plan(&self, plan_id: &str) -> Vec<TrainingSession> {
        self.load_sessions()
            .into_iter()
            .filter(|s| s.plan_id == plan_id)
            .collect()
    }

    pub fn delete_session(&self, session_id: &str) {
        let mut sessions = self.load_sessions();
        sessions.retain(|s| s.id != session_id);
        self.save_sessions(&sessions);
    }

    pub fn seed_default_plans(&self) {
        let existing = self.load_plans();
        if !existing.is_empty() {
            return;
        }
        let plans_dir = crate::image_utils::get_plans_dir();
        let path = PathBuf::from(&plans_dir).join("default_plans.json");
        if !path.exists() {
            return;
        }
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(plans) = serde_json::from_str::<Vec<TrainingPlan>>(&content) {
                self.save_plans(&plans);
            }
        }
    }
}

impl Default for DataStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_write_replaces_content_and_leaves_no_tmp() {
        let dir = std::env::temp_dir().join(format!("training-atomic-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("plans.json");

        atomic_write(&path, b"[1]").unwrap();
        atomic_write(&path, b"[1,2]").unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "[1,2]");
        // No leftover temp file
        let leftovers: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains("tmp"))
            .collect();
        assert!(leftovers.is_empty(), "temp file left behind: {:?}", leftovers);

        let _ = fs::remove_dir_all(&dir);
    }
}
