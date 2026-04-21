import json
import os
from gi.repository import GLib


_USER_EXERCISES_PATH = os.path.join(GLib.get_user_data_dir(), "training-flatpak", "user_exercises.json")


def load_user_exercises() -> list[dict]:
    if not os.path.exists(_USER_EXERCISES_PATH):
        return []
    try:
        with open(_USER_EXERCISES_PATH, "r") as f:
            return json.load(f)
    except (json.JSONDecodeError, IOError):
        return []


def save_user_exercises(exercises: list[dict]) -> None:
    os.makedirs(os.path.dirname(_USER_EXERCISES_PATH), exist_ok=True)
    with open(_USER_EXERCISES_PATH, "w") as f:
        json.dump(exercises, f, indent=2)


def add_user_exercise(name: str, primary_muscles: list[str], equipment: str, image_path: str | None) -> dict:
    exercises = load_user_exercises()
    entry = {
        "name": name,
        "primaryMuscles": primary_muscles,
        "equipment": equipment,
        "image_path": image_path,
        "source": "user",
    }
    exercises.append(entry)
    save_user_exercises(exercises)
    return entry


def delete_user_exercise(index: int) -> None:
    exercises = load_user_exercises()
    if 0 <= index < len(exercises):
        exercises.pop(index)
        save_user_exercises(exercises)