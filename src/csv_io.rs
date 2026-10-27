use crate::models::{ExerciseLog, TrainingPlan, TrainingSession};
use chrono::{DateTime, Local, NaiveDateTime, TimeZone};
use std::collections::HashMap;
use std::fs;

pub const CSV_HEADER: &[&str] = &[
    "session_id",
    "plan_name",
    "started_at",
    "finished_at",
    "total_planned_seconds",
    "total_actual_seconds",
    "total_rounds",
    "rest_between_rounds_seconds",
    "exercise_name",
    "planned_duration_seconds",
    "actual_duration_seconds",
    "planned_reps",
    "actual_reps",
    "planned_weight_kg",
    "actual_weight_kg",
    "rest_seconds",
    "actual_rest_seconds",
    "completed",
    "round_number",
    "exercise_started_at",
    "exercise_finished_at",
    "set_number",
];

fn parse_datetime(s: &str) -> Option<DateTime<Local>> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
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

fn fmt_int(val: Option<i64>) -> String {
    val.map(|v| v.to_string()).unwrap_or_default()
}

fn fmt_float(val: Option<f64>) -> String {
    val.map(|v| format!("{}", v)).unwrap_or_default()
}

fn blank_to_none_int(val: &str) -> Option<i64> {
    let v = val.trim();
    if v.is_empty() {
        None
    } else {
        v.parse().ok()
    }
}

fn blank_to_none_float(val: &str) -> Option<f64> {
    let v = val.trim();
    if v.is_empty() {
        None
    } else {
        v.parse().ok()
    }
}

fn parse_int(val: &str, default: i64) -> i64 {
    val.trim().parse().unwrap_or(default)
}

pub fn export_history_csv(sessions: &[TrainingSession], filepath: &str) -> std::io::Result<()> {
    let mut wtr = csv::Writer::from_path(filepath)?;
    wtr.write_record(CSV_HEADER)?;

    for session in sessions {
        for ex in &session.exercises {
            wtr.write_record(&[
                session.id.clone(),
                session.plan_name.clone(),
                session.started_at.format("%Y-%m-%dT%H:%M:%S%.6f").to_string(),
                session
                    .finished_at
                    .map(|d| d.format("%Y-%m-%dT%H:%M:%S%.6f").to_string())
                    .unwrap_or_default(),
                session.total_planned_seconds.to_string(),
                session.compute_total_actual_seconds().to_string(),
                session.total_rounds.to_string(),
                session.rest_between_rounds_seconds.to_string(),
                ex.exercise_name.clone(),
                fmt_int(ex.planned_duration_seconds),
                fmt_int(ex.actual_duration_seconds),
                fmt_int(ex.planned_reps),
                fmt_int(ex.actual_reps),
                fmt_float(ex.planned_weight_kg),
                fmt_float(ex.actual_weight_kg),
                ex.rest_seconds.to_string(),
                fmt_int(ex.actual_rest_seconds),
                if ex.completed { "True" } else { "False" }.to_string(),
                ex.round_number.to_string(),
                ex.started_at.map(|d| d.format("%Y-%m-%dT%H:%M:%S%.6f").to_string()).unwrap_or_default(),
                ex.finished_at.map(|d| d.format("%Y-%m-%dT%H:%M:%S%.6f").to_string()).unwrap_or_default(),
                ex.set_number.to_string(),
            ])?;
        }
    }

    wtr.flush()?;
    Ok(())
}

/// Import history from CSV. Returns the parsed sessions and the number of
/// rows that were skipped because they were unreadable or had no session id.
pub fn import_history_csv(filepath: &str) -> (Vec<TrainingSession>, usize) {
    if !std::path::Path::new(filepath).exists() {
        return (Vec::new(), 0);
    }

    let mut sessions_by_id: HashMap<String, SessionBuilder> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    let mut skipped_rows: usize = 0;

    let Ok(mut rdr) = csv::Reader::from_path(filepath) else {
        return (Vec::new(), 0);
    };

    for result in rdr.records() {
        let Ok(row) = result else {
            skipped_rows += 1;
            continue;
        };
        let get = |idx: usize| row.get(idx).unwrap_or("").to_string();

        let sid = get(0).trim().to_string();
        if sid.is_empty() {
            skipped_rows += 1;
            continue;
        }

        if !sessions_by_id.contains_key(&sid) {
            let started_str = get(2);
            let finished_str = get(3);
            sessions_by_id.insert(
                sid.clone(),
                SessionBuilder {
                    id: sid.clone(),
                    plan_name: get(1).trim().to_string(),
                    total_planned_seconds: parse_int(&get(4), 0),
                    total_actual_seconds: parse_int(&get(5), 0),
                    total_rounds: parse_int(&get(6), 1),
                    rest_between_rounds_seconds: parse_int(&get(7), 0),
                    started_at: parse_datetime(&started_str).unwrap_or_else(Local::now),
                    finished_at: parse_datetime(&finished_str),
                    exercises: Vec::new(),
                },
            );
            order.push(sid.clone());
        }

        let ex_log = ExerciseLog {
            exercise_name: get(8).trim().to_string(),
            planned_duration_seconds: blank_to_none_int(&get(9)),
            actual_duration_seconds: blank_to_none_int(&get(10)),
            planned_reps: blank_to_none_int(&get(11)),
            actual_reps: blank_to_none_int(&get(12)),
            planned_weight_kg: blank_to_none_float(&get(13)),
            actual_weight_kg: blank_to_none_float(&get(14)),
            rest_seconds: parse_int(&get(15), 0),
            actual_rest_seconds: blank_to_none_int(&get(16)),
            completed: matches!(
                get(17).trim().to_lowercase().as_str(),
                "true" | "1" | "yes" | "on"
            ),
            round_number: parse_int(&get(18), 1),
            started_at: parse_datetime(&get(19)),
            finished_at: parse_datetime(&get(20)),
            set_number: parse_int(&get(21), 1),
        };

        if let Some(builder) = sessions_by_id.get_mut(&sid) {
            builder.exercises.push(ex_log);
        }
    }

    let mut sessions: Vec<TrainingSession> = order
        .iter()
        .filter_map(|sid| {
            sessions_by_id.remove(sid).map(|b| TrainingSession {
                id: b.id,
                plan_id: String::new(),
                plan_name: b.plan_name,
                started_at: b.started_at,
                finished_at: b.finished_at,
                total_planned_seconds: b.total_planned_seconds,
                total_actual_seconds: b.total_actual_seconds,
                total_rounds: b.total_rounds,
                rest_between_rounds_seconds: b.rest_between_rounds_seconds,
                exercises: b.exercises,
            })
        })
        .collect();

    sessions.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    (sessions, skipped_rows)
}

struct SessionBuilder {
    id: String,
    plan_name: String,
    total_planned_seconds: i64,
    total_actual_seconds: i64,
    total_rounds: i64,
    rest_between_rounds_seconds: i64,
    started_at: DateTime<Local>,
    finished_at: Option<DateTime<Local>>,
    exercises: Vec<ExerciseLog>,
}

pub fn export_plans_json(plans: &[TrainingPlan], filepath: &str) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(plans)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    fs::write(filepath, json)
}

pub fn import_plans_json(filepath: &str) -> Result<Vec<TrainingPlan>, String> {
    if !std::path::Path::new(filepath).exists() {
        return Err("File not found".to_string());
    }
    match fs::read_to_string(filepath) {
        Ok(content) => {
            serde_json::from_str(&content).map_err(|e| format!("Invalid JSON: {}", e))
        }
        Err(e) => Err(format!("Could not read file: {}", e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Exercise, ExerciseLog, TrainingPlan, TrainingSession};
    use chrono::{DateTime, Local, TimeZone};

    fn sample_session(when: &str) -> TrainingSession {
        let started = Local
            .with_ymd_and_hms(2026, 4, 21, 19, 11, 34)
            .single()
            .unwrap();
        let _ = when;
        TrainingSession {
            id: "325926d2-b874-4f13-978f-7b829757d484".to_string(),
            plan_id: "b227d8ea-5000-4b12-8213-0ddb07fc0a80".to_string(),
            plan_name: "Hyrox Kettlebell Prep".to_string(),
            started_at: started,
            finished_at: Some(started + chrono::Duration::seconds(8)),
            total_planned_seconds: 330,
            total_actual_seconds: 4,
            total_rounds: 1,
            rest_between_rounds_seconds: 60,
            exercises: vec![ExerciseLog {
                exercise_name: "Kettlebell Swings".to_string(),
                planned_duration_seconds: None,
                actual_duration_seconds: Some(3),
                planned_reps: Some(15),
                actual_reps: Some(15),
                planned_weight_kg: Some(12.0),
                actual_weight_kg: Some(12.0),
                rest_seconds: 30,
                actual_rest_seconds: Some(0),
                completed: true,
                round_number: 1,
                started_at: Some(started),
                finished_at: Some(started),
                set_number: 1,
            }],
        }
    }

    fn sample_plan() -> TrainingPlan {
        TrainingPlan {
            id: "af876cbf-4ed3-46fd-b40a-4715de79e270".to_string(),
            name: "Full Body Beginner".to_string(),
            exercises: vec![Exercise {
                name: "Bodyweight Squat".to_string(),
                duration_seconds: None,
                reps: Some(10),
                rest_seconds: 60,
                weight_kg: None,
                image_path: Some("bundled:Bodyweight_Squat".to_string()),
                sets: 1,
            }],
            total_rounds: 1,
            rest_between_rounds_seconds: 60,
            created: Local
                .with_ymd_and_hms(2026, 4, 21, 19, 11, 34)
                .single()
                .unwrap(),
            is_favorite: false,
        }
    }

    #[test]
    fn test_parse_python_naive_iso() {
        // Python datetime.isoformat() produces naive local timestamps
        let json = r#"{
            "id": "x", "plan_id": "", "plan_name": "T",
            "started_at": "2026-04-21T19:11:34.120317",
            "finished_at": null,
            "total_planned_seconds": 60,
            "total_actual_seconds": 0,
            "total_rounds": 1,
            "rest_between_rounds_seconds": 0,
            "exercises": []
        }"#;
        let s: TrainingSession = serde_json::from_str(json).unwrap();
        assert_eq!(s.started_at.format("%Y-%m-%d %H:%M:%S").to_string(), "2026-04-21 19:11:34");
        // and with a timezone offset (Rust-write → Python-read is also fine)
        let json_tz = json.replace("19:11:34.120317", "19:11:34.120317+02:00");
        let s2: TrainingSession = serde_json::from_str(&json_tz).unwrap();
        // The stored value is converted to the machine's local timezone,
        // so assert on the absolute instant (TZ-independent): the
        // +02:00 timestamp is 17:11:34 UTC.
        assert_eq!(
            s2.started_at
                .with_timezone(&chrono::Utc)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string(),
            "2026-04-21 17:11:34"
        );
    }

    #[test]
    fn test_round_trip_matches_python_layout() {
        let session = sample_session("");
        let json = serde_json::to_string(&session).unwrap();
        // Same key layout the Python dataclass produced
        for key in [
            "\"id\":", "\"plan_id\":", "\"plan_name\":", "\"started_at\":",
            "\"finished_at\":", "\"total_planned_seconds\":",
            "\"total_actual_seconds\":", "\"total_rounds\":",
            "\"rest_between_rounds_seconds\":", "\"exercises\":",
        ] {
            assert!(json.contains(key), "missing key {}", key);
        }
        // Naive local ISO like Python
        assert!(json.contains("\"started_at\":\"2026-04-21T19:11:34.000000\""));
        assert!(json.contains("\"set_number\":1"));
        assert!(json.contains("\"planned_weight_kg\":12.0"));

        let reparsed: TrainingSession = serde_json::from_str(&json).unwrap();
        assert_eq!(reparsed.exercises.len(), 1);
        assert_eq!(reparsed.total_actual_seconds, 4);
    }

    #[test]
    fn test_csv_round_trip() {
        let sessions = vec![sample_session("")];
        let path = "/tmp/opencode/test_history.csv";
        std::fs::create_dir_all("/tmp/opencode").unwrap();
        export_history_csv(&sessions, path).unwrap();
        let (imported, skipped) = import_history_csv(path);
        assert_eq!(skipped, 0);
        assert_eq!(imported.len(), 1);
        assert_eq!(imported[0].plan_name, "Hyrox Kettlebell Prep");
        assert_eq!(imported[0].exercises.len(), 1);
        assert_eq!(imported[0].exercises[0].exercise_name, "Kettlebell Swings");
        assert_eq!(imported[0].exercises[0].planned_reps, Some(15));
    }

    #[test]
    fn test_csv_import_counts_skipped_rows() {
        let sessions = vec![sample_session("")];
        let path = "/tmp/opencode/test_history_skip.csv";
        std::fs::create_dir_all("/tmp/opencode").unwrap();
        export_history_csv(&sessions, path).unwrap();
        let content = fs::read_to_string(path).unwrap();
        // Append a row with an empty session id (invalid)
        let mut bad = content;
        bad.push_str(",,,,,,,,,,,,,,,,,,,,,,,\n");
        fs::write(path, bad).unwrap();
        let (imported, skipped) = import_history_csv(path);
        assert_eq!(imported.len(), 1);
        assert_eq!(skipped, 1);
    }

    #[test]
    fn test_plan_round_trip() {
        let plan = sample_plan();
        let path = "/tmp/opencode/test_plans.json";
        export_plans_json(&[plan.clone()], path).unwrap();
        let imported = import_plans_json(path).unwrap();
        assert_eq!(imported.len(), 1);
        assert_eq!(imported[0].exercises[0].image_path, plan.exercises[0].image_path);
        assert_eq!(imported[0].exercises[0].rest_seconds, 60);
    }
}

#[cfg(test)]
mod real_data_tests {
    use super::*;
    use crate::models::{ExerciseLog, TrainingPlan, TrainingSession};
    use std::path::PathBuf;

    #[test]
    fn test_load_real_user_data() {
        let home = std::env::var("HOME").unwrap();
        let plans_path = PathBuf::from(&home).join(".local/share/training-flatpak/plans.json");
        let history_path =
            PathBuf::from(&home).join(".local/share/training-flatpak/history.json");
        let settings_path =
            PathBuf::from(&home).join(".local/share/training-flatpak/settings.json");

        if let Ok(text) = std::fs::read_to_string(&plans_path) {
            let plans: Vec<TrainingPlan> = serde_json::from_str(&text).expect("real plans parse");
            println!("real plans parsed: {}", plans.len());
            for p in plans.iter().take(3) {
                println!("  plan: {} ({} exercises, {} rounds)", p.name, p.exercises.len(), p.total_rounds);
            }
            assert!(!plans.is_empty());
            // round-trip
            let json = serde_json::to_string_pretty(&plans).unwrap();
            let reparsed: Vec<TrainingPlan> = serde_json::from_str(&json).unwrap();
            assert_eq!(reparsed.len(), plans.len());
        } else {
            println!("no real plans file; skipping");
        }

        if let Ok(text) = std::fs::read_to_string(&history_path) {
            let sessions: Vec<TrainingSession> =
                serde_json::from_str(&text).expect("real history parse");
            println!("real history parsed: {} sessions", sessions.len());
            if let Some(s) = sessions.first() {
                let _ = &s;
                println!("  first: {} at {}", s.plan_name, s.started_at.format("%Y-%m-%d %H:%M"));
            }
            let json = serde_json::to_string_pretty(&sessions).unwrap();
            let reparsed: Vec<TrainingSession> = serde_json::from_str(&json).unwrap();
            assert_eq!(reparsed.len(), sessions.len());
        } else {
            println!("no real history file; skipping");
        }

        if let Ok(text) = std::fs::read_to_string(&settings_path) {
            let settings: crate::settings::AppSettings =
                serde_json::from_str(&text).expect("real settings parse");
            println!(
                "real settings parsed: ai_model={}, color_scheme={}, deck_mode={}",
                settings.ai_model, settings.color_scheme, settings.deck_mode
            );
            assert!(settings.sound_enabled);
        }
    }
}
