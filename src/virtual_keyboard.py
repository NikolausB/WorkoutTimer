from gi.repository import Gtk


_LAYOUT = [
    ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "-", "="],
    ["q", "w", "e", "r", "t", "y", "u", "i", "o", "p", "[", "]"],
    ["a", "s", "d", "f", "g", "h", "j", "k", "l", ";", "'", "\\"],
    ["z", "x", "c", "v", "b", "n", "m", ",", ".", "/", "Shift", "Space"],
]


class VirtualKeyboard(Gtk.Box):
    def __init__(self, on_text_typed=None):
        super().__init__(orientation=Gtk.Orientation.VERTICAL, spacing=6)
        self._on_text_typed = on_text_typed
        self._upper = False
        self._buttons = []
        self._cursor_x = 0
        self._cursor_y = 0
        self._shift_selected = False

        title = Gtk.Label(label="Virtual Keyboard", css_classes=["title-2"])
        title.set_margin_bottom(6)
        self.append(title)

        for row_idx, row_keys in enumerate(_LAYOUT):
            row_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=4)
            row_box.set_halign(Gtk.Align.CENTER)
            for col_idx, key in enumerate(row_keys):
                btn = Gtk.Button(label=key, css_classes=["flat"])
                if key in ["Shift", "Space"]:
                    btn.set_css_classes(["suggested-action"])
                row_box.append(btn)
                self._buttons.append((row_idx, col_idx, btn, key))
            self.append(row_box)

        self._update_focus()

    # ---- Controller API ---------------------------------------------------

    def on_dpad(self, direction):
        rows = len(_LAYOUT)
        cols = max(len(r) for r in _LAYOUT)
        if direction == "dpad_up":
            self._cursor_y = max(0, self._cursor_y - 1)
        elif direction == "dpad_down":
            self._cursor_y = min(rows - 1, self._cursor_y + 1)
        elif direction == "dpad_left":
            self._cursor_x = max(0, self._cursor_x - 1)
        elif direction == "dpad_right":
            self._cursor_x = min(cols - 1, self._cursor_x + 1)
        self._update_focus()

    def on_confirm(self):
        btn = self._find_button(self._cursor_x, self._cursor_y)
        if not btn:
            return
        key = _LAYOUT[self._cursor_y][self._cursor_x]
        if key == "Shift":
            self._toggle_shift()
            return
        if key == "Space":
            text = " "
        else:
            text = key.upper() if self._upper else key
            if self._shift_selected:
                self._toggle_shift()
                self._shift_selected = False
        if self._on_text_typed:
            self._on_text_typed(text)

    def on_backspace(self):
        if self._on_text_typed:
            self._on_text_typed("\b")

    def on_toggle_shift(self):
        self._shift_selected = not self._shift_selected
        self._toggle_shift()

    def on_submit(self):
        if self._on_text_typed:
            self._on_text_typed("\n")

    # ---- Private ----------------------------------------------------------

    def _toggle_shift(self):
        self._upper = not self._upper
        for _, _, btn, key in self._buttons:
            if key not in ["Shift", "Space"]:
                btn.set_label(key.upper() if self._upper else key)

    def _find_button(self, x, y):
        for ry, rx, btn, key in self._buttons:
            if rx == x and ry == y:
                return btn
        return None

    def _update_focus(self):
        btn = self._find_button(self._cursor_x, self._cursor_y)
        if btn:
            btn.grab_focus()
