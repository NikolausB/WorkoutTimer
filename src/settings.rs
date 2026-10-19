use gtk4::glib;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

fn settings_dir() -> PathBuf {
    let data_dir = glib::user_data_dir();
    data_dir.join("training-flatpak")
}

fn settings_path() -> PathBuf {
    settings_dir().join("settings.json")
}

#[allow(dead_code)]
pub const AVAILABLE_SOUNDS: &[&str] = &[
    "beep",
    "round_start",
    "round_end",
    "exercise_complete",
    "training_complete",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    #[serde(default = "default_true")]
    pub sound_enabled: bool,
    #[serde(default = "default_round_start_sound")]
    pub round_start_sound: String,
    #[serde(default = "default_round_end_sound")]
    pub round_end_sound: String,
    #[serde(default = "default_countdown_tick_sound")]
    pub countdown_tick_sound: String,
    #[serde(default = "default_exercise_complete_sound")]
    pub exercise_complete_sound: String,
    #[serde(default = "default_training_complete_sound")]
    pub training_complete_sound: String,
    #[serde(default = "default_true")]
    pub show_home_page: bool,
    #[serde(default = "default_true")]
    pub show_timer_page: bool,
    #[serde(default = "default_true")]
    pub show_workout_page: bool,
    #[serde(default)]
    pub show_ai_page: bool,
    #[serde(default = "default_ai_provider")]
    pub ai_provider: String,
    #[serde(default = "default_ai_model")]
    pub ai_model: String,
    #[serde(default = "default_ai_base_url")]
    pub ai_base_url: String,
    #[serde(default)]
    pub ai_api_key: String,
    #[serde(default = "default_true")]
    pub ai_include_history: bool,
    #[serde(default)]
    pub ai_system_prompt: String,
    #[serde(default = "default_true")]
    pub gamepad_enabled: bool,
    #[serde(default = "default_true")]
    pub gamepad_hints: bool,
    #[serde(default = "default_deck_mode")]
    pub deck_mode: String,
    #[serde(default = "default_color_scheme")]
    pub color_scheme: String,
    #[serde(default = "default_sound_volume")]
    pub sound_volume: f64,
    #[serde(default)]
    pub last_tab: String,
    #[serde(default)]
    pub window_width: i32,
    #[serde(default)]
    pub window_height: i32,
}

fn default_true() -> bool {
    true
}
fn default_round_start_sound() -> String {
    "round_start".to_string()
}
fn default_round_end_sound() -> String {
    "round_end".to_string()
}
fn default_countdown_tick_sound() -> String {
    "beep".to_string()
}
fn default_exercise_complete_sound() -> String {
    "exercise_complete".to_string()
}
fn default_training_complete_sound() -> String {
    "training_complete".to_string()
}
fn default_ai_provider() -> String {
    "ollama".to_string()
}
fn default_ai_model() -> String {
    "llama3.2".to_string()
}
fn default_ai_base_url() -> String {
    "http://localhost:11434/v1".to_string()
}
fn default_deck_mode() -> String {
    "auto".to_string()
}
fn default_color_scheme() -> String {
    "default".to_string()
}
fn default_sound_volume() -> f64 {
    1.0
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            sound_enabled: true,
            round_start_sound: "round_start".to_string(),
            round_end_sound: "round_end".to_string(),
            countdown_tick_sound: "beep".to_string(),
            exercise_complete_sound: "exercise_complete".to_string(),
            training_complete_sound: "training_complete".to_string(),
            show_home_page: true,
            show_timer_page: true,
            show_workout_page: true,
            show_ai_page: false,
            ai_provider: "ollama".to_string(),
            ai_model: "llama3.2".to_string(),
            ai_base_url: "http://localhost:11434/v1".to_string(),
            ai_api_key: String::new(),
            ai_include_history: true,
            ai_system_prompt: String::new(),
            gamepad_enabled: true,
            gamepad_hints: true,
            deck_mode: "auto".to_string(),
            color_scheme: "default".to_string(),
            sound_volume: 1.0,
            last_tab: String::new(),
            window_width: 0,
            window_height: 0,
        }
    }
}

impl AppSettings {
    pub fn get_sound(&self, event_key: &str) -> Option<String> {
        if !self.sound_enabled {
            return None;
        }
        let name = match event_key {
            "round_start_sound" => &self.round_start_sound,
            "round_end_sound" => &self.round_end_sound,
            "countdown_tick_sound" => &self.countdown_tick_sound,
            "exercise_complete_sound" => &self.exercise_complete_sound,
            "training_complete_sound" => &self.training_complete_sound,
            _ => return None,
        };
        if name == "none" || name.is_empty() {
            None
        } else {
            Some(name.clone())
        }
    }
}

pub fn load_settings() -> AppSettings {
    let path = settings_path();
    if !path.exists() {
        return AppSettings::default();
    }
    match fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => AppSettings::default(),
    }
}

pub fn save_settings(settings: &AppSettings) {
    let dir = settings_dir();
    let _ = fs::create_dir_all(&dir);
    let path = settings_path();
    if let Ok(json) = serde_json::to_string_pretty(settings) {
        let _ = crate::data_store::atomic_write(&path, json.as_bytes());
    }
}
