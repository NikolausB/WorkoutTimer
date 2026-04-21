from dataclasses import dataclass, field
from datetime import datetime
from typing import Optional
import uuid


@dataclass
class RoundConfig:
    rounds: int = 10
    round_seconds: int = 180
    pause_seconds: int = 60


@dataclass
class Exercise:
    name: str = ""
    duration_seconds: Optional[int] = None
    reps: Optional[int] = None
    rest_seconds: int = 30
    image_path: Optional[str] = None

    def is_timed(self) -> bool:
        return self.duration_seconds is not None and self.duration_seconds > 0

    def to_dict(self) -> dict:
        d = {
            "name": self.name,
            "duration_seconds": self.duration_seconds,
            "reps": self.reps,
            "rest_seconds": self.rest_seconds,
        }
        if self.image_path is not None:
            d["image_path"] = self.image_path
        return d

    @classmethod
    def from_dict(cls, d: dict) -> "Exercise":
        return cls(
            name=d.get("name", ""),
            duration_seconds=d.get("duration_seconds"),
            reps=d.get("reps"),
            rest_seconds=d.get("rest_seconds", 30),
            image_path=d.get("image_path"),
        )


@dataclass
class TrainingPlan:
    id: str = field(default_factory=lambda: str(uuid.uuid4()))
    name: str = ""
    exercises: list[Exercise] = field(default_factory=list)
    total_rounds: int = 1
    rest_between_rounds_seconds: int = 60
    created: datetime = field(default_factory=datetime.now)

    def total_planned_seconds(self) -> int:
        total = 0
        for ex in self.exercises:
            if ex.duration_seconds:
                total += ex.duration_seconds
            total += ex.rest_seconds
        total *= self.total_rounds
        if self.total_rounds > 1 and self.rest_between_rounds_seconds > 0:
            total += self.rest_between_rounds_seconds * (self.total_rounds - 1)
        return total

    def to_dict(self) -> dict:
        return {
            "id": self.id,
            "name": self.name,
            "exercises": [e.to_dict() for e in self.exercises],
            "total_rounds": self.total_rounds,
            "rest_between_rounds_seconds": self.rest_between_rounds_seconds,
            "created": self.created.isoformat(),
        }

    @classmethod
    def from_dict(cls, d: dict) -> "TrainingPlan":
        return cls(
            id=d.get("id", str(uuid.uuid4())),
            name=d.get("name", ""),
            exercises=[Exercise.from_dict(e) for e in d.get("exercises", [])],
            total_rounds=d.get("total_rounds", 1),
            rest_between_rounds_seconds=d.get("rest_between_rounds_seconds", 60),
            created=datetime.fromisoformat(d["created"]) if "created" in d else datetime.now(),
        )


@dataclass
class ExerciseLog:
    exercise_name: str = ""
    planned_duration_seconds: Optional[int] = None
    actual_duration_seconds: Optional[int] = None
    planned_reps: Optional[int] = None
    actual_reps: Optional[int] = None
    rest_seconds: int = 0
    actual_rest_seconds: Optional[int] = None
    completed: bool = True
    image_path: Optional[str] = None
    round_number: int = 1

    def to_dict(self) -> dict:
        d = {
            "exercise_name": self.exercise_name,
            "planned_duration_seconds": self.planned_duration_seconds,
            "actual_duration_seconds": self.actual_duration_seconds,
            "planned_reps": self.planned_reps,
            "actual_reps": self.actual_reps,
            "rest_seconds": self.rest_seconds,
            "actual_rest_seconds": self.actual_rest_seconds,
            "completed": self.completed,
            "round_number": self.round_number,
        }
        if self.image_path is not None:
            d["image_path"] = self.image_path
        return d

    @classmethod
    def from_dict(cls, d: dict) -> "ExerciseLog":
        return cls(
            exercise_name=d.get("exercise_name", ""),
            planned_duration_seconds=d.get("planned_duration_seconds"),
            actual_duration_seconds=d.get("actual_duration_seconds"),
            planned_reps=d.get("planned_reps"),
            actual_reps=d.get("actual_reps"),
            rest_seconds=d.get("rest_seconds", 0),
            actual_rest_seconds=d.get("actual_rest_seconds"),
            completed=d.get("completed", True),
            image_path=d.get("image_path"),
            round_number=d.get("round_number", 1),
        )


@dataclass
class TrainingSession:
    id: str = field(default_factory=lambda: str(uuid.uuid4()))
    plan_id: str = ""
    plan_name: str = ""
    started_at: datetime = field(default_factory=datetime.now)
    finished_at: Optional[datetime] = None
    total_planned_seconds: int = 0
    total_actual_seconds: int = 0
    total_rounds: int = 1
    rest_between_rounds_seconds: int = 0
    exercises: list[ExerciseLog] = field(default_factory=list)

    def compute_total_actual_seconds(self) -> int:
        total = 0
        for ex in self.exercises:
            if ex.actual_duration_seconds:
                total += ex.actual_duration_seconds
            if ex.actual_rest_seconds is not None:
                total += ex.actual_rest_seconds
            elif ex.completed:
                total += ex.rest_seconds
        return total

    def to_dict(self) -> dict:
        return {
            "id": self.id,
            "plan_id": self.plan_id,
            "plan_name": self.plan_name,
            "started_at": self.started_at.isoformat(),
            "finished_at": self.finished_at.isoformat() if self.finished_at else None,
            "total_planned_seconds": self.total_planned_seconds,
            "total_actual_seconds": self.compute_total_actual_seconds(),
            "total_rounds": self.total_rounds,
            "rest_between_rounds_seconds": self.rest_between_rounds_seconds,
            "exercises": [e.to_dict() for e in self.exercises],
        }

    @classmethod
    def from_dict(cls, d: dict) -> "TrainingSession":
        return cls(
            id=d.get("id", str(uuid.uuid4())),
            plan_id=d.get("plan_id", ""),
            plan_name=d.get("plan_name", ""),
            started_at=datetime.fromisoformat(d["started_at"]) if "started_at" in d else datetime.now(),
            finished_at=datetime.fromisoformat(d["finished_at"]) if d.get("finished_at") else None,
            total_planned_seconds=d.get("total_planned_seconds", 0),
            total_actual_seconds=d.get("total_actual_seconds", 0),
            total_rounds=d.get("total_rounds", 1),
            rest_between_rounds_seconds=d.get("rest_between_rounds_seconds", 0),
            exercises=[ExerciseLog.from_dict(e) for e in d.get("exercises", [])],
        )