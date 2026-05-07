import csv
import os
from datetime import datetime
from typing import Optional

from models import TrainingSession, ExerciseLog


CSV_HEADER = [
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
]


def export_history_csv(sessions: list[TrainingSession], filepath: str) -> None:
    """Export training sessions to a flat CSV file (one row per exercise)."""
    with open(filepath, "w", newline="", encoding="utf-8") as f:
        writer = csv.writer(f)
        writer.writerow(CSV_HEADER)

        for session in sessions:
            for ex in session.exercises:
                writer.writerow([
                    session.id,
                    session.plan_name,
                    session.started_at.isoformat(),
                    session.finished_at.isoformat() if session.finished_at else "",
                    session.total_planned_seconds,
                    session.compute_total_actual_seconds(),
                    session.total_rounds,
                    session.rest_between_rounds_seconds,
                    ex.exercise_name,
                    _fmt(ex.planned_duration_seconds),
                    _fmt(ex.actual_duration_seconds),
                    _fmt(ex.planned_reps),
                    _fmt(ex.actual_reps),
                    _fmt_f(ex.planned_weight_kg),
                    _fmt_f(ex.actual_weight_kg),
                    ex.rest_seconds,
                    _fmt(ex.actual_rest_seconds),
                    "True" if ex.completed else "False",
                    ex.round_number,
                ])


def import_history_csv(filepath: str) -> list[TrainingSession]:
    """Import training sessions from a flat CSV file."""
    if not os.path.exists(filepath):
        return []

    sessions_by_id: dict[str, dict] = {}

    with open(filepath, "r", newline="", encoding="utf-8") as f:
        reader = csv.DictReader(f)
        for row in reader:
            sid = row.get("session_id", "").strip()
            if not sid:
                continue

            if sid not in sessions_by_id:
                started_str = row.get("started_at", "").strip()
                finished_str = row.get("finished_at", "").strip()
                sessions_by_id[sid] = {
                    "id": sid,
                    "plan_name": row.get("plan_name", "").strip(),
                    "total_planned_seconds": _int(row.get("total_planned_seconds", "")),
                    "total_actual_seconds": _int(row.get("total_actual_seconds", "")),
                    "total_rounds": _int(row.get("total_rounds", "1")),
                    "rest_between_rounds_seconds": _int(row.get("rest_between_rounds_seconds", "0")),
                    "started_at": datetime.fromisoformat(started_str) if started_str else datetime.now(),
                    "finished_at": datetime.fromisoformat(finished_str) if finished_str else None,
                    "exercises": [],
                }

            ex_log = {
                "exercise_name": row.get("exercise_name", "").strip(),
                "planned_duration_seconds": _blank_to_none_int(row.get("planned_duration_seconds", "")),
                "actual_duration_seconds": _blank_to_none_int(row.get("actual_duration_seconds", "")),
                "planned_reps": _blank_to_none_int(row.get("planned_reps", "")),
                "actual_reps": _blank_to_none_int(row.get("actual_reps", "")),
                "planned_weight_kg": _blank_to_none_float(row.get("planned_weight_kg", "")),
                "actual_weight_kg": _blank_to_none_float(row.get("actual_weight_kg", "")),
                "rest_seconds": _int(row.get("rest_seconds", "0")),
                "actual_rest_seconds": _blank_to_none_int(row.get("actual_rest_seconds", "")),
                "completed": row.get("completed", "").strip().lower() in ("true", "1", "yes", "on"),
                "round_number": _int(row.get("round_number", "1")),
            }
            sessions_by_id[sid]["exercises"].append(ex_log)

    sessions: list[TrainingSession] = []
    for sid, data in sessions_by_id.items():
        session = TrainingSession(
            id=data["id"],
            plan_name=data["plan_name"],
            total_planned_seconds=data.get("total_planned_seconds", 0),
            total_actual_seconds=data.get("total_actual_seconds", 0),
            total_rounds=data.get("total_rounds", 1),
            rest_between_rounds_seconds=data.get("rest_between_rounds_seconds", 0),
            started_at=data["started_at"],
            finished_at=data.get("finished_at"),
            exercises=[ExerciseLog.from_dict(e) for e in data["exercises"]],
        )
        sessions.append(session)

    sessions.sort(key=lambda s: s.started_at, reverse=True)
    return sessions


# --- helpers ---

def _fmt(val: int | None) -> str:
    return "" if val is None else str(val)


def _fmt_f(val: float | None) -> str:
    return "" if val is None else f"{val:g}"


def _blank_to_none_int(val: str) -> Optional[int]:
    v = val.strip()
    if v == "":
        return None
    try:
        return int(v)
    except ValueError:
        return None


def _blank_to_none_float(val: str) -> Optional[float]:
    v = val.strip()
    if v == "":
        return None
    try:
        return float(v)
    except ValueError:
        return None


def _int(val: str) -> int:
    try:
        return int(val.strip() or 0)
    except ValueError:
        return 0