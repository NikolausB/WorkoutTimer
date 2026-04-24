import json
import threading
from gi.repository import Adw, Gtk, GLib
from data_store import DataStore
from models import TrainingPlan, Exercise
from settings import app_settings
from llm_client import chat_completion, build_history_context, parse_plan_response, DEFAULT_SYSTEM_PROMPT, LLMError
from image_utils import load_all_exercises


class AICoachPage(Adw.Bin):
    def __init__(self, data_store: DataStore, on_plan_saved=None, **kwargs):
        super().__init__(**kwargs)
        self._store = data_store
        self._on_plan_saved = on_plan_saved
        self._generated_plan: TrainingPlan | None = None
        self._generating = False
        self._build_ui()

    def _build_ui(self):
        scrolled = Gtk.ScrolledWindow(vexpand=True)
        clamp = Adw.Clamp()
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
        box.set_margin_top(24)
        box.set_margin_bottom(24)
        box.set_margin_start(24)
        box.set_margin_end(24)

        provider_group = Adw.PreferencesGroup(title="Provider")
        provider_group.set_description("Configure your LLM provider (Ollama, OpenAI, etc.)")

        self._provider_row = Adw.ComboRow(title="Provider", subtitle="Select your LLM backend")
        provider_model = Gtk.StringList.new(["Ollama (local)", "OpenAI-compatible"])
        self._provider_row.set_model(provider_model)
        if app_settings.ai_provider == "openai_compatible":
            self._provider_row.set_selected(1)
        else:
            self._provider_row.set_selected(0)
        self._provider_row.connect("notify::selected", self._on_provider_changed)
        provider_group.add(self._provider_row)

        self._model_row = Adw.EntryRow(title="Model")
        self._model_row.set_text(app_settings.ai_model)
        provider_group.add(self._model_row)

        self._url_row = Adw.EntryRow(title="API URL")
        self._url_row.set_text(app_settings.ai_base_url)
        provider_group.add(self._url_row)

        self._key_row = Adw.PasswordEntryRow(title="API Key")
        self._key_row.set_text(app_settings.ai_api_key)
        provider_group.add(self._key_row)

        self._key_row.set_visible(app_settings.ai_provider == "openai_compatible")

        box.append(provider_group)

        prompt_group = Adw.PreferencesGroup(title="Prompts")

        self._history_switch = Adw.SwitchRow(
            title="Include Training History",
            subtitle="Send your recent workout history as context for the AI",
        )
        self._history_switch.set_active(app_settings.ai_include_history)
        prompt_group.add(self._history_switch)

        sys_label = Gtk.Label(label="System Prompt", css_classes=["heading"], halign=Gtk.Align.START)
        sys_label.set_margin_top(6)
        prompt_group.add(sys_label)

        self._system_prompt = Gtk.TextView()
        self._system_prompt.set_wrap_mode(Gtk.WrapMode.WORD_CHAR)
        buf = self._system_prompt.get_buffer()
        prompt_text = app_settings.ai_system_prompt or DEFAULT_SYSTEM_PROMPT
        buf.set_text(prompt_text)
        sys_scroll = Gtk.ScrolledWindow(min_content_height=120, max_content_height=200)
        sys_scroll.set_child(self._system_prompt)
        sys_frame = Gtk.Frame()
        sys_frame.set_child(sys_scroll)
        prompt_group.add(sys_frame)

        user_label = Gtk.Label(label="Your Request", css_classes=["heading"], halign=Gtk.Align.START)
        user_label.set_margin_top(6)
        prompt_group.add(user_label)

        self._user_prompt = Gtk.TextView()
        self._user_prompt.set_wrap_mode(Gtk.WrapMode.WORD_CHAR)
        self._user_prompt.get_buffer().set_text("")
        user_scroll = Gtk.ScrolledWindow(min_content_height=80, max_content_height=150)
        user_scroll.set_child(self._user_prompt)
        user_frame = Gtk.Frame()
        user_frame.set_child(user_scroll)
        prompt_group.add(user_frame)

        box.append(prompt_group)

        self._generate_btn = Gtk.Button(label="Generate Plan", css_classes=["suggested-action"], halign=Gtk.Align.CENTER)
        self._generate_btn.connect("clicked", self._on_generate)
        box.append(self._generate_btn)

        self._spinner = Gtk.Spinner(halign=Gtk.Align.CENTER, visible=False)
        box.append(self._spinner)

        self._error_label = Gtk.Label(label="", css_classes=["error"], visible=False, wrap=True)
        box.append(self._error_label)

        self._result_group = Adw.PreferencesGroup(title="Generated Plan", visible=False)
        box.append(self._result_group)

        self._result_name_label = Gtk.Label(label="", css_classes=["title-2"])
        self._result_group.add(self._result_name_label)

        self._result_rows_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
        self._result_group.add(self._result_rows_box)

        result_btn_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12, halign=Gtk.Align.CENTER)
        self._save_btn = Gtk.Button(label="Save Plan", css_classes=["suggested-action"])
        self._save_btn.connect("clicked", self._on_save_plan)
        result_btn_box.append(self._save_btn)
        self._regen_btn = Gtk.Button(label="Regenerate")
        self._regen_btn.connect("clicked", self._on_generate)
        result_btn_box.append(self._regen_btn)
        self._result_group.add(result_btn_box)

        clamp.set_child(box)
        scrolled.set_child(clamp)
        self.set_child(scrolled)

    def _on_provider_changed(self, combo_row, param):
        idx = combo_row.get_selected()
        self._key_row.set_visible(idx == 1)

    def _on_generate(self, btn):
        if self._generating:
            return

        buf = self._user_prompt.get_buffer()
        start = buf.get_start_iter()
        end = buf.get_end_iter()
        user_text = buf.get_text(start, end, False).strip()
        if not user_text:
            return

        self._generating = True
        self._generate_btn.set_sensitive(False)
        self._spinner.set_visible(True)
        self._spinner.start()
        self._error_label.set_visible(False)
        self._result_group.set_visible(False)

        idx = self._provider_row.get_selected()
        provider = "ollama" if idx == 0 else "openai_compatible"
        model = self._model_row.get_text().strip() or "llama3.2"
        base_url = self._url_row.get_text().strip() or "http://localhost:11434/v1"
        api_key = self._key_row.get_text().strip()

        sys_buf = self._system_prompt.get_buffer()
        sys_start = sys_buf.get_start_iter()
        sys_end = sys_buf.get_end_iter()
        system_prompt = sys_buf.get_text(sys_start, sys_end, False).strip() or DEFAULT_SYSTEM_PROMPT

        include_history = self._history_switch.get_active()

        self._save_settings_to_global(provider, model, base_url, api_key, include_history, system_prompt)

        messages = [{"role": "system", "content": system_prompt}]

        if include_history:
            sessions = self._store.load_sessions()
            history_ctx = build_history_context(sessions)
            messages.append({"role": "user", "content": f"Here is my training history for context:\n{history_ctx}"})

        messages.append({"role": "user", "content": user_text})

        thread = threading.Thread(target=self._generate_thread, args=(base_url, api_key, model, messages), daemon=True)
        thread.start()

    def _generate_thread(self, base_url, api_key, model, messages):
        try:
            response_text = chat_completion(base_url, api_key, model, messages)
            plan_data = parse_plan_response(response_text)
            if plan_data is None:
                GLib.idle_add(self._on_generate_error, "AI returned invalid plan format. Try again or adjust your prompt.")
                return
            plan = self._parse_plan(plan_data)
            if plan is None:
                GLib.idle_add(self._on_generate_error, "Could not create a valid plan from the AI response.")
                return
            GLib.idle_add(self._on_generate_success, plan)
        except LLMError as e:
            GLib.idle_add(self._on_generate_error, str(e))
        except Exception as e:
            GLib.idle_add(self._on_generate_error, f"Unexpected error: {e}")

    def _parse_plan(self, data: dict) -> TrainingPlan | None:
        try:
            name = data.get("name", "AI Generated Plan")
            exercises = []
            all_exercises = load_all_exercises()
            name_to_key = {e["name"].lower(): e["name"] for e in all_exercises}

            for ex_data in data.get("exercises", []):
                ex_name = ex_data.get("name", "").strip()
                if not ex_name:
                    continue

                dur = ex_data.get("duration_seconds")
                reps = ex_data.get("reps")
                rest = ex_data.get("rest_seconds", 30)
                weight_kg = ex_data.get("weight_kg")

                if dur is not None:
                    try:
                        dur = int(dur)
                        if dur <= 0:
                            dur = None
                    except (ValueError, TypeError):
                        dur = None
                if reps is not None:
                    try:
                        reps = int(reps)
                        if reps <= 0:
                            reps = None
                    except (ValueError, TypeError):
                        reps = None
                try:
                    rest = max(0, int(rest))
                except (ValueError, TypeError):
                    rest = 30
                if weight_kg is not None:
                    try:
                        weight_kg = float(weight_kg)
                        if weight_kg <= 0:
                            weight_kg = None
                    except (ValueError, TypeError):
                        weight_kg = None

                image_key = self._match_exercise_image(ex_name, name_to_key)
                exercises.append(Exercise(
                    name=ex_name,
                    duration_seconds=dur,
                    reps=reps,
                    weight_kg=weight_kg,
                    rest_seconds=rest,
                    image_path=image_key,
                ))

            if not exercises:
                return None

            total_rounds = 1
            try:
                total_rounds = max(1, int(data.get("total_rounds", 1)))
            except (ValueError, TypeError):
                total_rounds = 1

            rest_between_rounds = 60
            try:
                rest_between_rounds = max(0, int(data.get("rest_between_rounds_seconds", 60)))
            except (ValueError, TypeError):
                rest_between_rounds = 60

            return TrainingPlan(name=name, exercises=exercises, total_rounds=total_rounds, rest_between_rounds_seconds=rest_between_rounds)
        except Exception:
            return None

    def _match_exercise_image(self, name: str, name_to_key: dict) -> str | None:
        lower = name.lower().strip()
        if lower in name_to_key:
            db_name = name_to_key[lower]
            return f"bundled:{db_name.replace(' ', '_')}"
        for key, db_name in name_to_key.items():
            if lower in key or key in lower:
                return f"bundled:{db_name.replace(' ', '_')}"
        return None

    def _on_generate_success(self, plan: TrainingPlan):
        self._generated_plan = plan
        self._generating = False
        self._generate_btn.set_sensitive(True)
        self._spinner.stop()
        self._spinner.set_visible(False)

        self._result_name_label.set_label(plan.name)
        if plan.total_rounds > 1:
            self._result_name_label.set_label(f"{plan.name} ({plan.total_rounds} rounds)")

        while child := self._result_rows_box.get_first_child():
            self._result_rows_box.remove(child)

        for ex in plan.exercises:
            parts = []
            if ex.weight_kg is not None and ex.weight_kg > 0:
                parts.append(f"{ex.weight_kg:g}kg")
            if ex.duration_seconds:
                parts.append(f"{ex.duration_seconds}s")
            if ex.reps:
                parts.append(f"{ex.reps} reps")
            rest = f"[{ex.rest_seconds}s rest]" if ex.rest_seconds > 0 else ""
            title = f"{ex.name} ({', '.join(parts)}) {rest}".strip()
            row = Adw.ActionRow(title=title)
            self._result_rows_box.append(row)

        self._result_group.set_visible(True)

    def _on_generate_error(self, error_msg: str):
        self._generating = False
        self._generate_btn.set_sensitive(True)
        self._spinner.stop()
        self._spinner.set_visible(False)
        self._error_label.set_label(error_msg)
        self._error_label.set_visible(True)

    def _on_save_plan(self, btn):
        if self._generated_plan:
            self._store.save_plan(self._generated_plan)
            self._generated_plan = None
            self._result_group.set_visible(False)
            if self._on_plan_saved:
                self._on_plan_saved()

    def _save_settings_to_global(self, provider, model, base_url, api_key, include_history, system_prompt):
        from settings import save_settings
        app_settings.ai_provider = provider
        app_settings.ai_model = model
        app_settings.ai_base_url = base_url
        app_settings.ai_api_key = api_key
        app_settings.ai_include_history = include_history
        app_settings.ai_system_prompt = system_prompt if system_prompt != DEFAULT_SYSTEM_PROMPT else ""
        save_settings(app_settings)