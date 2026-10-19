use gtk4::glib;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

fn bundled_exercises_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("resources")
        .join("exercises")
}

fn flatpak_exercises_dir() -> PathBuf {
    PathBuf::from("/app/share/training/resources/exercises")
}

pub fn get_exercises_dir() -> PathBuf {
    let flatpak = flatpak_exercises_dir();
    if flatpak.exists() {
        flatpak
    } else {
        bundled_exercises_dir()
    }
}

pub fn get_user_images_dir() -> PathBuf {
    let dir = glib::user_data_dir()
        .join("training-flatpak")
        .join("images");
    let _ = fs::create_dir_all(&dir);
    dir
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct BundledExercise {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub force: Option<String>,
    #[serde(default)]
    pub level: Option<String>,
    #[serde(default)]
    pub mechanic: Option<String>,
    #[serde(default)]
    pub equipment: Option<String>,
    #[serde(default, rename = "primaryMuscles")]
    pub primary_muscles: Vec<String>,
    #[serde(default, rename = "secondaryMuscles")]
    pub secondary_muscles: Vec<String>,
    #[serde(default)]
    pub instructions: Vec<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub images: Vec<String>,
    #[serde(default)]
    pub id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct UserExercise {
    #[serde(default)]
    pub name: String,
    #[serde(default, rename = "primaryMuscles")]
    pub primary_muscles: Vec<String>,
    #[serde(default)]
    pub equipment: Option<String>,
    #[serde(default)]
    pub image_path: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ExerciseEntry {
    pub name: String,
    pub primary_muscles: Vec<String>,
    pub equipment: String,
    pub category: Option<String>,
    pub image_key: Option<String>,
    pub is_user: bool,
}

static BUNDLED_CACHE: std::sync::OnceLock<Vec<BundledExercise>> = std::sync::OnceLock::new();

pub fn load_bundled_exercises() -> &'static Vec<BundledExercise> {
    BUNDLED_CACHE.get_or_init(|| {
        let path = get_exercises_dir().join("exercises.json");
        if !path.exists() {
            return Vec::new();
        }
        match fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => Vec::new(),
        }
    })
}

pub fn resolve_image_path(image_path: &Option<String>) -> Option<PathBuf> {
    let image_path = image_path.as_ref()?;

    if let Some(key) = image_path.strip_prefix("bundled:") {
        let img_path = get_exercises_dir()
            .join("images")
            .join(key)
            .join("0.jpg");
        if img_path.exists() {
            return Some(img_path);
        }
        return None;
    }

    if let Some(filename) = image_path.strip_prefix("user:") {
        let user_path = get_user_images_dir().join(filename);
        if user_path.exists() {
            return Some(user_path);
        }
        return None;
    }

    let path = PathBuf::from(image_path);
    if path.is_absolute() && path.exists() {
        return Some(path);
    }

    None
}

pub fn copy_user_image(source_path: &str) -> Option<String> {
    let source = PathBuf::from(source_path);
    if !source.exists() {
        return None;
    }
    let filename = source.file_name()?.to_str()?;
    let dir = get_user_images_dir();
    let mut dest = dir.join(filename);
    let stem = source.file_stem()?.to_str()?;
    let ext = source.extension()?.to_str()?;
    let mut counter = 1;
    while dest.exists() {
        dest = dir.join(format!("{}_{}.{}", stem, counter, ext));
        counter += 1;
    }
    fs::copy(&source, &dest).ok()?;
    Some(format!("user:{}", dest.file_name()?.to_str()?))
}

pub fn load_all_exercises() -> Vec<ExerciseEntry> {
    let mut entries = Vec::new();

    for ex in load_bundled_exercises() {
        let image_key = ex
            .images
            .first()
            .map(|img| format!("bundled:{}", img.split('/').next().unwrap_or("")));
        entries.push(ExerciseEntry {
            name: ex.name.clone(),
            primary_muscles: ex.primary_muscles.clone(),
            equipment: ex.equipment.clone().unwrap_or_default(),
            category: ex.category.clone(),
            image_key,
            is_user: false,
        });
    }

    for ex in crate::user_exercises::load_user_exercises() {
        entries.push(ExerciseEntry {
            name: ex.name.clone(),
            primary_muscles: ex.primary_muscles.clone(),
            equipment: ex.equipment.clone().unwrap_or_default(),
            category: None,
            image_key: ex.image_path.clone(),
            is_user: true,
        });
    }

    entries
}

pub fn get_plans_dir() -> String {
    let flatpak = PathBuf::from("/app/share/training/resources/plans");
    if flatpak.exists() {
        return flatpak.to_string_lossy().to_string();
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("resources")
        .join("plans")
        .to_string_lossy()
        .to_string()
}
