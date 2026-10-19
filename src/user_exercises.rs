use crate::image_utils::UserExercise;
use gtk4::glib;
use std::fs;
use std::path::PathBuf;

fn user_exercises_path() -> PathBuf {
    glib::user_data_dir()
        .join("training-flatpak")
        .join("user_exercises.json")
}

pub fn load_user_exercises() -> Vec<UserExercise> {
    let path = user_exercises_path();
    if !path.exists() {
        return Vec::new();
    }
    match fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

pub fn save_user_exercises(exercises: &[UserExercise]) {
    let path = user_exercises_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(exercises) {
        let _ = crate::data_store::atomic_write(&path, json.as_bytes());
    }
}

pub fn add_user_exercise(
    name: &str,
    primary_muscles: Vec<String>,
    equipment: &str,
    image_path: Option<String>,
) -> UserExercise {
    let mut exercises = load_user_exercises();
    let entry = UserExercise {
        name: name.to_string(),
        primary_muscles,
        equipment: Some(equipment.to_string()),
        image_path,
        source: Some("user".to_string()),
    };
    exercises.push(entry.clone());
    save_user_exercises(&exercises);
    entry
}

/// Remove the user exercise at `index` and return it, so the caller can offer
/// an "Undo" that re-inserts it at the same position.
pub fn delete_user_exercise(index: usize) -> Option<UserExercise> {
    let mut exercises = load_user_exercises();
    if index >= exercises.len() {
        return None;
    }
    let removed = exercises.remove(index);
    save_user_exercises(&exercises);
    Some(removed)
}

/// Re-insert a previously removed user exercise at its original position.
pub fn insert_user_exercise(index: usize, exercise: &UserExercise) {
    let mut exercises = load_user_exercises();
    let index = index.min(exercises.len());
    exercises.insert(index, exercise.clone());
    save_user_exercises(&exercises);
}
