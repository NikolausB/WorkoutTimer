from gi.repository import Adw, Gtk, Gio, GLib
from round_timer import RoundTimerPage
from training_plan import TrainingPlanPage
from history import HistoryPage
from home import HomePage
from ai_coach import AICoachPage
from data_store import DataStore
from settings import app_settings


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
        self._home = HomePage(self._store, self._training_plan)
        self._ai_coach = AICoachPage(self._store, on_plan_saved=self._on_ai_plan_saved)

        self._stack = Adw.ViewStack()

        self._home_page = self._stack.add_titled(self._home, "home", "Home")
        self._home_page.set_icon_name("go-home-symbolic")
        self._timer_page = self._stack.add_titled(self._round_timer, "timer", "Round Timer")
        self._timer_page.set_icon_name("preferences-system-time-symbolic")
        self._plans_page = self._stack.add_titled(self._training_plan, "plans", "Training Plans")
        self._plans_page.set_icon_name("view-list-symbolic")
        self._ai_page = self._stack.add_titled(self._ai_coach, "ai", "AI Coach")
        self._ai_page.set_icon_name("computer-symbolic")
        self._history_page = self._stack.add_titled(self._history, "history", "History")
        self._history_page.set_icon_name("document-open-recent-symbolic")

        self._home._on_switch_to_plans = lambda: self._stack.set_visible_child_name("plans")

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

        self._rebuild_tabs()
        self._home.refresh()

    def _rebuild_tabs(self):
        show_home = app_settings.show_home_page
        show_timer = app_settings.show_timer_page
        show_workout = app_settings.show_workout_page
        show_ai = app_settings.show_ai_page

        self._home_page.set_visible(show_home)
        self._timer_page.set_visible(show_timer)
        self._plans_page.set_visible(show_workout)
        self._ai_page.set_visible(show_ai)

        if show_home:
            self._stack.set_visible_child_name("home")
        elif show_timer:
            self._stack.set_visible_child_name("timer")
        elif show_workout:
            self._stack.set_visible_child_name("plans")
        elif show_ai:
            self._stack.set_visible_child_name("ai")
        else:
            self._stack.set_visible_child_name("history")

    def _on_ai_plan_saved(self):
        self._training_plan.refresh_plans()
        self._stack.set_visible_child_name("plans")

    def _on_tab_switched(self, stack, param):
        child = stack.get_visible_child()
        if child == self._history:
            self._history.refresh()
        elif child == self._home:
            self._home.refresh()

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
        dialog.connect("closed", lambda *_: self._rebuild_tabs())
        dialog.present(self)