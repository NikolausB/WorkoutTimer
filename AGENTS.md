# Agent Notes

## Project overview

This is a GTK 4 / libadwaita **Rust** application called **Workout Timer**. It is distributed as a Flatpak and built with Meson (which invokes Cargo). It is a full rewrite of the original Python/PyGObject version (see git history for the Python sources).

## How to run

```bash
bash run.sh
```

This builds with `cargo build` and launches the binary. Alternatively:

```bash
cargo run
```

## Code style and architecture

- Rust 2021 edition, GTK 4 (`gtk4` crate, feature `v4_18`) and libadwaita (`libadwaita` crate, feature `v1_7`).
- Pages are plain structs (not glib subclasses) wrapped in `Rc`, held by `MainWindow` (`src/window.rs`). Closures use `Rc::downgrade` / stored `Weak` fields so there are no reference cycles.
- All UI pages live in `src/*.rs` and are managed by `src/window.rs` (an `Adw.ViewStack`).
- Shared styling lives in `src/app_style.css` and is loaded once by `MainWindow`.
- Controller/gamepad focus cycling uses `src/controller_focus.rs` (`ControllerFocusTracker`).

## Controller focus conventions

- `controller_dpad_up/down` cycles through the page's focusable widgets in visual order.
- `controller_dpad_left/right` adjusts the focused widget value where appropriate (spin rows, combo rows, switches).
- `controller_a` activates the focused widget (expand/collapse rows, click buttons).
- `controller_x` toggles an expander row in the training plan editor or collapses a focused child back to its parent row.
- `controller_b` goes back / cancels, with confirmation dialogs for stopping workouts.
- `controller_select` opens the virtual keyboard when a text view is focused in the AI Coach.
- `controller_start` pauses/resumes in runner, starts training in editor, saves preferences.

Raw controller input is read from `/dev/input/event*` (+ `/dev/input/js*` fallback) via `src/controller_manager.rs` using `libc` ioctl/poll — this is thread-pooled with `glib::idle_add_once` used to hop back onto the main thread. Detection is capability-based (EVIOCGBIT key bitmap contains BTN_JOYSTICK/GAMEPAD codes) with a name-keyword fallback for js nodes; a 3s glib timeout rescans for hot-plugged pads, and POLLERR/HUP detaches vanished devices (`connected()` derives from the active-path set — no count races). `CONTROLLER_DEBUG=1` logs raw events + actions. Note: in `connect_local` signal handlers `args[0]` is the emitting object, not the first signal parameter (regression-tested in `ui_smoke_all`).

## UI conventions

- Page content is wrapped in `Adw.Clamp` with 24 px page margins and 12 px group spacing.
- Use `Adw.PreferencesGroup` / `Adw.PreferencesPage` for settings-style layouts. A `PreferencesGroup` only accepts preference rows — never put a `ListBox` inside it (it triggers `gtk_list_box_row_grab_focus: box != NULL` warnings).
- Labels with `&` in their text must use the markup entity (`&amp;`), otherwise GTK fails to parse the markup.
- The shared CSS class `controller-focus` gives the visible gamepad focus outline.
- File dialogs use `gtk4::FileDialog` via `src/dialogs.rs` (not the deprecated FileChooserNative).

## Preferences

The Preferences dialog is an `adw::PreferencesDialog` (auto-saves on close with a toast):
- Sounds master switch, volume SpinRow (0-100), per-event sound combos.
- Tab visibility toggles (Home, Timer, Plans, AI Coach).
- Controller settings (Enable Gamepad, Show Hints, Deck Mode).
- Tristate `color_scheme` (`default`/`light`/`dark`) controlling the application appearance.
- Backup group: export/restore all data (plans + history + settings + user exercises) as one JSON bundle (`src/backup.rs`).

Removed options (not loaded/saved): `show_exercise_images`, `fullscreen_mode`.

## Additional subsystems

- `src/progress_ring.rs`: cairo circular progress ring (runner + round timer), phase colors, `set_dimmed` for paused state.
- `src/bar_chart.rs`: cairo bar chart used by the history page (range toggle 8 weeks/12 months × metric toggle time/volume/sessions).
- `src/analytics.rs`: pure training analytics — Epley 1RM, volume load, work seconds, adherence, per-exercise personal records (`exercise_stats`), weekly buckets and `progressed_plan` (progressive overload: +1 rep / +2.5 kg / +5 s for completed exercises).
- `src/backup.rs`: whole-data backup/restore, merge-by-id semantics.
- `src/help_page.rs`: "Help & Shortcuts" window; the controller hint strings live in `src/controller_hints.rs::hints_for` (single source of truth for the hint bar and the help page).
- `src/onboarding.rs`: first-run wizard (goal/equipment/frequency) that adds matching starter plans; `settings.onboarding_done` marks it as done. The bundled default plans are only auto-seeded once the wizard ran.
- `src/timer.rs`: TimerCore is wall-clock anchored (Instant deadline); `add_seconds` for the runner's "+30s" button.
- `src/data_store.rs::atomic_write`: all JSON persistence (plans, history, settings, user exercises) is atomic (tmp + rename).
- Undo: `window::show_app_toast_action(title, action, cb)` shows a toast with a button (used for deleted plans, sessions and custom exercises).
- Keyboard shortcuts (window-level EventControllerKey): Space = start/pause, S = skip, R = reset (timer + plan runner only; skipped while typing or when buttons have focus), `?` = help, Ctrl+, = preferences.
- Desktop notifications via `src/dialogs.rs::send_notification` on round-timer/training completion.
- Settings extras (all `#[serde(default)]`, old files stay compatible): `sound_volume` (0-1), `last_tab`, `window_width`, `window_height` (restored/persisted via `close-request`), `timer_config` (round timer setup, incl. prepare/warm-up/custom rounds), `onboarding_done`.
- Round timer phases (`Phase`): Prepare → Warm-up → Round → Rest → Done; custom rounds (`RoundConfig::per_round`) override the global round/pause length.
- History filters: `HistoryFilter { plan, days }` + `history::apply_filter` — list, chart and totals all use the filtered list (the filter combo notification is guarded by `updating_filters` to avoid a refresh loop).
- Screenshot debug mode: `TRAINING_SHOT_DIR=/some/dir` renders every main tab to PNG at startup (uses WidgetPaintable + `gtk_snapshot_to_node` via ffi + one gsk renderer; unrealize the renderer before drop or GSK aborts). Skips the onboarding wizard.
- AI Coach requests stream via SSE (`chat_completion_stream` in `src/llm.rs`, `stream:true`): progress (chars + tail) throttled to 200ms into the UI, a Cancel button aborts through an `Arc<AtomicBool>` checked between SSE lines, the worker call is wrapped in `catch_unwind` (a panic can no longer leave the spinner running forever), and non-SSE responses fall back to plain JSON parsing. `HTTP 410` gets a clear "model no longer available" message (Ollama retires cloud models). GLM models stream a long `delta.reasoning` phase first — surfaced as "Model is thinking…" (an unawaited `async_channel::send()` future is silently dropped: worker→UI events MUST use `send_blocking`). Live test: `AI_DEBUG=1 cargo test -- --ignored ai_coach` (or `STREAM_TEST_MODEL=<model> cargo test -- --ignored stream`).
- Live LLM test: `STREAM_TEST_MODEL=<model> cargo test -- --ignored stream` (needs local Ollama).
- Import functions return counts: `import_history_csv -> (sessions, skipped_rows)`, `import_plans_json -> Result<Vec<TrainingPlan>, String>`; results are surfaced as toasts.

## Data model

- `TrainingPlan`, `Exercise`, `TrainingSession`, and `ExerciseLog` live in `src/models.rs` (serde structs).
- `ExerciseLog` stores `started_at` and `finished_at` for each exercise. Older sessions without these timestamps still load, and the UI falls back to showing only duration.
- CSV export/import includes `exercise_started_at` and `exercise_finished_at` columns.

## Data compatibility (important!)

The app reads/writes the exact same formats as the old Python version:
- `~/.local/share/training-flatpak/plans.json` and `history.json` (serde round-trip has tests)
- `settings.json` (all keys preserved)
- `user_exercises.json`
- CSV: same 22-column header
- Datetimes are stored as Python-style naive local ISO strings (`%Y-%m-%dT%H:%M:%S%.6f`), serialized via chrono format strings in `src/models.rs`. Do **not** change this to RFC 3339 with offset, or old files become unreadable by the Python builds.

## Images

Exercise images are only used as small thumbnails in the exercise picker; plan editors and runners do not display them. `image_path` is preserved in saved JSON/CSV for backward compatibility, and the `show_exercise_images` setting was removed from preferences.

## Build & packaging

- `Cargo.toml` at the root; `vendor/` + `.cargo/config.toml` are **untracked local-only** files that enable offline builds (regenerate with `cargo vendor`, never commit them — a tracked config would break every clean checkout since `vendor/` does not exist there); clean checkouts and CI resolve crates from crates.io, and the Flatpak manifests use `cargo-sources.json` (generated by `flatpak-cargo-generator.py`).
- Flatpak manifests: `io.github.NikolausB.WorkoutTimer.yml` (offline, Flathub-style) and `local-training.yml` (network allowed during build).
- Target runtime: `org.gnome.Platform//51`. Sdk extension is declared as `org.freedesktop.Sdk.Extension.rust-stable` WITHOUT an explicit branch — the Sdk 51 metadata (`version = 26.08`) resolves it; flatpak-builder rejects `//26.08` there.
- Local bundle workflow: `flatpak-builder --force-clean --repo=wt-repo local-build-dir io.github.NikolausB.WorkoutTimer.yml` (needs org.gnome.Platform//51 + org.gnome.Sdk//51 + rust-stable installed), then `flatpak build-bundle wt-repo workout-timer-1.3.0.flatpak io.github.NikolausB.WorkoutTimer master` (branch is `master` — no `branch:` key in the manifest).
- Meson (`meson.build`, `src/meson.build`) wraps cargo and installs the binary + resources.