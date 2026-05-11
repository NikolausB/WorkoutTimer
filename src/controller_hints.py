from gi.repository import Gtk


_CONTEXT_HINTS = {
    "runner_exercise": {
        "exercise": "[A] Done  [Y] Skip  [B] Back  [Start] Pause  [Guide] Fullscreen",
        "rest":     "[A] Done  [Y] Skip Rest  [Start] Pause  [Guide] Fullscreen",
        "timed":    "[Start] Pause/Resume  [B] Back  [Guide] Fullscreen",
    },
    "runner_summary":  "[A] Back to Menu  [Guide] Fullscreen",
    "timer":           "[A] Start  [Start] Pause  [B] Skip  [X] Reset  [L1/R1] Tabs  [Guide] Fullscreen",
    "list":            "[A] Open  [B] Back  [L1/R1] Tabs  [Guide] Fullscreen",
    "editor":          "[A] Enter  [B] Back  [←/→] Adjust  [X] Toggle  [Start] Start Training  [L1/R1] Tabs  [Guide] Fullscreen",
    "ai_coach":        "[A] Submit  [B] Back  [←/→] Toggle  [Select] Keyboard  [L1/R1] Tabs  [Guide] Fullscreen",
    "keyboard":        "[A] Type  [B] Backspace  [Y] Shift  [L/R] Cursor  [Start] Close",
}


class ControllerHintsOverlay(Gtk.Box):
    def __init__(self):
        super().__init__(orientation=Gtk.Orientation.HORIZONTAL, spacing=18)
        self._label = Gtk.Label(label="", css_classes=["controller-hints-label"])
        self.append(self._label)
        self.set_visible(False)
        self.set_halign(Gtk.Align.END)
        self.set_valign(Gtk.Align.END)
        self.set_margin_bottom(8)
        self._context = None
        self._sub_key = ""

    def set_context(self, key):
        text = _CONTEXT_HINTS.get(key, "")
        if isinstance(text, dict):
            text = text.get(self._sub_key, "")
        self._label.set_label(text)
        self.set_visible(bool(text))
        self._context = key

    def set_sub_key(self, sub_key: str):
        self._sub_key = sub_key
        if self._context:
            self.set_context(self._context)

    def show(self):
        self.set_visible(True)

    def hide(self):
        self.set_visible(False)
