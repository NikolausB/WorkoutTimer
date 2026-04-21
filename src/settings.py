import json
import os
from dataclasses import dataclass, field, asdict
from gi.repository import GLib


_SETTINGS_DIR = os.path.join(GLib.get_user_data_dir(), "training-flatpak")
_SETTINGS_PATH = os.path.join(_SETTINGS_DIR, "settings.json")

_SOUND_EVENTS = (
    "round_start_sound",
    "round_end_sound",
    "countdown_tick_sound",
    "exercise_complete_sound",
    "training_complete_sound",
)

DEFAULT_SETTINGS = {
    "sound_enabled": True,
    "round_start_sound": "round_start",
    "round_end_sound": "round_end",
    "countdown_tick_sound": "beep",
    "exercise_complete_sound": "exercise_complete",
    "training_complete_sound": "training_complete",
    "show_exercise_images": True,
    "show_home_page": True,
    "show_timer_page": True,
    "show_workout_page": True,
}


@dataclass
class AppSettings:
    sound_enabled: bool = True
    round_start_sound: str = "round_start"
    round_end_sound: str = "round_end"
    countdown_tick_sound: str = "beep"
    exercise_complete_sound: str = "exercise_complete"
    training_complete_sound: str = "training_complete"
    show_exercise_images: bool = True
    show_home_page: bool = True
    show_timer_page: bool = True
    show_workout_page: bool = True

    def get_sound(self, event_key: str) -> str | None:
        if not self.sound_enabled:
            return None
        name = getattr(self, event_key, None)
        if name == "none" or name is None:
            return None
        return name

    def to_dict(self) -> dict:
        return asdict(self)

    @classmethod
    def from_dict(cls, d: dict) -> "AppSettings":
        defaults = DEFAULT_SETTINGS.copy()
        defaults.update({k: v for k, v in d.items() if k in defaults})
        return cls(**defaults)


def load_settings() -> AppSettings:
    if not os.path.exists(_SETTINGS_PATH):
        return AppSettings()
    try:
        with open(_SETTINGS_PATH, "r") as f:
            data = json.load(f)
        return AppSettings.from_dict(data)
    except (json.JSONDecodeError, IOError):
        return AppSettings()


def save_settings(settings: AppSettings) -> None:
    os.makedirs(_SETTINGS_DIR, exist_ok=True)
    with open(_SETTINGS_PATH, "w") as f:
        json.dump(settings.to_dict(), f, indent=2)


app_settings = load_settings()