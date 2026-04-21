from gi.repository import Adw, Gtk, GObject
from timer_core import TimerCore
from sound import sound_player
from settings import app_settings
from models import RoundConfig


class RoundTimerPage(Adw.Bin):
    def __init__(self, **kwargs):
        super().__init__(**kwargs)

        self._config = RoundConfig()
        self._timer = TimerCore()
        self._timer.on_tick = self._on_timer_tick
        self._timer.on_finished = self._on_timer_finished
        self._current_round = 0
        self._is_pause = False
        self._total_rounds = 0

        self._build_ui()

    def _build_ui(self):
        main_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12, vexpand=True)
        main_box.set_margin_top(24)
        main_box.set_margin_bottom(24)
        main_box.set_margin_start(24)
        main_box.set_margin_end(24)

        config_group = Adw.PreferencesGroup()
        config_group.set_title("Configuration")

        self._rounds_spin = self._add_spin_row(config_group, "Rounds", 1, 50, 10, 1)
        self._round_time_spin = self._add_spin_row(config_group, "Round duration (seconds)", 10, 3600, 180, 10)
        self._pause_time_spin = self._add_spin_row(config_group, "Pause duration (seconds)", 0, 600, 60, 5)

        main_box.append(config_group)

        self._round_label = Gtk.Label(label="Round 0 / 0")
        self._round_label.add_css_class("title-2")
        self._round_label.set_margin_top(18)
        main_box.append(self._round_label)

        self._phase_label = Gtk.Label(label="")
        self._phase_label.add_css_class("heading")
        main_box.append(self._phase_label)

        self._countdown_label = Gtk.Label(label="00:00", css_classes=["timer-display"])
        self._countdown_label.set_vexpand(True)
        self._countdown_label.set_valign(Gtk.Align.CENTER)
        main_box.append(self._countdown_label)

        btn_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12, halign=Gtk.Align.CENTER)
        btn_box.set_margin_bottom(12)

        self._start_btn = Gtk.Button(label="Start", css_classes=["suggested-action"])
        self._start_btn.connect("clicked", self._on_start_clicked)
        btn_box.append(self._start_btn)

        self._pause_btn = Gtk.Button(label="Pause", sensitive=False)
        self._pause_btn.connect("clicked", self._on_pause_clicked)
        btn_box.append(self._pause_btn)

        self._reset_btn = Gtk.Button(label="Reset", sensitive=False)
        self._reset_btn.connect("clicked", self._on_reset_clicked)
        btn_box.append(self._reset_btn)

        self._skip_btn = Gtk.Button(label="Skip", sensitive=False, css_classes=["destructive-action"])
        self._skip_btn.connect("clicked", self._on_skip_clicked)
        btn_box.append(self._skip_btn)

        main_box.append(btn_box)
        self.set_child(main_box)

    def _add_spin_row(self, group, title, lower, upper, value, step):
        adj = Gtk.Adjustment(value=value, lower=lower, upper=upper, step_increment=step)
        row = Adw.SpinRow(title=title, adjustment=adj)
        group.add(row)
        return row

    def _format_time(self, seconds: float) -> str:
        total = max(0, int(seconds))
        mins = total // 60
        secs = total % 60
        return f"{mins:02d}:{secs:02d}"

    def _on_start_clicked(self, btn):
        if self._timer.is_running:
            return

        if self._current_round == 0:
            self._config.rounds = int(self._rounds_spin.get_value())
            self._config.round_seconds = int(self._round_time_spin.get_value())
            self._config.pause_seconds = int(self._pause_time_spin.get_value())
            self._total_rounds = self._config.rounds
            self._current_round = 1
            self._is_pause = False
            self._start_round()
        else:
            self._timer.resume()

        self._update_buttons_running(True)

    def _start_round(self):
        self._round_label.set_label(f"Round {self._current_round} / {self._total_rounds}")
        self._phase_label.set_label("FIGHT")
        self._phase_label.remove_css_class("dim-label")
        sound_player.play_sound(app_settings.get_sound("round_start_sound"))
        self._timer.start(self._config.round_seconds)

    def _start_pause(self):
        self._phase_label.set_label("PAUSE")
        self._phase_label.add_css_class("dim-label")
        if self._config.pause_seconds > 0:
            sound_player.play_sound(app_settings.get_sound("round_end_sound"))
            self._timer.start(self._config.pause_seconds)
        else:
            self._advance_round()

    def _advance_round(self):
        self._current_round += 1
        if self._current_round > self._total_rounds:
            self._on_training_complete()
        else:
            self._start_round()

    def _on_timer_tick(self, remaining, total):
        self._countdown_label.set_label(self._format_time(remaining))

    def _on_timer_finished(self):
        sound_player.play_sound(app_settings.get_sound("countdown_tick_sound"))
        if self._is_pause:
            self._is_pause = False
            self._advance_round()
        else:
            if self._current_round >= self._total_rounds:
                self._on_training_complete()
            elif self._config.pause_seconds > 0:
                self._is_pause = True
                self._start_pause()
            else:
                self._advance_round()

    def _on_training_complete(self):
        self._phase_label.set_label("COMPLETE!")
        self._phase_label.remove_css_class("dim-label")
        self._countdown_label.set_label("00:00")
        self._current_round = 0
        self._update_buttons_running(False)
        sound_player.play_sound(app_settings.get_sound("training_complete_sound"))

    def _on_pause_clicked(self, btn):
        if self._timer.is_running:
            self._timer.pause()
            self._pause_btn.set_label("Resume")
        else:
            self._timer.resume()
            self._pause_btn.set_label("Pause")

    def _on_reset_clicked(self, btn):
        self._timer.stop()
        self._current_round = 0
        self._is_pause = False
        self._countdown_label.set_label("00:00")
        self._round_label.set_label("Round 0 / 0")
        self._phase_label.set_label("")
        self._update_buttons_running(False)

    def _on_skip_clicked(self, btn):
        self._timer.stop()
        sound_player.play_sound(app_settings.get_sound("countdown_tick_sound"))
        if self._is_pause:
            self._is_pause = False
            self._advance_round()
        else:
            if self._current_round >= self._total_rounds:
                self._on_training_complete()
            elif self._config.pause_seconds > 0:
                self._is_pause = True
                self._start_pause()
            else:
                self._advance_round()

    def _update_buttons_running(self, running: bool):
        self._pause_btn.set_sensitive(running)
        self._reset_btn.set_sensitive(running or self._current_round > 0)
        self._skip_btn.set_sensitive(running)
        if not running:
            self._pause_btn.set_label("Pause")
        self._rounds_spin.set_sensitive(not running and self._current_round == 0)
        self._round_time_spin.set_sensitive(not running and self._current_round == 0)
        self._pause_time_spin.set_sensitive(not running and self._current_round == 0)