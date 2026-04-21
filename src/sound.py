import os
from pathlib import Path
import gi
gi.require_version('Gst', '1.0')
from gi.repository import Gst


Gst.init(None)

_SOUNDS_DIR_LOCAL = os.path.join(os.path.dirname(os.path.abspath(__file__)), "resources", "sounds")
_SOUNDS_DIR_FLATPAK = "/app/share/training/resources/sounds"

AVAILABLE_SOUNDS = ("beep", "round_start", "round_end", "exercise_complete", "training_complete")


def _get_sounds_dir() -> str:
    if os.path.exists(_SOUNDS_DIR_FLATPAK):
        return _SOUNDS_DIR_FLATPAK
    return _SOUNDS_DIR_LOCAL


class SoundPlayer:
    def __init__(self):
        self._playbin = None
        self._bus_watch_id = None

    def play_sound(self, name: str) -> None:
        if name not in AVAILABLE_SOUNDS:
            return
        filename = f"{name}.ogg"
        path = os.path.join(_get_sounds_dir(), filename)
        if not os.path.exists(path):
            return

        self.stop()

        self._playbin = Gst.ElementFactory.make("playbin", "playbin")
        if self._playbin is None:
            return

        uri = Path(path).as_uri()
        self._playbin.set_property("uri", uri)

        bus = self._playbin.get_bus()
        bus.add_signal_watch()
        self._bus_watch_id = bus.connect("message::eos", self._on_eos)
        bus.connect("message::error", self._on_error)

        self._playbin.set_state(Gst.State.PLAYING)

    def play_beep(self) -> None:
        self.play_sound("beep")

    def _on_eos(self, bus, message):
        if self._playbin:
            self._playbin.set_state(Gst.State.NULL)

    def _on_error(self, bus, message):
        if self._playbin:
            self._playbin.set_state(Gst.State.NULL)

    def stop(self) -> None:
        if self._playbin:
            self._playbin.set_state(Gst.State.NULL)
            self._playbin = None


sound_player = SoundPlayer()