use chrono::{DateTime, Local, NaiveDateTime, TimeZone};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

fn default_uuid() -> String {
    Uuid::new_v4().to_string()
}

fn default_rest_seconds() -> i64 {
    30
}

fn default_sets() -> i64 {
    1
}

fn default_round_number() -> i64 {
    1
}

fn default_set_number() -> i64 {
    1
}

fn default_completed() -> bool {
    true
}

fn default_rest_seconds_0() -> i64 {
    0
}

fn now() -> DateTime<Local> {
    Local::now()
}

fn parse_iso_datetime(s: &str) -> Option<DateTime<Local>> {
    let naive_parse = |fmt: &str| -> Option<DateTime<Local>> {
        NaiveDateTime::parse_from_str(s, fmt)
            .ok()
            .map(|ndt| Local.from_local_datetime(&ndt).single())
            .flatten()
    };
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|d| d.with_timezone(&Local))
        .or_else(|| naive_parse("%Y-%m-%dT%H:%M:%S%.f"))
        .or_else(|| naive_parse("%Y-%m-%dT%H:%M:%S"))
}

/// A user-saved round-timer preset (name + full timer configuration).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimerPresetEntry {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub config: RoundConfig,
}

/// One round of a custom interval list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoundSpec {
    #[serde(default = "default_round_seconds")]
    pub work_seconds: i64,
    #[serde(default)]
    pub rest_seconds: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoundConfig {
    #[serde(default = "default_rounds")]
    pub rounds: i64,
    #[serde(default = "default_round_seconds")]
    pub round_seconds: i64,
    #[serde(default = "default_pause_seconds")]
    pub pause_seconds: i64,
    /// Countdown before the first round (0 = off).
    #[serde(default)]
    pub prep_seconds: i64,
    /// One extra warm-up round at the pause length.
    #[serde(default)]
    pub warmup: bool,
    /// Custom per-round work/rest; when set it overrides rounds/round_seconds.
    #[serde(default)]
    pub per_round: Option<Vec<RoundSpec>>,
    /// Free-text title, e.g. "Chores" or "Stretching".
    #[serde(default)]
    pub title: String,
}

fn default_rounds() -> i64 {
    10
}
fn default_round_seconds() -> i64 {
    180
}
fn default_pause_seconds() -> i64 {
    60
}

impl Default for RoundConfig {
    fn default() -> Self {
        Self {
            rounds: 10,
            round_seconds: 180,
            pause_seconds: 60,
            prep_seconds: 0,
            warmup: false,
            per_round: None,
            title: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Exercise {
    #[serde(default)]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reps: Option<i64>,
    #[serde(default = "default_rest_seconds")]
    pub rest_seconds: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight_kg: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_path: Option<String>,
    #[serde(default = "default_sets")]
    pub sets: i64,
}

impl Exercise {
    pub fn is_timed(&self) -> bool {
        self.duration_seconds.map(|d| d > 0).unwrap_or(false)
    }
}

impl Default for Exercise {
    fn default() -> Self {
        Self {
            name: String::new(),
            duration_seconds: None,
            reps: None,
            rest_seconds: 30,
            weight_kg: None,
            image_path: None,
            sets: 1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingPlan {
    #[serde(default = "default_uuid")]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub exercises: Vec<Exercise>,
    #[serde(default = "default_sets")]
    pub total_rounds: i64,
    #[serde(default = "default_rest_between_rounds")]
    pub rest_between_rounds_seconds: i64,
    #[serde(default = "now", serialize_with = "serialize_datetime", deserialize_with = "deserialize_datetime")]
    pub created: DateTime<Local>,
    #[serde(default)]
    pub is_favorite: bool,
}

fn default_rest_between_rounds() -> i64 {
    60
}

fn serialize_datetime<S>(dt: &DateTime<Local>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    // Python datetime.isoformat() — naive local time, keeps files
    // interchangeable between the Python and Rust versions.
    serializer.serialize_str(&dt.format("%Y-%m-%dT%H:%M:%S%.6f").to_string())
}

fn deserialize_datetime<'de, D>(deserializer: D) -> Result<DateTime<Local>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Ok(parse_iso_datetime(&s).unwrap_or_else(Local::now))
}

impl TrainingPlan {
    pub fn total_planned_seconds(&self) -> i64 {
        let mut total: i64 = 0;
        for ex in &self.exercises {
            let sets = ex.sets.max(1);
            if let Some(d) = ex.duration_seconds {
                if d > 0 {
                    total += d * sets;
                }
            }
            total += ex.rest_seconds * sets;
        }
        total *= self.total_rounds.max(1);
        if self.total_rounds > 1 && self.rest_between_rounds_seconds > 0 {
            total += self.rest_between_rounds_seconds * (self.total_rounds - 1);
        }
        total
    }
}

impl Default for TrainingPlan {
    fn default() -> Self {
        Self {
            id: default_uuid(),
            name: String::new(),
            exercises: Vec::new(),
            total_rounds: 1,
            rest_between_rounds_seconds: 60,
            created: Local::now(),
            is_favorite: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExerciseLog {
    #[serde(default)]
    pub exercise_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub planned_duration_seconds: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual_duration_seconds: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub planned_reps: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual_reps: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub planned_weight_kg: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual_weight_kg: Option<f64>,
    #[serde(default = "default_rest_seconds_0")]
    pub rest_seconds: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual_rest_seconds: Option<i64>,
    #[serde(default = "default_completed")]
    pub completed: bool,
    #[serde(default = "default_round_number")]
    pub round_number: i64,
    #[serde(default, skip_serializing_if = "Option::is_none", serialize_with = "serialize_opt_datetime", deserialize_with = "deserialize_opt_datetime")]
    pub started_at: Option<DateTime<Local>>,
    #[serde(default, skip_serializing_if = "Option::is_none", serialize_with = "serialize_opt_datetime", deserialize_with = "deserialize_opt_datetime")]
    pub finished_at: Option<DateTime<Local>>,
    #[serde(default = "default_set_number")]
    pub set_number: i64,
}

fn serialize_opt_datetime<S>(dt: &Option<DateTime<Local>>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    match dt {
        Some(d) => serializer.serialize_str(&d.format("%Y-%m-%dT%H:%M:%S%.6f").to_string()),
        None => serializer.serialize_none(),
    }
}

fn deserialize_opt_datetime<'de, D>(deserializer: D) -> Result<Option<DateTime<Local>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt = Option::<String>::deserialize(deserializer)?;
    Ok(opt.and_then(|s| parse_iso_datetime(&s)))
}

impl Default for ExerciseLog {
    fn default() -> Self {
        Self {
            exercise_name: String::new(),
            planned_duration_seconds: None,
            actual_duration_seconds: None,
            planned_reps: None,
            actual_reps: None,
            planned_weight_kg: None,
            actual_weight_kg: None,
            rest_seconds: 0,
            actual_rest_seconds: None,
            completed: true,
            round_number: 1,
            started_at: None,
            finished_at: None,
            set_number: 1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingSession {
    #[serde(default = "default_uuid")]
    pub id: String,
    #[serde(default)]
    pub plan_id: String,
    #[serde(default)]
    pub plan_name: String,
    #[serde(default = "now", serialize_with = "serialize_datetime", deserialize_with = "deserialize_datetime")]
    pub started_at: DateTime<Local>,
    #[serde(default, skip_serializing_if = "Option::is_none", serialize_with = "serialize_opt_datetime", deserialize_with = "deserialize_opt_datetime")]
    pub finished_at: Option<DateTime<Local>>,
    #[serde(default)]
    pub total_planned_seconds: i64,
    #[serde(default)]
    pub total_actual_seconds: i64,
    #[serde(default = "default_sets")]
    pub total_rounds: i64,
    #[serde(default)]
    pub rest_between_rounds_seconds: i64,
    #[serde(default)]
    pub exercises: Vec<ExerciseLog>,
}

impl TrainingSession {
    pub fn compute_total_actual_seconds(&self) -> i64 {
        let mut total: i64 = 0;
        for ex in &self.exercises {
            if let Some(d) = ex.actual_duration_seconds {
                if d > 0 {
                    total += d;
                }
            }
            if let Some(r) = ex.actual_rest_seconds {
                total += r;
            } else if ex.completed {
                total += ex.rest_seconds;
            }
        }
        total
    }
}

impl Default for TrainingSession {
    fn default() -> Self {
        Self {
            id: default_uuid(),
            plan_id: String::new(),
            plan_name: String::new(),
            started_at: Local::now(),
            finished_at: None,
            total_planned_seconds: 0,
            total_actual_seconds: 0,
            total_rounds: 1,
            rest_between_rounds_seconds: 0,
            exercises: Vec::new(),
        }
    }
}
