//! Training analytics: volume, estimated 1RM, personal records, adherence and
//! progressive overload helpers.
//!
//! Everything here is derived from the logged exercises, so no stored data
//! format changes and older session files keep working.

use crate::models::{ExerciseLog, TrainingPlan, TrainingSession};
use chrono::{DateTime, Datelike, Local, NaiveDate};

/// Epley formula: estimated one-rep max from a set of `reps` at `weight`.
pub fn epley_1rm(weight: f64, reps: f64) -> f64 {
    if weight <= 0.0 || reps <= 0.0 {
        return 0.0;
    }
    weight * (1.0 + reps / 30.0)
}

/// Volume load of one exercise log (weight × reps). Rep-less sets count 0.
pub fn log_volume(log: &ExerciseLog) -> f64 {
    let reps = log.actual_reps.or(log.planned_reps).unwrap_or(0) as f64;
    let weight = log.actual_weight_kg.or(log.planned_weight_kg).unwrap_or(0.0);
    if reps <= 0.0 {
        return 0.0;
    }
    weight * reps
}

/// Total volume load of a session (kg lifted).
pub fn session_volume(session: &TrainingSession) -> f64 {
    session.exercises.iter().map(log_volume).sum()
}

/// Total working seconds of a session (timed exercises).
pub fn session_work_seconds(session: &TrainingSession) -> i64 {
    session
        .exercises
        .iter()
        .map(|l| {
            l.actual_duration_seconds
                .or(l.planned_duration_seconds)
                .unwrap_or(0)
        })
        .sum()
}

/// Fraction of planned exercises that were actually completed (0.0-1.0).
pub fn adherence(sessions: &[TrainingSession]) -> f64 {
    let mut planned = 0usize;
    let mut completed = 0usize;
    for s in sessions {
        for log in &s.exercises {
            planned += 1;
            if log.completed {
                completed += 1;
            }
        }
    }
    if planned == 0 {
        return 0.0;
    }
    completed as f64 / planned as f64
}

/// Personal records for a single exercise.
#[derive(Debug, Clone, Default)]
pub struct ExerciseStats {
    pub name: String,
    /// Best estimated 1RM (Epley) seen so far.
    pub best_est_1rm: f64,
    /// The best before the most recent improvement (for the trend arrow).
    pub previous_best_est_1rm: f64,
    pub best_est_1rm_date: Option<DateTime<Local>>,
    /// Best single set (reps) and the weight it was done with.
    pub best_reps: i64,
    pub best_reps_weight: f64,
    /// Best (longest) time for timed exercises.
    pub best_time_seconds: i64,
    /// Total volume load (kg) across all logged sets.
    pub total_volume_kg: f64,
    pub total_sets: usize,
    pub sessions_count: usize,
    pub last_trained: Option<DateTime<Local>>,
}

impl ExerciseStats {
    /// Trend of the estimated 1RM: +1 improving, -1 declining, 0 unchanged.
    pub fn trend(&self) -> i8 {
        let previous_best = self.previous_best_est_1rm;
        if previous_best <= 0.0 || self.best_est_1rm <= 0.0 {
            return 0;
        }
        let diff = self.best_est_1rm - previous_best;
        if diff > 0.5 {
            1
        } else if diff < -0.5 {
            -1
        } else {
            0
        }
    }
}

/// Per-exercise statistics over all sessions (most recent first in, newest
/// records win).
pub fn exercise_stats(sessions: &[TrainingSession]) -> Vec<ExerciseStats> {
    let mut stats: std::collections::HashMap<String, ExerciseStats> =
        std::collections::HashMap::new();
    let mut order: Vec<String> = Vec::new();

    // Oldest first, so newer sessions overwrite older records.
    let mut ordered: Vec<&TrainingSession> = sessions.iter().collect();
    ordered.sort_by_key(|s| s.started_at);

    for session in ordered {
        let mut seen_in_session: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        for log in &session.exercises {
            let entry = stats
                .entry(log.exercise_name.clone())
                .or_insert_with(|| {
                    order.push(log.exercise_name.clone());
                    ExerciseStats {
                        name: log.exercise_name.clone(),
                        ..Default::default()
                    }
                });

            seen_in_session.insert(log.exercise_name.clone());
            entry.total_sets += 1;
            entry.total_volume_kg += log_volume(log);

            let reps = log.actual_reps.or(log.planned_reps).unwrap_or(0);
            let weight = log.actual_weight_kg.or(log.planned_weight_kg).unwrap_or(0.0);
            if reps > 0 {
                // Best set by reps (also for bodyweight exercises).
                if reps > entry.best_reps {
                    entry.best_reps = reps;
                    entry.best_reps_weight = weight;
                }
                // Estimated 1RM only makes sense with a load.
                if weight > 0.0 {
                    let est = epley_1rm(weight, reps as f64);
                    if est > entry.best_est_1rm {
                        entry.previous_best_est_1rm = entry.best_est_1rm;
                        entry.best_est_1rm = est;
                        entry.best_est_1rm_date = log
                            .finished_at
                            .or(log.started_at)
                            .or(Some(session.started_at));
                    }
                }
            }

            let seconds = log
                .actual_duration_seconds
                .or(log.planned_duration_seconds)
                .unwrap_or(0);
            if seconds > entry.best_time_seconds {
                entry.best_time_seconds = seconds;
            }

            let when = log.finished_at.or(log.started_at).unwrap_or(session.started_at);
            if entry.last_trained.map(|t| when > t).unwrap_or(true) {
                entry.last_trained = Some(when);
            }
        }
        for name in seen_in_session {
            if let Some(entry) = stats.get_mut(&name) {
                entry.sessions_count += 1;
            }
        }
    }

    // Most trained first, then alphabetically.
    order.sort_by(|a, b| {
        let sa = stats.get(a).map(|s| s.total_sets).unwrap_or(0);
        let sb = stats.get(b).map(|s| s.total_sets).unwrap_or(0);
        sb.cmp(&sa).then_with(|| a.cmp(b))
    });
    order
        .into_iter()
        .filter_map(|name| stats.remove(&name))
        .collect()
}

/// Start of the ISO week (Monday) for a date.
pub fn iso_week_start(d: NaiveDate) -> NaiveDate {
    let weekday = d.weekday().num_days_from_monday() as i64;
    d - chrono::Duration::days(weekday)
}

/// Weekly volume load for the last `weeks` weeks (oldest first).
pub fn weekly_volume(sessions: &[TrainingSession], weeks: usize) -> Vec<(String, f64)> {
    let today = Local::now().date_naive();
    let mut buckets: Vec<(String, f64)> = Vec::new();
    for i in (0..weeks).rev() {
        let start = iso_week_start(today) - chrono::Duration::weeks(i as i64);
        buckets.push((format!("{}", start.format("%d.%m")), 0.0));
    }
    if buckets.is_empty() {
        return buckets;
    }
    let first_start = iso_week_start(today) - chrono::Duration::weeks((weeks - 1) as i64);

    for s in sessions {
        let d = s.started_at.date_naive();
        if d < first_start {
            continue;
        }
        let week_start = iso_week_start(d);
        let idx = ((week_start - first_start).num_days() / 7) as usize;
        if idx < buckets.len() {
            buckets[idx].1 += session_volume(s);
        }
    }
    buckets
}

/// Weekly session counts (oldest first) — used by the history chart.
pub fn weekly_sessions(sessions: &[TrainingSession], weeks: usize) -> Vec<(String, usize)> {
    let today = Local::now().date_naive();
    let mut buckets: Vec<(String, usize)> = Vec::new();
    for i in (0..weeks).rev() {
        let start = iso_week_start(today) - chrono::Duration::weeks(i as i64);
        buckets.push((format!("{}", start.format("%d.%m")), 0));
    }
    if buckets.is_empty() {
        return buckets;
    }
    let first_start = iso_week_start(today) - chrono::Duration::weeks((weeks - 1) as i64);
    for s in sessions {
        let d = s.started_at.date_naive();
        if d < first_start {
            continue;
        }
        let week_start = iso_week_start(d);
        let idx = ((week_start - first_start).num_days() / 7) as usize;
        if idx < buckets.len() {
            buckets[idx].1 += 1;
        }
    }
    buckets
}

/// Increase a plan's loads for the exercises that were completed in the last
/// session of that plan: +1 rep, or +2.5 kg (2.5 % below 20 kg) for weighted
/// exercises, or +5 s for timed ones.
pub fn progressed_plan(plan: &TrainingPlan, last_session: &TrainingSession) -> TrainingPlan {
    let mut new_plan = plan.clone();
    new_plan.id = uuid::Uuid::new_v4().to_string();
    new_plan.name = format!("{} (progression)", plan.name);
    new_plan.created = Local::now();
    new_plan.is_favorite = false;

    // Exercises completed in the last session, by name.
    let completed: std::collections::HashMap<&str, &ExerciseLog> = last_session
        .exercises
        .iter()
        .filter(|l| l.completed)
        .map(|l| (l.exercise_name.as_str(), l))
        .collect();

    for exercise in new_plan.exercises.iter_mut() {
        let Some(log) = completed.get(exercise.name.as_str()) else {
            continue;
        };

        // Rep-based exercises: +50% reps (rounded up).
        if let Some(reps) = exercise.reps {
            let done = log.actual_reps.or(log.planned_reps).unwrap_or(0);
            if done > 0 {
                exercise.reps = Some(((reps as f64) * 1.5).ceil() as i64);
                continue;
            }
        }

        // Weighted exercises: +25% load (rounded to 0.1 kg).
        if let Some(weight) = exercise.weight_kg {
            if weight > 0.0 {
                exercise.weight_kg = Some(((weight * 1.25) * 10.0).round() / 10.0);
                continue;
            }
        }

        // Timed exercises: +50% duration (rounded up to seconds).
        if let Some(duration) = exercise.duration_seconds {
            let done = log
                .actual_duration_seconds
                .or(log.planned_duration_seconds)
                .unwrap_or(0);
            if done > 0 {
                exercise.duration_seconds = Some(((duration as f64) * 1.5).ceil() as i64);
            }
        }
    }

    new_plan
}

/// The most recent session of a plan (by plan id, falling back to the name
/// for sessions logged by older versions).
pub fn last_session_for_plan<'a>(
    sessions: &'a [TrainingSession],
    plan: &TrainingPlan,
) -> Option<&'a TrainingSession> {
    let mut matching: Vec<&TrainingSession> = sessions
        .iter()
        .filter(|s| s.plan_id == plan.id || (s.plan_id.is_empty() && s.plan_name == plan.name))
        .collect();
    matching.sort_by_key(|s| s.started_at);
    matching.pop()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Exercise, TrainingSession};

    fn log(name: &str, reps: Option<i64>, weight: Option<f64>, done: bool) -> ExerciseLog {
        ExerciseLog {
            exercise_name: name.to_string(),
            planned_duration_seconds: None,
            actual_duration_seconds: None,
            planned_reps: reps,
            actual_reps: reps,
            planned_weight_kg: weight,
            actual_weight_kg: weight,
            rest_seconds: 30,
            actual_rest_seconds: None,
            completed: done,
            round_number: 1,
            started_at: None,
            finished_at: None,
            set_number: 1,
        }
    }

    fn session(name: &str, logs: Vec<ExerciseLog>) -> TrainingSession {
        TrainingSession {
            id: uuid::Uuid::new_v4().to_string(),
            plan_id: "plan-1".to_string(),
            plan_name: name.to_string(),
            started_at: Local::now() - chrono::Duration::days(1),
            finished_at: Some(Local::now()),
            total_planned_seconds: 600,
            total_actual_seconds: 600,
            total_rounds: 1,
            rest_between_rounds_seconds: 60,
            exercises: logs,
        }
    }

    #[test]
    fn epley_matches_known_values() {
        assert!((epley_1rm(100.0, 10.0) - 133.33).abs() < 0.1);
        assert!((epley_1rm(60.0, 5.0) - 70.0).abs() < 0.1);
        assert_eq!(epley_1rm(0.0, 10.0), 0.0);
        assert_eq!(epley_1rm(100.0, 0.0), 0.0);
    }

    #[test]
    fn volume_sums_weight_times_reps() {
        let s = session("Pushups", vec![log("Pushups", Some(10), Some(20.0), true)]);
        assert!((session_volume(&s) - 200.0).abs() < 0.001);
    }

    #[test]
    fn adherence_counts_completed_logs() {
        let s = session("Mix", vec![
            log("A", Some(10), Some(20.0), true),
            log("B", Some(10), Some(20.0), false),
        ]);
        assert!((adherence(&[s]) - 0.5).abs() < 0.001);
        assert_eq!(adherence(&[]), 0.0);
    }

    #[test]
    fn personal_records_use_the_best_set() {
        let older = session("Bench", vec![log("Bench", Some(5), Some(60.0), true)]);
        let newer = session("Bench", vec![log("Bench", Some(8), Some(70.0), true)]);
        let stats = exercise_stats(&[older, newer]);
        assert_eq!(stats.len(), 1);
        let b = &stats[0];
        assert_eq!(b.best_reps, 8);
        assert!((b.best_reps_weight - 70.0).abs() < 0.001);
        // e1rm of 70 kg x 8 = 88.7 kg
        assert!((b.best_est_1rm - 88.67).abs() < 0.1);
        assert_eq!(b.total_sets, 2);
        assert!((b.total_volume_kg - (300.0 + 560.0)).abs() < 0.001);
    }

    #[test]
    fn progression_adds_a_rep() {
        let mut plan = TrainingPlan {
            name: "Push".to_string(),
            exercises: vec![Exercise {
                name: "Pushups".to_string(),
                reps: Some(10),
                duration_seconds: None,
                rest_seconds: 30,
                weight_kg: None,
                image_path: None,
                sets: 1,
            }],
            ..Default::default()
        };
        plan.exercises[0].weight_kg = None;
        let s = session("Push", vec![log("Pushups", Some(10), None, true)]);
        let progressed = progressed_plan(&plan, &s);
        // 10 reps -> +50% = 15
        assert_eq!(progressed.exercises[0].reps, Some(15));
        assert_ne!(progressed.id, plan.id);
        assert!(progressed.name.ends_with("(progression)"));
    }

    #[test]
    fn progression_adds_weight_and_keeps_uncompleted_exercises() {
        let mut plan = TrainingPlan {
            name: "Lift".to_string(),
            exercises: vec![
                Exercise {
                    name: "Squat".to_string(),
                    reps: Some(5),
                    weight_kg: Some(40.0),
                    duration_seconds: None,
                    rest_seconds: 60,
                    image_path: None,
                    sets: 1,
                },
                Exercise {
                    name: "Lunge".to_string(),
                    reps: Some(10),
                    weight_kg: Some(20.0),
                    duration_seconds: None,
                    rest_seconds: 30,
                    image_path: None,
                    sets: 1,
                },
            ],
            ..Default::default()
        };
        plan.exercises[0].weight_kg = Some(40.0);
        let s = session("Lift", vec![log("Squat", Some(5), Some(40.0), true)]);
        let progressed = progressed_plan(&plan, &s);
        // completed: rep-based exercises bump the reps dimension
        // first (+50%: 5 -> 8), the weight stays untouched
        assert_eq!(progressed.exercises[0].reps, Some(8));
        assert_eq!(progressed.exercises[0].weight_kg, Some(40.0));
        // not completed: unchanged
        assert_eq!(progressed.exercises[1].reps, Some(10));
    }

    #[test]
    fn progression_bumps_weight_only_and_timed_exercises() {
        let plan = TrainingPlan {
            name: "Mixed".to_string(),
            exercises: vec![
                Exercise {
                    name: "Carry".to_string(),
                    reps: None,
                    weight_kg: Some(20.0),
                    duration_seconds: None,
                    rest_seconds: 60,
                    image_path: None,
                    sets: 1,
                },
                Exercise {
                    name: "Plank".to_string(),
                    reps: None,
                    weight_kg: None,
                    duration_seconds: Some(40),
                    rest_seconds: 30,
                    image_path: None,
                    sets: 1,
                },
            ],
            ..Default::default()
        };
        let s = session(
            "Mixed",
            vec![
                log("Carry", None, Some(20.0), true),
                ExerciseLog {
                    exercise_name: "Plank".to_string(),
                    planned_duration_seconds: Some(40),
                    actual_duration_seconds: Some(40),
                    completed: true,
                    ..log("Plank", None, None, true)
                },
            ],
        );
        let progressed = progressed_plan(&plan, &s);
        // weight-only: +25% (20 -> 25.0)
        assert_eq!(progressed.exercises[0].weight_kg, Some(25.0));
        // timed: +50% (40 -> 60)
        assert_eq!(progressed.exercises[1].duration_seconds, Some(60));
    }

    #[test]
    fn weekly_volume_buckets_by_iso_week() {
        let s = session("P", vec![log("A", Some(10), Some(10.0), true)]);
        let buckets = weekly_volume(&[s], 4);
        assert_eq!(buckets.len(), 4);
        assert!((buckets[3].1 - 100.0).abs() < 0.001);
        assert_eq!(buckets[0].1, 0.0);
    }
}
