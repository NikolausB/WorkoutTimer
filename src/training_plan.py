from gi.repository import Adw, Gtk, GObject, Gdk
from models import TrainingPlan, Exercise, TrainingSession, ExerciseLog
from data_store import DataStore
from timer_core import TimerCore
from sound import sound_player
from settings import app_settings
from image_utils import load_image_widget, load_thumbnail_widget, copy_user_image, resolve_image_path
from exercise_picker import ExercisePicker
from datetime import datetime


class TrainingPlanPage(Adw.Bin):
    def __init__(self, data_store: DataStore, **kwargs):
        super().__init__(**kwargs)
        self._store = data_store
        self._plans: list[TrainingPlan] = []
        self._running_session: TrainingSession | None = None
        self._running_plan: TrainingPlan | None = None
        self._current_exercise_idx = 0
        self._phase = "exercise"
        self._exercise_start_time: float = 0
        self._timer = TimerCore()
        self._timer.on_tick = self._on_timer_tick
        self._timer.on_finished = self._on_timer_finished
        self._editing_plan_id: str | None = None
        self._editor_exercises: list[Exercise] = []
        self._exercise_rows: list[Adw.ExpanderRow] = []
        self._build_ui()
        self._refresh_plans()

    def _build_ui(self):
        self._stack = Gtk.Stack(transition_type=Gtk.StackTransitionType.SLIDE_LEFT_RIGHT)
        self._stack.set_vexpand(True)

        self._build_list_view()
        self._build_editor_view()
        self._build_runner_view()
        self._build_summary_view()

        self.set_child(self._stack)

    def _build_list_view(self):
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
        box.set_margin_top(24)
        box.set_margin_bottom(24)
        box.set_margin_start(24)
        box.set_margin_end(24)

        header = Gtk.Label(label="Training Plans", css_classes=["title-1"])
        box.append(header)

        new_btn = Gtk.Button(label="New Plan", css_classes=["suggested-action"], halign=Gtk.Align.START)
        new_btn.connect("clicked", self._on_new_plan)
        box.append(new_btn)

        self._list_stack = Gtk.Stack()
        self._list_stack.set_vexpand(True)

        scrolled = Gtk.ScrolledWindow()
        self._plan_list_box = Gtk.ListBox(css_classes=["boxed-list"])
        self._plan_list_box.set_activate_on_single_click(True)
        self._plan_list_box.connect("row-activated", self._on_plan_activated)
        scrolled.set_child(self._plan_list_box)
        self._list_stack.add_named(scrolled, "list")

        empty_label = Gtk.Label(label="No plans yet. Create one!", css_classes=["dim-label"])
        empty_label.set_vexpand(True)
        self._list_stack.add_named(empty_label, "empty")

        box.append(self._list_stack)
        self._stack.add_named(box, "main")

    def _build_editor_view(self):
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
        box.set_margin_top(24)
        box.set_margin_bottom(24)
        box.set_margin_start(24)
        box.set_margin_end(24)

        back_btn = Gtk.Button(label="Back")
        back_btn.connect("clicked", lambda _: self._show_list())
        box.append(back_btn)

        self._plan_name_entry = Adw.EntryRow(title="Plan Name")
        name_group = Adw.PreferencesGroup()
        name_group.add(self._plan_name_entry)
        box.append(name_group)

        exercises_label = Gtk.Label(label="Exercises", css_classes=["title-2"])
        exercises_label.set_margin_top(12)
        box.append(exercises_label)

        self._exercises_group = Adw.PreferencesGroup()
        box.append(self._exercises_group)

        add_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12)
        add_exercise_btn = Gtk.Button(label="Add Exercise", halign=Gtk.Align.START)
        add_exercise_btn.connect("clicked", self._on_add_exercise)
        add_box.append(add_exercise_btn)
        browse_exercise_btn = Gtk.Button(label="Browse Exercises", halign=Gtk.Align.START)
        browse_exercise_btn.connect("clicked", self._on_browse_add_exercise)
        add_box.append(browse_exercise_btn)
        box.append(add_box)

        btn_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12)
        save_btn = Gtk.Button(label="Save Plan", css_classes=["suggested-action"])
        save_btn.connect("clicked", self._on_save_plan)
        delete_btn = Gtk.Button(label="Delete Plan", css_classes=["destructive-action"])
        delete_btn.connect("clicked", self._on_delete_plan)
        run_btn = Gtk.Button(label="Start Training", css_classes=["pill"])
        run_btn.connect("clicked", self._on_run_plan)
        btn_box.append(save_btn)
        btn_box.append(delete_btn)
        btn_box.append(run_btn)
        box.append(btn_box)

        scrolled = Gtk.ScrolledWindow(vexpand=True)
        scrolled.set_child(box)
        self._stack.add_named(scrolled, "editor")

    def _build_runner_view(self):
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
        box.set_margin_top(24)
        box.set_margin_bottom(24)
        box.set_margin_start(24)
        box.set_margin_end(24)

        self._runner_plan_label = Gtk.Label(label="", css_classes=["title-2"])
        box.append(self._runner_plan_label)

        self._runner_image = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, halign=Gtk.Align.CENTER)
        box.append(self._runner_image)

        self._runner_exercise_label = Gtk.Label(label="", css_classes=["title-1"])
        self._runner_exercise_label.set_vexpand(True)
        self._runner_exercise_label.set_valign(Gtk.Align.CENTER)
        box.append(self._runner_exercise_label)

        self._runner_phase_label = Gtk.Label(label="", css_classes=["heading"])
        box.append(self._runner_phase_label)

        self._runner_countdown = Gtk.Label(label="00:00", css_classes=["timer-display"])
        box.append(self._runner_countdown)

        self._runner_next_label = Gtk.Label(label="", css_classes=["dim-label"])
        box.append(self._runner_next_label)

        self._reps_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8, visible=False)
        self._reps_spin = Adw.SpinRow(title="Reps completed")
        rep_adj = Gtk.Adjustment(value=0, lower=0, upper=9999, step_increment=1)
        self._reps_spin.set_adjustment(rep_adj)
        self._reps_box.append(self._reps_spin)
        done_reps_btn = Gtk.Button(label="Done", css_classes=["suggested-action"])
        done_reps_btn.connect("clicked", self._on_reps_done)
        self._reps_box.append(done_reps_btn)
        box.append(self._reps_box)

        btn_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12, halign=Gtk.Align.CENTER)
        self._runner_pause_btn = Gtk.Button(label="Pause")
        self._runner_pause_btn.connect("clicked", self._on_runner_pause)
        self._runner_skip_btn = Gtk.Button(label="Skip", css_classes=["destructive-action"])
        self._runner_skip_btn.connect("clicked", self._on_runner_skip)
        self._runner_stop_btn = Gtk.Button(label="Stop")
        self._runner_stop_btn.connect("clicked", self._on_runner_stop)
        btn_box.append(self._runner_pause_btn)
        btn_box.append(self._runner_skip_btn)
        btn_box.append(self._runner_stop_btn)
        box.append(btn_box)

        self._stack.add_named(box, "runner")

    def _build_summary_view(self):
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
        box.set_margin_top(24)
        box.set_margin_bottom(24)
        box.set_margin_start(24)
        box.set_margin_end(24)

        self._summary_title = Gtk.Label(label="Training Complete!", css_classes=["title-1"])
        box.append(self._summary_title)

        self._summary_plan_name = Gtk.Label(label="", css_classes=["title-2"])
        box.append(self._summary_plan_name)

        self._summary_info = Gtk.Label(label="", css_classes=["dim-label"])
        box.append(self._summary_info)

        self._summary_exercises_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6)
        scrolled = Gtk.ScrolledWindow(vexpand=True)
        scrolled.set_child(self._summary_exercises_box)
        box.append(scrolled)

        back_btn = Gtk.Button(label="Back to Menu", css_classes=["suggested-action", "pill"])
        back_btn.set_halign(Gtk.Align.CENTER)
        back_btn.connect("clicked", lambda _: self._show_list())
        box.append(back_btn)

        self._stack.add_named(box, "summary")

    def _populate_summary(self, session: TrainingSession):
        self._summary_plan_name.set_label(session.plan_name)

        date_str = session.started_at.strftime("%Y-%m-%d %H:%M")
        finished_str = session.finished_at.strftime("%H:%M") if session.finished_at else "?"
        planned_min = session.total_planned_seconds // 60
        planned_sec = session.total_planned_seconds % 60
        actual_min = session.total_actual_seconds // 60
        actual_sec = session.total_actual_seconds % 60
        self._summary_info.set_label(
            f"{date_str} - {finished_str} | "
            f"Actual: {actual_min:02d}:{actual_sec:02d} / Planned: {planned_min:02d}:{planned_sec:02d}"
        )

        while child := self._summary_exercises_box.get_first_child():
            self._summary_exercises_box.remove(child)

        group = Adw.PreferencesGroup()
        group.set_title("Exercises")

        for ex_log in session.exercises:
            row = Adw.ActionRow(title=ex_log.exercise_name)

            thumb = load_thumbnail_widget(ex_log.image_path, 36)
            if thumb:
                row.add_prefix(thumb)

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

        self._summary_exercises_box.append(group)

    @staticmethod
    def _fmt_dur(seconds) -> str:
        if seconds is None:
            return "--:--"
        s = max(0, int(seconds))
        return f"{s // 60:02d}:{s % 60:02d}"

    def _show_list(self):
        self._refresh_plans()
        self._stack.set_visible_child_name("main")

    def _show_editor(self, plan: TrainingPlan | None = None):
        self._editor_exercises = []
        for row in self._exercise_rows:
            self._exercises_group.remove(row)
        self._exercise_rows.clear()

        if plan:
            self._editing_plan_id = plan.id
            self._plan_name_entry.set_text(plan.name)
            self._editor_exercises = [Exercise.from_dict(e.to_dict()) for e in plan.exercises]
        else:
            self._editing_plan_id = None
            self._plan_name_entry.set_text("")

        for ex in self._editor_exercises:
            self._append_exercise_row(ex)

        self._stack.set_visible_child_name("editor")

    def _append_exercise_row(self, exercise: Exercise):
        row = Adw.ExpanderRow(title=exercise.name or "New Exercise")
        row.set_expanded(True)

        name_entry = Adw.EntryRow(title="Name")
        name_entry.set_text(exercise.name)
        row.add_row(name_entry)

        browse_box = Adw.ActionRow(title="Exercise Image")

        thumb = load_thumbnail_widget(exercise.image_path, 36)
        if thumb:
            browse_box.add_prefix(thumb)
        row._thumb_widget = thumb

        browse_btn = Gtk.Button(label="Browse", css_classes=["flat"])
        browse_btn.connect("clicked", lambda _, r=row, e=exercise: self._on_browse_exercise(r, e))
        browse_box.add_suffix(browse_btn)

        pick_btn = Gtk.Button(label="Pick Image", css_classes=["flat"])
        pick_btn.connect("clicked", lambda _, r=row, e=exercise: self._on_pick_custom_image(r, e))
        browse_box.add_suffix(pick_btn)

        row.add_row(browse_box)

        dur_adj = Gtk.Adjustment(value=exercise.duration_seconds or 0, lower=0, upper=3600, step_increment=5)
        dur_spin = Adw.SpinRow(title="Duration (seconds)", adjustment=dur_adj)
        row.add_row(dur_spin)

        reps_adj = Gtk.Adjustment(value=exercise.reps or 0, lower=0, upper=9999, step_increment=1)
        reps_spin = Adw.SpinRow(title="Reps", adjustment=reps_adj)
        row.add_row(reps_spin)

        rest_adj = Gtk.Adjustment(value=exercise.rest_seconds, lower=0, upper=600, step_increment=5)
        rest_spin = Adw.SpinRow(title="Rest after (seconds)", adjustment=rest_adj)
        row.add_row(rest_spin)

        remove_btn = Gtk.Button(label="Remove", css_classes=["destructive-action"])
        remove_btn.set_margin_top(6)
        remove_btn.connect("clicked", lambda _, r=row, e=exercise: self._remove_exercise_row(r, e))
        row.add_row(remove_btn)

        name_entry.connect("changed", lambda *_: self._sync_exercise_from_row(row, name_entry, dur_spin, reps_spin, rest_spin, exercise))

        self._exercise_rows.append(row)
        self._exercises_group.add(row)

    def _on_browse_exercise(self, row, exercise):
        def on_selected(name, image_key):
            for child in self._iter_expander_children(row):
                if isinstance(child, Adw.EntryRow) and child.get_title() == "Name":
                    child.set_text(name)
                    break
            exercise.name = name
            exercise.image_path = image_key
            row.set_title(name or "New Exercise")
            self._update_row_thumbnail(row, image_key)
        ExercisePicker(on_selected=on_selected).present(self.get_root())

    def _on_pick_custom_image(self, row, exercise):
        chooser = Gtk.FileChooserNative(
            title="Select Exercise Image",
            action=Gtk.FileChooserAction.OPEN,
        )
        image_filter = Gtk.FileFilter()
        image_filter.set_name("Images")
        image_filter.add_mime_type("image/*")
        chooser.add_filter(image_filter)
        chooser.connect("response", lambda _, response: self._on_image_selected(chooser, response, row, exercise))
        chooser.show()

    def _on_image_selected(self, chooser, response, row, exercise):
        if response == Gtk.ResponseType.ACCEPT:
            file = chooser.get_file()
            if file:
                path = file.get_path()
                if path:
                    key = copy_user_image(path)
                    exercise.image_path = key
                    self._update_row_thumbnail(row, key)
        chooser.destroy()

    def _update_row_thumbnail(self, row, image_key):
        old_thumb = getattr(row, "_thumb_widget", None)
        if old_thumb and old_thumb.get_parent():
            old_thumb.get_parent().remove(old_thumb)
        new_thumb = load_thumbnail_widget(image_key, 36)
        if new_thumb:
            row._thumb_widget = new_thumb
            for child in self._iter_expander_children(row):
                if isinstance(child, Adw.ActionRow) and child.get_title() == "Exercise Image":
                    child.add_prefix(new_thumb)
                    break

    def _iter_expander_children(self, row):
        children = []
        child = row.get_first_child()
        while child:
            children.append(child)
            child = child.get_next_sibling()
        return children

    def _sync_exercise_from_row(self, row, name_entry, dur_spin, reps_spin, rest_spin, exercise):
        exercise.name = name_entry.get_text()
        row.set_title(exercise.name or "New Exercise")
        dur_val = int(dur_spin.get_value())
        exercise.duration_seconds = dur_val if dur_val > 0 else None
        reps_val = int(reps_spin.get_value())
        exercise.reps = reps_val if reps_val > 0 else None
        exercise.rest_seconds = int(rest_spin.get_value())

    def _remove_exercise_row(self, row, exercise):
        if exercise in self._editor_exercises:
            self._editor_exercises.remove(exercise)
        self._exercises_group.remove(row)
        if row in self._exercise_rows:
            self._exercise_rows.remove(row)

    def _refresh_plans(self):
        row = self._plan_list_box.get_row_at_index(0)
        while row is not None:
            self._plan_list_box.remove(row)
            row = self._plan_list_box.get_row_at_index(0)

        self._plans = self._store.load_plans()

        if not self._plans:
            self._list_stack.set_visible_child_name("empty")
            return

        for plan in self._plans:
            row = Adw.ActionRow(title=plan.name, subtitle=f"{len(plan.exercises)} exercises")
            row.set_activatable(True)
            self._plan_list_box.append(row)

        self._list_stack.set_visible_child_name("list")

    def _on_plan_activated(self, list_box, row):
        idx = row.get_index()
        if idx < 0 or idx >= len(self._plans):
            return
        plan = self._plans[idx]
        if plan:
            self._show_editor(plan)

    def _on_new_plan(self, btn):
        self._show_editor(None)

    def _on_add_exercise(self, btn):
        ex = Exercise(name="", duration_seconds=30, rest_seconds=30)
        self._editor_exercises.append(ex)
        self._append_exercise_row(ex)

    def _on_browse_add_exercise(self, btn):
        def on_selected(name, image_key):
            ex = Exercise(name=name, duration_seconds=30, rest_seconds=30, image_path=image_key)
            self._editor_exercises.append(ex)
            self._append_exercise_row(ex)
        ExercisePicker(on_selected=on_selected).present(self.get_root())

    def _on_save_plan(self, btn):
        name = self._plan_name_entry.get_text().strip()
        if not name:
            return

        if self._editing_plan_id:
            plan = self._store.get_plan(self._editing_plan_id)
            if plan:
                plan.name = name
                plan.exercises = [e for e in self._editor_exercises if e.name.strip()]
                self._store.save_plan(plan)
        else:
            plan = TrainingPlan(name=name, exercises=[e for e in self._editor_exercises if e.name.strip()])
            self._store.save_plan(plan)
            self._editing_plan_id = plan.id

        self._show_list()

    def _on_delete_plan(self, btn):
        if self._editing_plan_id:
            self._store.delete_plan(self._editing_plan_id)
            self._editing_plan_id = None
            self._show_list()

    def _on_run_plan(self, btn):
        name = self._plan_name_entry.get_text().strip()
        if not name or not self._editor_exercises:
            return

        self._on_save_plan(btn)

        plan = None
        if self._editing_plan_id:
            plan = self._store.get_plan(self._editing_plan_id)
        if not plan:
            plans = self._store.load_plans()
            for p in plans:
                if p.name == name:
                    plan = p
                    break

        if not plan or not plan.exercises:
            return

        self._start_training(plan)

    def _start_training(self, plan: TrainingPlan):
        self._running_plan = plan
        self._current_exercise_idx = 0
        self._phase = "exercise"

        self._running_session = TrainingSession(
            plan_id=plan.id,
            plan_name=plan.name,
            total_planned_seconds=plan.total_planned_seconds(),
            exercises=[],
        )

        self._runner_plan_label.set_label(plan.name)
        self._stack.set_visible_child_name("runner")
        self._start_current_exercise()

    def _start_current_exercise(self):
        if self._current_exercise_idx >= len(self._running_plan.exercises):
            self._finish_training()
            return

        ex = self._running_plan.exercises[self._current_exercise_idx]
        self._phase = "exercise"
        self._runner_exercise_label.set_label(ex.name)

        while child := self._runner_image.get_first_child():
            self._runner_image.remove(child)
        pic = load_image_widget(ex.image_path, 200)
        if pic:
            self._runner_image.append(pic)

        next_idx = self._current_exercise_idx + 1
        if next_idx < len(self._running_plan.exercises):
            self._runner_next_label.set_label(f"Next: {self._running_plan.exercises[next_idx].name}")
        else:
            self._runner_next_label.set_label("Last exercise!")

        if ex.is_timed():
            self._reps_box.set_visible(False)
            self._runner_phase_label.set_label("GO!")
            self._exercise_start_time = datetime.now().timestamp()
            sound_player.play_sound(app_settings.get_sound("round_start_sound"))
            self._timer.start(ex.duration_seconds)
        else:
            self._reps_box.set_visible(True)
            self._runner_phase_label.set_label("Reps")
            self._reps_spin.set_value(ex.reps or 0)
            self._exercise_start_time = datetime.now().timestamp()
            self._runner_countdown.set_label("--:--")

    def _start_rest(self):
        ex = self._running_plan.exercises[self._current_exercise_idx]
        if ex.rest_seconds > 0:
            self._phase = "rest"
            self._runner_phase_label.set_label("REST")
            sound_player.play_sound(app_settings.get_sound("round_end_sound"))
            self._timer.start(ex.rest_seconds)
        else:
            self._advance_exercise()

    def _advance_exercise(self):
        self._current_exercise_idx += 1
        self._start_current_exercise()

    def _finish_training(self):
        self._timer.stop()
        self._running_session.finished_at = datetime.now()
        self._running_session.total_actual_seconds = self._running_session.compute_total_actual_seconds()
        self._store.save_session(self._running_session)

        sound_player.play_sound(app_settings.get_sound("training_complete_sound"))

        self._populate_summary(self._running_session)
        self._stack.set_visible_child_name("summary")

        self._running_session = None
        self._running_plan = None

    def _on_timer_tick(self, remaining, total):
        mins = max(0, int(remaining)) // 60
        secs = max(0, int(remaining)) % 60
        self._runner_countdown.set_label(f"{mins:02d}:{secs:02d}")

    def _on_timer_finished(self):
        ex = self._running_plan.exercises[self._current_exercise_idx]
        sound_player.play_sound(app_settings.get_sound("exercise_complete_sound"))

        if self._phase == "exercise":
            elapsed = int(datetime.now().timestamp() - self._exercise_start_time)
            ex_log = ExerciseLog(
                exercise_name=ex.name,
                planned_duration_seconds=ex.duration_seconds,
                actual_duration_seconds=elapsed,
                planned_reps=ex.reps,
                actual_reps=int(self._reps_spin.get_value()) if not ex.is_timed() else None,
                rest_seconds=ex.rest_seconds,
                completed=True,
                image_path=ex.image_path,
            )
            self._running_session.exercises.append(ex_log)
            self._start_rest()
        elif self._phase == "rest":
            ex_log = self._running_session.exercises[-1] if self._running_session.exercises else None
            if ex_log:
                ex_log.actual_rest_seconds = ex.rest_seconds
            self._advance_exercise()

    def _on_reps_done(self, btn):
        ex = self._running_plan.exercises[self._current_exercise_idx]
        elapsed = int(datetime.now().timestamp() - self._exercise_start_time)
        ex_log = ExerciseLog(
            exercise_name=ex.name,
            planned_duration_seconds=ex.duration_seconds,
            actual_duration_seconds=elapsed,
            planned_reps=ex.reps,
            actual_reps=int(self._reps_spin.get_value()),
            rest_seconds=ex.rest_seconds,
            completed=True,
            image_path=ex.image_path,
        )
        self._running_session.exercises.append(ex_log)
        sound_player.play_sound(app_settings.get_sound("exercise_complete_sound"))
        self._reps_box.set_visible(False)
        self._start_rest()

    def _on_runner_pause(self, btn):
        if self._timer.is_running:
            self._timer.pause()
            self._runner_pause_btn.set_label("Resume")
        else:
            self._timer.resume()
            self._runner_pause_btn.set_label("Pause")

    def _on_runner_skip(self, btn):
        self._timer.stop()

        ex = self._running_plan.exercises[self._current_exercise_idx]

        if self._phase == "exercise":
            elapsed = int(datetime.now().timestamp() - self._exercise_start_time)
            ex_log = ExerciseLog(
                exercise_name=ex.name,
                planned_duration_seconds=ex.duration_seconds,
                actual_duration_seconds=elapsed,
                planned_reps=ex.reps,
                actual_reps=int(self._reps_spin.get_value()) if not ex.is_timed() else None,
                rest_seconds=ex.rest_seconds,
                completed=False,
                image_path=ex.image_path,
            )
            self._running_session.exercises.append(ex_log)
            self._start_rest()
        elif self._phase == "rest":
            ex_log = self._running_session.exercises[-1] if self._running_session.exercises else None
            if ex_log:
                elapsed_rest = ex.rest_seconds - int(self._timer.remaining_seconds) if self._timer.remaining_seconds > 0 else 0
                ex_log.actual_rest_seconds = elapsed_rest
            self._advance_exercise()

    def _on_runner_stop(self, btn):
        self._timer.stop()

        if self._running_session and not self._running_session.finished_at:
            self._running_session.finished_at = datetime.now()
            self._running_session.total_actual_seconds = self._running_session.compute_total_actual_seconds()
            self._store.save_session(self._running_session)

            sound_player.play_sound(app_settings.get_sound("training_complete_sound"))

            self._populate_summary(self._running_session)
            self._stack.set_visible_child_name("summary")

            self._running_session = None
            self._running_plan = None
        else:
            self._show_list()