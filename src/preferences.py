from gi.repository import Adw, Gtk
from settings import app_settings, save_settings, AppSettings, DEFAULT_SETTINGS
from sound import sound_player, AVAILABLE_SOUNDS


_DISPLAY_NAMES = {
    "beep": "Beep",
    "round_start": "Ding (Round Start)",
    "round_end": "Bell (Round End)",
    "exercise_complete": "Chime (Exercise Complete)",
    "training_complete": "Fanfare (Training Complete)",
    "none": "None (Silent)",
}

_EVENT_LABELS = {
    "round_start_sound": "Round Start Sound",
    "round_end_sound": "Round End / Pause Sound",
    "countdown_tick_sound": "Countdown Tick Sound",
    "exercise_complete_sound": "Exercise Complete Sound",
    "training_complete_sound": "Training Complete Sound",
}

_EVENT_SUBTITLES = {
    "round_start_sound": "Played when a round or exercise begins",
    "round_end_sound": "Played when a round ends or rest starts",
    "countdown_tick_sound": "Played on timer countdown transitions",
    "exercise_complete_sound": "Played when a single exercise is completed",
    "training_complete_sound": "Played when the entire training session finishes",
}

_SOUND_OPTIONS = list(AVAILABLE_SOUNDS) + ["none"]


class PreferencesDialog(Adw.Dialog):
    def __init__(self, **kwargs):
        super().__init__(**kwargs)
        self.set_title("Preferences")
        self.set_content_width(400)
        self.set_content_height(500)

        self._settings = AppSettings(**app_settings.to_dict())

        toolbar = Adw.ToolbarView()
        header = Adw.HeaderBar()
        toolbar.add_top_bar(header)

        scrolled = Gtk.ScrolledWindow(vexpand=True)
        clamp = Adw.Clamp()
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
        box.set_margin_top(12)
        box.set_margin_bottom(12)
        box.set_margin_start(12)
        box.set_margin_end(12)

        sound_group = Adw.PreferencesGroup(title="Sounds")
        sound_group.set_description("Choose sounds played during training events")

        self._sound_switch = Adw.SwitchRow(title="Sound Enabled")
        self._sound_switch.set_active(self._settings.sound_enabled)
        self._sound_switch.connect("notify::active", self._on_sound_enabled_toggled)
        sound_group.add(self._sound_switch)

        self._combo_rows: dict[str, Adw.ComboRow] = {}
        self._preview_rows: dict[str, Gtk.Button] = {}

        string_list = Gtk.StringList.new([_DISPLAY_NAMES.get(s, s) for s in _SOUND_OPTIONS])

        for event_key in _EVENT_LABELS:
            row = Adw.ComboRow(
                title=_EVENT_LABELS[event_key],
                subtitle=_EVENT_SUBTITLES[event_key],
            )
            row.set_model(string_list)

            current = getattr(self._settings, event_key)
            idx = self._sound_key_to_index(current)
            row.set_selected(idx)
            row.connect("notify::selected", self._on_combo_changed, event_key)
            row.set_sensitive(self._settings.sound_enabled)

            preview_btn = Gtk.Button(icon_name="media-playback-start-symbolic", css_classes=["flat"])
            preview_btn.set_valign(Gtk.Align.CENTER)
            preview_btn.connect("clicked", self._on_preview_clicked, event_key)
            preview_btn.set_sensitive(self._settings.sound_enabled)

            row.add_suffix(preview_btn)

            self._combo_rows[event_key] = row
            self._preview_rows[event_key] = preview_btn
            sound_group.add(row)

        box.append(sound_group)

        display_group = Adw.PreferencesGroup(title="Display")
        display_group.set_description("Control what is shown during workouts")

        self._images_switch = Adw.SwitchRow(
            title="Show Exercise Images",
            subtitle="Display exercise images during workouts and in summaries",
        )
        self._images_switch.set_active(self._settings.show_exercise_images)
        display_group.add(self._images_switch)

        box.append(display_group)

        tabs_group = Adw.PreferencesGroup(title="Tabs")
        tabs_group.set_description("Choose which tabs are shown in the main view")

        self._home_switch = Adw.SwitchRow(
            title="Show Home Page",
            subtitle="Landing page with recent and recommended workouts",
        )
        self._home_switch.set_active(self._settings.show_home_page)
        tabs_group.add(self._home_switch)

        self._timer_switch = Adw.SwitchRow(
            title="Show Round Timer",
            subtitle="Configurable round timer with pause periods",
        )
        self._timer_switch.set_active(self._settings.show_timer_page)
        tabs_group.add(self._timer_switch)

        self._workout_switch = Adw.SwitchRow(
            title="Show Training Plans",
            subtitle="Training plan builder and runner",
        )
        self._workout_switch.set_active(self._settings.show_workout_page)
        tabs_group.add(self._workout_switch)

        self._ai_switch = Adw.SwitchRow(
            title="Show AI Coach",
            subtitle="AI-powered training plan generator (requires network access)",
        )
        self._ai_switch.set_active(self._settings.show_ai_page)
        tabs_group.add(self._ai_switch)

        box.append(tabs_group)

        save_btn = Gtk.Button(label="Save", css_classes=["suggested-action"], halign=Gtk.Align.END)
        save_btn.connect("clicked", self._on_save_clicked)
        box.append(save_btn)

        clamp.set_child(box)
        scrolled.set_child(clamp)
        toolbar.set_content(scrolled)
        self.set_child(toolbar)

    def _sound_key_to_index(self, key: str) -> int:
        for i, s in enumerate(_SOUND_OPTIONS):
            if s == key:
                return i
        return 0

    def _index_to_sound_key(self, index: int) -> str:
        if 0 <= index < len(_SOUND_OPTIONS):
            return _SOUND_OPTIONS[index]
        return "beep"

    def _on_sound_enabled_toggled(self, switch, param):
        active = switch.get_active()
        self._settings.sound_enabled = active
        for event_key in self._combo_rows:
            self._combo_rows[event_key].set_sensitive(active)
            self._preview_rows[event_key].set_sensitive(active)

    def _on_combo_changed(self, combo_row, param, event_key):
        idx = combo_row.get_selected()
        key = self._index_to_sound_key(idx)
        setattr(self._settings, event_key, key)

    def _on_preview_clicked(self, btn, event_key):
        active = self._settings.sound_enabled
        if not active:
            return
        key = getattr(self._settings, event_key)
        if key == "none":
            return
        sound_player.play_sound(key)

    def _on_save_clicked(self, btn):
        from settings import save_settings, app_settings as global_settings
        global_settings.sound_enabled = self._settings.sound_enabled
        for event_key in _EVENT_LABELS:
            setattr(global_settings, event_key, getattr(self._settings, event_key))
        global_settings.show_exercise_images = self._images_switch.get_active()
        global_settings.show_home_page = self._home_switch.get_active()
        global_settings.show_timer_page = self._timer_switch.get_active()
        global_settings.show_workout_page = self._workout_switch.get_active()
        global_settings.show_ai_page = self._ai_switch.get_active()
        save_settings(global_settings)
        self.close()