use crate::data_store::DataStore;
use crate::image_utils::UserExercise;
use crate::models::{TrainingPlan, TrainingSession};
use crate::settings::{load_settings, save_settings, AppSettings};
use crate::user_exercises::{load_user_exercises, save_user_exercises};
use serde::{Deserialize, Serialize};
use std::rc::Rc;

const BACKUP_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct BackupFile {    version: u32,
    plans: Vec<TrainingPlan>,
    sessions: Vec<TrainingSession>,
    settings: AppSettings,
    user_exercises: Vec<UserExercise>,
}

pub struct BackupSummary {
    pub plans_added: usize,
    pub sessions_added: usize,
    pub user_exercises_added: usize,
    #[allow(dead_code)]
    pub settings_restored: bool,
}

/// Export plans, history, settings and user exercises into one JSON bundle.
pub fn export_backup(store: &Rc<DataStore>, filepath: &str) -> std::io::Result<()> {
    let backup = BackupFile {
        version: BACKUP_VERSION,
        plans: store.load_plans(),
        sessions: store.load_sessions(),
        settings: load_settings(),
        user_exercises: load_user_exercises(),
    };
    let json = serde_json::to_string_pretty(&backup)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    crate::data_store::atomic_write(std::path::Path::new(filepath), json.as_bytes())
}

/// Restore a backup bundle. Plans and sessions are merged by id (existing
/// entries are kept), settings are replaced, user exercises are merged by
/// name.
pub fn import_backup(store: &Rc<DataStore>, filepath: &str) -> Result<BackupSummary, String> {
    let content = std::fs::read_to_string(filepath)
        .map_err(|e| format!("Could not read file: {}", e))?;
    let backup: BackupFile =
        serde_json::from_str(&content).map_err(|e| format!("Invalid backup file: {}", e))?;

    let existing_plan_ids: Vec<String> = store
        .load_plans()
        .iter()
        .map(|p| p.id.clone())
        .collect();
    let new_plans: Vec<TrainingPlan> = backup
        .plans
        .into_iter()
        .filter(|p| !existing_plan_ids.contains(&p.id))
        .collect();
    let plans_added = new_plans.len();
    if !new_plans.is_empty() {
        let mut plans = store.load_plans();
        plans.extend(new_plans);
        store.save_plans(&plans);
    }

    let existing_session_ids: Vec<String> = store
        .load_sessions()
        .iter()
        .map(|s| s.id.clone())
        .collect();
    let new_sessions: Vec<TrainingSession> = backup
        .sessions
        .into_iter()
        .filter(|s| !existing_session_ids.contains(&s.id))
        .collect();
    let sessions_added = new_sessions.len();
    if !new_sessions.is_empty() {
        let mut sessions = store.load_sessions();
        sessions.extend(new_sessions);
        store.save_sessions(&sessions);
    }

    save_settings(&backup.settings);

    let existing_names: Vec<String> = load_user_exercises()
        .iter()
        .map(|e| e.name.clone())
        .collect();
    let new_exercises: Vec<UserExercise> = backup
        .user_exercises
        .into_iter()
        .filter(|e| !existing_names.contains(&e.name))
        .collect();
    let user_exercises_added = new_exercises.len();
    if !new_exercises.is_empty() {
        let mut exercises = load_user_exercises();
        exercises.extend(new_exercises);
        save_user_exercises(&exercises);
    }

    Ok(BackupSummary {
        plans_added,
        sessions_added,
        user_exercises_added,
        settings_restored: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backup_format_round_trip() {
        let backup = BackupFile {
            version: BACKUP_VERSION,
            plans: vec![],
            sessions: vec![],
            settings: AppSettings::default(),
            user_exercises: vec![],
        };
        let json = serde_json::to_string_pretty(&backup).unwrap();
        let parsed: BackupFile = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.version, BACKUP_VERSION);

        // An old/foreign file must produce a readable error, not a panic
        assert!(serde_json::from_str::<BackupFile>("{\"version\": 999}").is_err());
    }
}
