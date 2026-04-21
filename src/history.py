import os
from gi.repository import Adw, Gtk, GLib
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

        header_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12)
        header_box.set_valign(Gtk.Align.CENTER)

        header = Gtk.Label(label="Training History", css_classes=["title-1"])
        header.set_hexpand(True)
        header.set_halign(Gtk.Align.START)
        header_box.append(header)

        export_btn = Gtk.Button(label="Export", icon_name="document-save-symbolic", css_classes=["flat"])
        export_btn.set_tooltip_text("Export history to CSV")
        export_btn.connect("clicked", self._on_export_clicked)
        header_box.append(export_btn)

        import_btn = Gtk.Button(label="Import", icon_name="document-open-symbolic", css_classes=["flat"])
        import_btn.set_tooltip_text("Import history from CSV")
        import_btn.connect("clicked", self._on_import_clicked)
        header_box.append(import_btn)

        box.append(header_box)

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
        show_rounds = session.total_rounds > 1
        group_title = "Exercises"
        if show_rounds:
            rbr_str = f", {session.rest_between_rounds_seconds}s rest between" if session.rest_between_rounds_seconds > 0 else ""
            group_title = f"Exercises ({session.total_rounds} rounds{rbr_str})"
        group.set_title(group_title)

        for ex_log in session.exercises:
            title = ex_log.exercise_name
            if show_rounds and ex_log.round_number > 1:
                title = f"R{ex_log.round_number}: {ex_log.exercise_name}"
            row = Adw.ActionRow(title=title)

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

    def _on_export_clicked(self, btn):
        from csv_io import export_history_csv

        chooser = Gtk.FileChooserNative(
            title="Export Training History",
            transient_for=self.get_native(),
            action=Gtk.FileChooserAction.SAVE,
        )
        filter_csv = Gtk.FileFilter()
        filter_csv.add_pattern("*.csv")
        filter_csv.set_name("CSV files")
        chooser.add_filter(filter_csv)
        chooser.set_current_name("training-history.csv")

        def on_response(chooser, response_id):
            if response_id == Gtk.ResponseType.ACCEPT:
                path = chooser.get_file().get_path()
                try:
                    self._sessions = self._store.load_sessions()
                    export_history_csv(self._sessions, path)
                except Exception as e:
                    dialog = Adw.AlertDialog()
                    dialog.set_heading("Export failed")
                    dialog.set_body(str(e))
                    dialog.add_response("ok", "OK")
                    dialog.present(self.get_native())
            chooser.destroy()

        chooser.connect("response", on_response)
        chooser.show()

    def _on_import_clicked(self, btn):
        from csv_io import import_history_csv

        chooser = Gtk.FileChooserNative(
            title="Import Training History",
            transient_for=self.get_native(),
            action=Gtk.FileChooserAction.OPEN,
        )
        filter_csv = Gtk.FileFilter()
        filter_csv.add_pattern("*.csv")
        filter_csv.set_name("CSV files")
        chooser.add_filter(filter_csv)

        def on_response(chooser, response_id):
            if response_id == Gtk.ResponseType.ACCEPT:
                path = chooser.get_file().get_path()
                try:
                    if not os.path.exists(path):
                        chooser.destroy()
                        return
                    imported = import_history_csv(path)
                    if not imported:
                        chooser.destroy()
                        return
                    existing_ids = {s.id for s in self._store.load_sessions()}
                    new_sessions = [s for s in imported if s.id not in existing_ids]
                    if new_sessions:
                        sessions = self._store.load_sessions()
                        sessions.extend(new_sessions)
                        self._store.save_sessions(sessions)
                        self.refresh()
                except Exception as e:
                    dialog = Adw.AlertDialog()
                    dialog.set_heading("Import failed")
                    dialog.set_body(str(e))
                    dialog.add_response("ok", "OK")
                    dialog.present(self.get_native())
            chooser.destroy()

        chooser.connect("response", on_response)
        chooser.show()