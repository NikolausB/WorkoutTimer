from gi.repository import Adw, Gtk, Gio, GLib
from round_timer import RoundTimerPage
from training_plan import TrainingPlanPage
from history import HistoryPage
from data_store import DataStore


class MainWindow(Adw.ApplicationWindow):
    def __init__(self, **kwargs):
        super().__init__(**kwargs)
        self.set_title("Training")
        self.set_default_size(600, 700)

        self._store = DataStore()
        self._store.seed_default_plans()

        self._round_timer = RoundTimerPage()
        self._training_plan = TrainingPlanPage(self._store)
        self._history = HistoryPage(self._store)

        self._stack = Adw.ViewStack()

        self._stack.add_titled(self._round_timer, "timer", "Round Timer")
        self._stack.add_titled(self._training_plan, "plans", "Training Plans")
        self._stack.add_titled(self._history, "history", "History")

        switcher = Adw.ViewSwitcher(stack=self._stack, policy=Adw.ViewSwitcherPolicy.WIDE)

        self._prefs_btn = Gtk.Button(icon_name="preferences-system-symbolic", css_classes=["flat"])
        self._prefs_btn.set_tooltip_text("Preferences")
        self._prefs_btn.connect("clicked", self._on_preferences_clicked)

        self._fullscreen_btn = Gtk.Button(icon_name="view-fullscreen-symbolic", css_classes=["flat"])
        self._fullscreen_btn.connect("clicked", self._on_toggle_fullscreen)

        header_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
        header_box.append(switcher)
        header_box.append(self._prefs_btn)
        header_box.append(self._fullscreen_btn)

        toolbar = Adw.ToolbarView()
        toolbar.add_top_bar(header_box)
        toolbar.set_content(self._stack)

        self.set_content(toolbar)

        self._stack.connect("notify::visible-child", self._on_tab_switched)

        action = Gio.SimpleAction.new("toggle-fullscreen", None)
        action.connect("activate", lambda *_: self._on_toggle_fullscreen(None))
        self.add_action(action)

    def _on_tab_switched(self, stack, param):
        child = stack.get_visible_child()
        if child == self._history:
            self._history.refresh()

    def _on_toggle_fullscreen(self, btn):
        if self.is_fullscreen():
            self.unfullscreen()
            self._fullscreen_btn.set_icon_name("view-fullscreen-symbolic")
        else:
            self.fullscreen()
            self._fullscreen_btn.set_icon_name("view-restore-symbolic")

    def _on_preferences_clicked(self, btn):
        from preferences import PreferencesDialog
        dialog = PreferencesDialog()
        dialog.present(self)