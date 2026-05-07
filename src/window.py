from gi.repository import Adw, Gtk, Gio, GLib, Gdk
from round_timer import RoundTimerPage
from training_plan import TrainingPlanPage
from history import HistoryPage
from home import HomePage
from ai_coach import AICoachPage
from data_store import DataStore
from settings import app_settings
from ui_scaling import apply_scaling
from controller_manager import ControllerManager
from controller_hints import ControllerHintsOverlay
from textwrap import dedent
import logging

_log = logging.getLogger("window")
_log.setLevel(logging.DEBUG)
if not _log.handlers:
    _log.addHandler(logging.NullHandler())


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
        self._stack.set_vhomogeneous(True)

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

        header = Adw.HeaderBar()
        header.set_title_widget(switcher)
        header.pack_end(self._prefs_btn)

        toolbar = Adw.ToolbarView()
        toolbar.add_top_bar(header)

        self._hints_overlay = ControllerHintsOverlay()
        overlay = Gtk.Overlay()
        overlay.set_child(self._stack)
        overlay.add_overlay(self._hints_overlay)
        toolbar.set_content(overlay)

        self.set_content(toolbar)

        self._stack.connect("notify::visible-child", self._on_tab_switched)

        self._rebuild_tabs()
        self._home.refresh()

        self.connect("notify::is-fullscreen", self._on_window_resize)
        self.connect("notify::is-maximized", self._on_window_resize)
        self.connect("realize", self._on_realize)

        # Controller — initialised lazily in _on_realize() so the GTK main
        # loop is already running (required by libmanette's udev monitor).
        self._controller = None
        self._open_dialog = None
        self._deck_baseline = None

    # ---------- Controller Handling ----------

    def _is_deck_mode(self):
        if self._controller is None:
            return False
        mode = app_settings.deck_mode
        if mode == "on":
            return True
        if mode == "off":
            return False
        return self._controller.deck_mode

    def _apply_controller_css(self):
        display = Gdk.Display.get_default()
        if display is None:
            return
        css = dedent("""
            .controller-focus {
                outline: 3px solid @accent_bg_color;
                outline-offset: -1px;
            }
        """)
        provider = Gtk.CssProvider()
        provider.load_from_data(css.encode())
        Gtk.StyleContext.add_provider_for_display(display, provider, Gtk.STYLE_PROVIDER_PRIORITY_USER)

    def _apply_deck_css(self):
        display = Gdk.Display.get_default()
        if display is None:
            return
        css = dedent("""
            .deck-mode button {
                min-width: 64px;
                min-height: 52px;
            }
            .deck-mode .controller-hints-label {
                font-size: 18px;
                padding: 14px 20px;
                border-radius: 24px;
                background: rgba(0,0,0,0.45);
                color: white;
            }
        """)
        provider = Gtk.CssProvider()
        provider.load_from_data(css.encode())
        Gtk.StyleContext.add_provider_for_display(display, provider, Gtk.STYLE_PROVIDER_PRIORITY_USER)
        self.add_css_class("deck-mode")

    def _on_controller_connected(self, manager):
        self._update_hints_visibility()

    def _on_controller_disconnected(self, manager):
        self._hints_overlay.hide()

    def _update_hints_visibility(self):
        if self._controller is None or not app_settings.gamepad_enabled or not self._controller.connected:
            self._hints_overlay.hide()
            return
        if not app_settings.gamepad_hints:
            self._hints_overlay.hide()
            return
        child = self._stack.get_visible_child()
        if child is None:
            return
        if child == self._round_timer:
            self._hints_overlay.set_context("timer")
        elif child == self._training_plan:
            ctx = self._training_plan.get_controller_context()
            sub = self._training_plan.get_controller_sub_key()
            self._hints_overlay.set_sub_key(sub)
            self._hints_overlay.set_context(ctx)
        elif child == self._home:
            self._hints_overlay.set_context("list")
        elif child == self._history:
            self._hints_overlay.set_context("list")
        elif child == self._ai_coach:
            self._hints_overlay.set_context("ai_coach")
        else:
            self._hints_overlay.hide()

    def _on_controller_action(self, manager, action):
        if not app_settings.gamepad_enabled:
            return

        if action == "l1":
            self._switch_tab(-1)
            return
        if action == "r1":
            self._switch_tab(1)
            return
        if action == "guide":
            GLib.idle_add(lambda: (self.unfullscreen() if self.is_fullscreen() else self.fullscreen(), GLib.SOURCE_REMOVE))
            return

        if self._open_dialog is not None:
            dialog, confirm_id, cancel_id = self._open_dialog
            method_name = f"controller_{action}"
            if hasattr(dialog, method_name):
                getattr(dialog, method_name)()
            elif action == "a" and confirm_id:
                dialog.emit("response", confirm_id)
                dialog.close()
            elif action == "b" and cancel_id:
                dialog.emit("response", cancel_id)
                dialog.close()
            elif action == "b":
                dialog.close()
            return

        child = self._stack.get_visible_child()
        if action == "select":
            if getattr(child, "controller_select", None):
                child.controller_select()
            else:
                self._on_preferences_clicked(None)
        elif action == "dpad_up":
            if getattr(child, "controller_dpad_up", None):
                child.controller_dpad_up()
            else:
                self._move_focus(Gtk.DirectionType.UP)
        elif action == "dpad_down":
            if getattr(child, "controller_dpad_down", None):
                child.controller_dpad_down()
            else:
                self._move_focus(Gtk.DirectionType.DOWN)
        elif action == "dpad_left":
            if getattr(child, "controller_dpad_left", None):
                child.controller_dpad_left()
            else:
                self._move_focus(Gtk.DirectionType.LEFT)
        elif action == "dpad_right":
            if getattr(child, "controller_dpad_right", None):
                child.controller_dpad_right()
            else:
                self._move_focus(Gtk.DirectionType.RIGHT)
        elif action == "a":
            if hasattr(child, "controller_a"):
                child.controller_a()
            else:
                self._activate_focused()
        elif action == "b":
            if hasattr(child, "controller_back"):
                child.controller_back()
        elif action == "start":
            self._handle_start()
        elif action == "y":
            self._handle_y()
        elif action == "x":
            self._handle_x()
        elif action == "l2":
            if hasattr(child, "controller_scroll_up"):
                child.controller_scroll_up()
        elif action == "r2":
            if hasattr(child, "controller_scroll_down"):
                child.controller_scroll_down()

    def _register_controller_dialog(self, dialog, confirm_id=None, cancel_id=None):
        self._open_dialog = (dialog, confirm_id, cancel_id)
        try:
            dialog.connect("response", lambda d, r: self._clear_controller_dialog())
        except TypeError:
            dialog.connect("closed", lambda d: self._clear_controller_dialog())

    def _clear_controller_dialog(self):
        self._open_dialog = None

    def _switch_tab(self, delta):
        pages = [
            self._home_page,
            self._timer_page,
            self._plans_page,
            self._ai_page,
            self._history_page,
        ]
        visible = [p for p in pages if p.get_visible()]
        current_child = self._stack.get_visible_child()
        current_page = next((p for p in visible if p.get_child() == current_child), None)
        if current_page is None:
            return
        idx = visible.index(current_page)
        next_idx = (idx + delta) % len(visible)
        self._stack.set_visible_child_name(visible[next_idx].get_name())

    def _move_focus(self, direction):
        focus = self.get_focus()
        if focus is None:
            self._stack.child_focus(Gtk.DirectionType.TAB_FORWARD)
            return
        focus.child_focus(direction)

    def _activate_focused(self):
        focus = self.get_focus()
        if focus is None:
            return
        if isinstance(focus, Gtk.Button) or isinstance(focus, Adw.ActionRow):
            focus.activate()
        elif isinstance(focus, Gtk.Entry):
            focus.grab_focus()
        elif hasattr(focus, "activate"):
            focus.activate()

    def _handle_back(self):
        focus = self.get_focus()
        if focus and isinstance(focus, Gtk.Entry):
            focus.emit("backspace")
            return
        child = self._stack.get_visible_child()
        if hasattr(child, "controller_back"):
            child.controller_back()

    def _handle_start(self):
        child = self._stack.get_visible_child()
        if hasattr(child, "controller_start"):
            child.controller_start()

    def _handle_y(self):
        child = self._stack.get_visible_child()
        if hasattr(child, "controller_y"):
            child.controller_y()

    def _handle_x(self):
        child = self._stack.get_visible_child()
        if hasattr(child, "controller_x"):
            child.controller_x()
        else:
            # Default: toggle sound
            app_settings.sound_enabled = not app_settings.sound_enabled

    # ---------- Existing methods ----------

    def _font_dims(self):
        if self._deck_baseline is not None:
            return self._deck_baseline
        return (self.get_width(), self.get_height())

    def _on_realize(self, *args):
        _log.info("Window realized — initialising controller")
        self._apply_controller_css()
        self._controller = ControllerManager()
        self._controller.connect("action_activated", self._on_controller_action)
        self._controller.connect("device_connected", self._on_controller_connected)
        self._controller.connect("device_disconnected", self._on_controller_disconnected)

        if self._is_deck_mode():
            self._apply_deck_css()
            self._deck_baseline = (1280, 800)
            GLib.idle_add(self.fullscreen)

        GLib.idle_add(self._initial_font_update)

    def _initial_font_update(self):
        w, h = self._font_dims()
        if w <= 0 or h <= 0:
            return GLib.SOURCE_CONTINUE
        child = self._stack.get_visible_child()
        if child == self._round_timer:
            self._round_timer.update_fonts(w, h)
        elif child == self._training_plan:
            self._training_plan.update_fonts(w, h)
        return GLib.SOURCE_REMOVE

    def _on_window_resize(self, *args):
        w, h = self._font_dims()
        if w <= 0 or h <= 0:
            return
        child = self._stack.get_visible_child()
        if child == self._round_timer:
            self._round_timer.update_fonts(w, h)
        elif child == self._training_plan:
            self._training_plan.update_fonts(w, h)

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

        w, h = self._font_dims()
        if w <= 0 or h <= 0:
            return
        if child == self._round_timer:
            self._round_timer.update_fonts(w, h)
        elif child == self._training_plan:
            self._training_plan.update_fonts(w, h)

        self._update_hints_visibility()

    def _on_preferences_clicked(self, btn):
        from preferences import PreferencesDialog
        dialog = PreferencesDialog()
        self._open_dialog = (dialog, None, None)
        dialog.connect("closed", lambda *_: self._on_dialog_closed())
        dialog.present(self)

    def _on_dialog_closed(self):
        self._open_dialog = None
        self._rebuild_tabs()
