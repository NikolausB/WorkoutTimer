from gi.repository import Adw, Gtk
from models import TrainingSession, ExerciseLog
from data_store import DataStore


class HistoryPage(Adw.Bin):
    def __init__(self, data_store: DataStore, **kwargs):
        super().__init__(**kwargs)
        self._store = data_store
        self._sessions: list[TrainingSession] = []
        self._build_ui()

    def _build_ui(self):
        self._stack = Gtk.Stack(transition_type=Gtk.StackTransitionType.SLIDE_LEFT_RIGHT)
        self._stack.set_vexpand(True)

        self._build_list_view()
        self._build_detail_view()

        self.set_child(self._stack)

    def _build_list_view(self):
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
        box.set_margin_top(24)
        box.set_margin_bottom(24)
        box.set_margin_start(24)
        box.set_margin_end(24)

        header = Gtk.Label(label="Training History", css_classes=["title-1"])
        box.append(header)

        self._list_stack = Gtk.Stack()
        self._list_stack.set_vexpand(True)

        scrolled = Gtk.ScrolledWindow()
        self._session_list_box = Gtk.ListBox(css_classes=["boxed-list"])
        self._session_list_box.set_activate_on_single_click(True)
        self._session_list_box.connect("row-activated", self._on_session_activated)
        scrolled.set_child(self._session_list_box)
        self._list_stack.add_named(scrolled, "list")

        empty_label = Gtk.Label(label="No training sessions yet.", css_classes=["dim-label"])
        empty_label.set_vexpand(True)
        self._list_stack.add_named(empty_label, "empty")

        box.append(self._list_stack)
        self._stack.add_named(box, "main")

    def _build_detail_view(self):
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
        box.set_margin_top(24)
        box.set_margin_bottom(24)
        box.set_margin_start(24)
        box.set_margin_end(24)

        back_btn = Gtk.Button(label="Back")
        back_btn.connect("clicked", lambda _: self._show_list())
        box.append(back_btn)

        self._detail_header = Gtk.Label(label="", css_classes=["title-1"])
        box.append(self._detail_header)

        self._detail_info = Gtk.Label(label="", css_classes=["dim-label"])
        box.append(self._detail_info)

        self._detail_exercises_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6)
        scrolled = Gtk.ScrolledWindow(vexpand=True)
        scrolled.set_child(self._detail_exercises_box)
        box.append(scrolled)

        self._delete_btn = Gtk.Button(label="Delete Session", css_classes=["destructive-action"])
        self._delete_btn.connect("clicked", self._on_delete_current_session)
        box.append(self._delete_btn)

        self._stack.add_named(box, "detail")

    def refresh(self):
        self._sessions = self._store.load_sessions()

        row = self._session_list_box.get_row_at_index(0)
        while row is not None:
            self._session_list_box.remove(row)
            row = self._session_list_box.get_row_at_index(0)

        if not self._sessions:
            self._list_stack.set_visible_child_name("empty")
            return

        for session in self._sessions:
            self._add_session_row(session)

        self._list_stack.set_visible_child_name("list")

    def _add_session_row(self, session: TrainingSession):
        planned_min = session.total_planned_seconds // 60
        planned_sec = session.total_planned_seconds % 60
        actual_min = session.total_actual_seconds // 60
        actual_sec = session.total_actual_seconds % 60
        date_str = session.started_at.strftime("%Y-%m-%d %H:%M")

        title = f"{session.plan_name}"
        subtitle = f"{date_str} - Actual: {actual_min:02d}:{actual_sec:02d} / Planned: {planned_min:02d}:{planned_sec:02d}"

        list_row = Adw.ActionRow(title=title, subtitle=subtitle)
        list_row.set_activatable(True)
        self._session_list_box.append(list_row)

    def _on_session_activated(self, list_box, row):
        idx = row.get_index()
        if idx < 0 or idx >= len(self._sessions):
            return
        session = self._sessions[idx]
        self._show_detail(session)

    def _on_delete_current_session(self, btn):
        if self._current_session_id:
            self._store.delete_session(self._current_session_id)
            self._current_session_id = None
            self._show_list()

    def _show_list(self):
        self.refresh()
        self._stack.set_visible_child_name("main")

    def _show_detail(self, session: TrainingSession):
        self._current_session_id = session.id
        self._detail_header.set_label(session.plan_name)
        date_str = session.started_at.strftime("%Y-%m-%d %H:%M")
        finished_str = session.finished_at.strftime("%H:%M") if session.finished_at else "?"
        planned_min = session.total_planned_seconds // 60
        planned_sec = session.total_planned_seconds % 60
        actual_min = session.total_actual_seconds // 60
        actual_sec = session.total_actual_seconds % 60
        self._detail_info.set_label(
            f"{date_str} - {finished_str} | "
            f"Actual: {actual_min:02d}:{actual_sec:02d} / Planned: {planned_min:02d}:{planned_sec:02d}"
        )

        while child := self._detail_exercises_box.get_first_child():
            self._detail_exercises_box.remove(child)

        group = Adw.PreferencesGroup()
        group.set_title("Exercises")

        for ex_log in session.exercises:
            row = Adw.ActionRow(title=ex_log.exercise_name)

            if ex_log.planned_duration_seconds is not None:
                detail = f"Time: {self._fmt_dur(ex_log.actual_duration_seconds)} / {self._fmt_dur(ex_log.planned_duration_seconds)}"
            elif ex_log.planned_reps is not None:
                actual = ex_log.actual_reps if ex_log.actual_reps is not None else "?"
                detail = f"Reps: {actual} / {ex_log.planned_reps}"
            else:
                detail = ""

            rest_info = f"Rest: {self._fmt_dur(ex_log.actual_rest_seconds)} / {self._fmt_dur(ex_log.rest_seconds)}"
            if detail:
                row.set_subtitle(f"{detail} | {rest_info}")
            else:
                row.set_subtitle(rest_info)

            if not ex_log.completed:
                row.add_css_class("error")

            status_label = Gtk.Label(label="Done" if ex_log.completed else "Skipped")
            row.add_suffix(status_label)

            group.add(row)

        self._detail_exercises_box.append(group)
        self._stack.set_visible_child_name("detail")

    @staticmethod
    def _fmt_dur(seconds) -> str:
        if seconds is None:
            return "--:--"
        s = max(0, int(seconds))
        return f"{s // 60:02d}:{s % 60:02d}"