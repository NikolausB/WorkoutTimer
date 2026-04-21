import json
import os
from typing import Optional

from gi.repository import GLib

from models import TrainingPlan, TrainingSession


def _get_data_dir() -> str:
    data_dir = GLib.get_user_data_dir()
    app_dir = os.path.join(data_dir, "training-flatpak")
    os.makedirs(app_dir, exist_ok=True)
    return app_dir


def _plans_path() -> str:
    return os.path.join(_get_data_dir(), "plans.json")


def _history_path() -> str:
    return os.path.join(_get_data_dir(), "history.json")


def _read_json(path: str) -> list:
    if not os.path.exists(path):
        return []
    try:
        with open(path, "r") as f:
            return json.load(f)
    except (json.JSONDecodeError, IOError):
        return []


def _write_json(path: str, data: list) -> None:
    with open(path, "w") as f:
        json.dump(data, f, indent=2)


class DataStore:
    def load_plans(self) -> list[TrainingPlan]:
        raw = _read_json(_plans_path())
        return [TrainingPlan.from_dict(d) for d in raw]

    def save_plans(self, plans: list[TrainingPlan]) -> None:
        _write_json(_plans_path(), [p.to_dict() for p in plans])

    def save_plan(self, plan: TrainingPlan) -> None:
        plans = self.load_plans()
        for i, p in enumerate(plans):
            if p.id == plan.id:
                plans[i] = plan
                self.save_plans(plans)
                return
        plans.append(plan)
        self.save_plans(plans)

    def delete_plan(self, plan_id: str) -> None:
        plans = self.load_plans()
        plans = [p for p in plans if p.id != plan_id]
        self.save_plans(plans)

    def get_plan(self, plan_id: str) -> Optional[TrainingPlan]:
        for p in self.load_plans():
            if p.id == plan_id:
                return p
        return None

    def load_sessions(self) -> list[TrainingSession]:
        raw = _read_json(_history_path())
        sessions = [TrainingSession.from_dict(d) for d in raw]
        sessions.sort(key=lambda s: s.started_at, reverse=True)
        return sessions

    def save_sessions(self, sessions: list[TrainingSession]) -> None:
        _write_json(_history_path(), [s.to_dict() for s in sessions])

    def save_session(self, session: TrainingSession) -> None:
        sessions = self.load_sessions()
        for i, s in enumerate(sessions):
            if s.id == session.id:
                sessions[i] = session
                self.save_sessions(sessions)
                return
        sessions.append(session)
        self.save_sessions(sessions)

    def get_session(self, session_id: str) -> Optional[TrainingSession]:
        for s in self.load_sessions():
            if s.id == session_id:
                return s
        return None

    def get_sessions_for_plan(self, plan_id: str) -> list[TrainingSession]:
        return [s for s in self.load_sessions() if s.plan_id == plan_id]

    def delete_session(self, session_id: str) -> None:
        sessions = self.load_sessions()
        sessions = [s for s in sessions if s.id != session_id]
        self.save_sessions(sessions)

    def seed_default_plans(self) -> None:
        existing = self.load_plans()
        if existing:
            return
        from image_utils import get_plans_dir
        path = os.path.join(get_plans_dir(), "default_plans.json")
        if not os.path.exists(path):
            return
        try:
            with open(path, "r") as f:
                raw = json.load(f)
            plans = [TrainingPlan.from_dict(d) for d in raw]
            self.save_plans(plans)
        except (json.JSONDecodeError, IOError):
            pass