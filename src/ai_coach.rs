use crate::controller_focus::{adjust_focused_widget, ControllerFocusTracker};
use crate::data_store::DataStore;
use crate::image_utils::load_all_exercises;
use crate::llm::{
    build_history_context, chat_completion_stream, parse_plan_response, DEFAULT_SYSTEM_PROMPT,
};
use crate::models::{Exercise, TrainingPlan};
use crate::settings::{load_settings, save_settings};
use crate::virtual_keyboard::VirtualKeyboard;
use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, Frame, Label, Orientation, ScrolledWindow, TextView, WrapMode,
};
use libadwaita::prelude::*;
use libadwaita::{
    ActionRow, Bin, ButtonRow, Clamp, EntryRow, PasswordEntryRow, PreferencesGroup, SwitchRow,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

pub struct AICoachPage {
    widget: Bin,
    store: Rc<DataStore>,
    on_plan_saved: RefCell<Option<Rc<dyn Fn()>>>,
    generated_plan: RefCell<Option<TrainingPlan>>,
    pub(crate) generating: Cell<bool>,
    focus: RefCell<ControllerFocusTracker>,
    // Widgets
    provider_combo: libadwaita::ComboRow,
    pub(crate) model_row: EntryRow,
    url_row: EntryRow,
    key_row: PasswordEntryRow,
    history_switch: SwitchRow,
    system_prompt: TextView,
    pub(crate) user_prompt: TextView,
    generate_btn: ButtonRow,
    cancel_btn: gtk4::Button,
    pub(crate) progress_label: Label,
    cancel_flag: RefCell<Option<Arc<std::sync::atomic::AtomicBool>>>,
    spinner: gtk4::Spinner,
    pub(crate) error_label: Label,
    result_group: PreferencesGroup,
    result_name_label: Label,
    result_rows_box: GtkBox,
    result_action_group: PreferencesGroup,
    save_btn: ButtonRow,
    regen_btn: ButtonRow,
    keyboard: Rc<VirtualKeyboard>,
    keyboard_visible: Cell<bool>,
    self_weak: RefCell<std::rc::Weak<AICoachPage>>,
}

impl AICoachPage {
    pub fn new(store: Rc<DataStore>, _on_plan_saved_unused: ()) -> Rc<Self> {
        let widget = Bin::new();

        let scrolled = ScrolledWindow::new();
        scrolled.set_vexpand(true);
        let clamp = Clamp::new();
        let box_widget = GtkBox::new(Orientation::Vertical, 12);
        box_widget.set_margin_top(12);
        box_widget.set_margin_bottom(12);
        box_widget.set_margin_start(12);
        box_widget.set_margin_end(12);

        // Provider group
        let provider_group = PreferencesGroup::builder()
            .title("Provider")
            .description("Configure your LLM provider")
            .build();

        let settings = load_settings();

        let provider_combo = libadwaita::ComboRow::new();
        provider_combo.set_title("Provider");
        provider_combo.set_subtitle("Select your LLM backend");
        let provider_model = gtk4::StringList::new(&["Ollama (local)", "OpenAI-compatible"]);
        provider_combo.set_model(Some(&provider_model));
        if settings.ai_provider == "openai_compatible" {
            provider_combo.set_selected(1);
        } else {
            provider_combo.set_selected(0);
        }
        provider_group.add(&provider_combo);

        let model_row = EntryRow::builder().title("Model").build();
        model_row.set_text(&settings.ai_model);
        provider_group.add(&model_row);

        let url_row = EntryRow::builder().title("API URL").build();
        url_row.set_text(&settings.ai_base_url);
        provider_group.add(&url_row);

        let key_row = PasswordEntryRow::builder().title("API Key").build();
        key_row.set_text(&settings.ai_api_key);
        let key_row_visible = settings.ai_provider == "openai_compatible";
        key_row.set_visible(key_row_visible);
        provider_group.add(&key_row);

        box_widget.append(&provider_group);

        // Prompt group
        let prompt_group = PreferencesGroup::builder()
            .title("Prompts")
            .description("Describe the training plan you want")
            .build();

        let history_switch = SwitchRow::builder()
            .title("Include Training History")
            .subtitle("Send your recent workout history as context for the AI")
            .build();
        history_switch.set_active(settings.ai_include_history);
        prompt_group.add(&history_switch);

        let system_prompt = TextView::new();
        system_prompt.set_wrap_mode(WrapMode::WordChar);
        let prompt_text = if settings.ai_system_prompt.is_empty() {
            DEFAULT_SYSTEM_PROMPT.to_string()
        } else {
            settings.ai_system_prompt.clone()
        };
        system_prompt.buffer().set_text(&prompt_text);
        let sys_scroll = ScrolledWindow::new();
        sys_scroll.set_min_content_height(120);
        sys_scroll.set_max_content_height(200);
        sys_scroll.set_child(Some(&system_prompt));
        let sys_frame = Frame::new(None);
        sys_frame.set_child(Some(&sys_scroll));
        prompt_group.add(&sys_frame);

        let user_prompt = TextView::new();
        user_prompt.set_wrap_mode(WrapMode::WordChar);
        user_prompt.buffer().set_text("");
        let user_scroll = ScrolledWindow::new();
        user_scroll.set_min_content_height(80);
        user_scroll.set_max_content_height(150);
        user_scroll.set_child(Some(&user_prompt));
        let user_frame = Frame::new(None);
        user_frame.set_child(Some(&user_scroll));
        prompt_group.add(&user_frame);

        box_widget.append(&prompt_group);

        // Action group
        let action_group = PreferencesGroup::new();
        let generate_btn = ButtonRow::builder()
            .title("Generate Plan")
            .css_classes(["suggested-action"])
            .build();
        action_group.add(&generate_btn);
        box_widget.append(&action_group);

        let spinner = gtk4::Spinner::new();
        spinner.set_halign(Align::Center);
        spinner.set_visible(false);
        box_widget.append(&spinner);

        let progress_label = Label::new(None);
        progress_label.add_css_class("dim-label");
        progress_label.add_css_class("caption");
        progress_label.set_halign(Align::Center);
        progress_label.set_visible(false);
        progress_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        box_widget.append(&progress_label);

        let cancel_btn = Button::builder()
            .label("Cancel")
            .css_classes(["destructive-action"])
            .halign(Align::Center)
            .visible(false)
            .build();
        box_widget.append(&cancel_btn);

        let error_label = Label::new(None);
        error_label.add_css_class("error");
        error_label.set_visible(false);
        error_label.set_wrap(true);
        box_widget.append(&error_label);

        let result_group = PreferencesGroup::builder()
            .title("Generated Plan")
            .visible(false)
            .build();
        box_widget.append(&result_group);

        // Keyboard — the callback is wired through a shared RefCell so that it can
        // reach the page after construction without a reference cycle.
        let keyboard_typed_shared: Rc<RefCell<Box<dyn Fn(&str)>>> =
            Rc::new(RefCell::new(Box::new(|_| {})));
        let keyboard_shared = Rc::clone(&keyboard_typed_shared);
        let keyboard = Rc::new(VirtualKeyboard::with_callback(Box::new(
            move |text: &str| {
                let cb = keyboard_shared.borrow();
                cb(text);
            },
        )));
        keyboard.widget().set_visible(false);
        box_widget.append(keyboard.widget());

        system_prompt.set_focusable(true);
        system_prompt.set_can_focus(true);
        user_prompt.set_focusable(true);
        user_prompt.set_can_focus(true);

        let result_name_label = Label::new(None);
        result_name_label.add_css_class("title-2");
        result_group.add(&result_name_label);

        let result_rows_box = GtkBox::new(Orientation::Vertical, 4);
        result_group.add(&result_rows_box);

        let result_action_group = PreferencesGroup::builder().visible(false).build();
        let save_btn = ButtonRow::builder()
            .title("Save Plan")
            .css_classes(["suggested-action"])
            .build();
        result_action_group.add(&save_btn);
        let regen_btn = ButtonRow::builder().title("Regenerate").build();
        result_action_group.add(&regen_btn);
        box_widget.append(&result_action_group);

        clamp.set_child(Some(&box_widget));
        scrolled.set_child(Some(&clamp));
        widget.set_child(Some(&scrolled));

        let page = Rc::new(Self {
            widget,
            store: Rc::clone(&store),
            on_plan_saved: RefCell::new(None),
            generated_plan: RefCell::new(None),
            generating: Cell::new(false),
            focus: RefCell::new(ControllerFocusTracker::new()),
            provider_combo,
            model_row,
            url_row,
            key_row,
            history_switch,
            system_prompt,
            user_prompt,
            generate_btn,
            cancel_btn: cancel_btn.clone(),
            progress_label: progress_label.clone(),
            cancel_flag: RefCell::new(None),
            spinner,
            error_label,
            result_group,
            result_name_label,
            result_rows_box,
            result_action_group,
            save_btn,
            regen_btn,
            keyboard,
            keyboard_visible: Cell::new(false),
            self_weak: RefCell::new(std::rc::Weak::new()),
        });
        *page.self_weak.borrow_mut() = Rc::downgrade(&page);

        // Wire the shared keyboard callback to the page
        {
            let page_weak = Rc::downgrade(&page);
            *keyboard_typed_shared.borrow_mut() = Box::new(move |text: &str| {
                if let Some(p) = page_weak.upgrade() {
                    p.on_keyboard_typed(text);
                }
            });
        }

        // Provider changed
        {
            let page_weak = Rc::downgrade(&page);
            page.provider_combo.connect_selected_notify(move |combo| {
                if let Some(p) = page_weak.upgrade() {
                    let idx = combo.selected() as i32;
                    let key_row = p.key_row.clone();
                    key_row.set_visible(idx == 1);
                    p.refresh_focus();
                }
            });
        }

        // Generate / regenerate / save
        {
            let page_weak = Rc::downgrade(&page);
            page.generate_btn.connect_activated(move |_| {
                let Some(p) = page_weak.upgrade() else { return };
                p.on_generate();
            });
        }
        {
            let page_weak = Rc::downgrade(&page);
            page.cancel_btn.connect_clicked(move |_| {
                let Some(p) = page_weak.upgrade() else { return };
                if let Some(flag) = p.cancel_flag.borrow().as_ref() {
                    flag.store(true, std::sync::atomic::Ordering::Relaxed);
                }
                p.progress_label.set_label("Cancelling…");
            });
        }
        {
            let page_weak = Rc::downgrade(&page);
            page.regen_btn.connect_activated(move |_| {
                let Some(p) = page_weak.upgrade() else { return };
                p.on_generate();
            });
        }
        {
            let page_weak = Rc::downgrade(&page);
            page.save_btn.connect_activated(move |_| {
                let Some(p) = page_weak.upgrade() else { return };
                p.on_save_plan();
            });
        }

        page
    }

    pub fn widget(&self) -> &Bin {
        &self.widget
    }

    pub fn set_on_plan_saved(&self, cb: Rc<dyn Fn()>) {
        *self.on_plan_saved.borrow_mut() = Some(cb);
    }

    pub fn on_page_visible(&self) {
        self.refresh_focus();
        self.focus.borrow_mut().focus_first();
    }

    fn keyboard_is_visible(&self) -> bool {
        self.keyboard_visible.get()
    }

    fn set_keyboard_visible(&self, visible: bool) {
        self.keyboard_visible.set(visible);
        self.keyboard.widget().set_visible(visible);
        self.refresh_focus();
    }

    pub(crate) fn on_generate(&self) {
        if self.generating.get() {
            return;
        }

        let buf = self.user_prompt.buffer();
        let start = buf.start_iter();
        let end = buf.end_iter();
        let user_text = buf.text(&start, &end, false).trim().to_string();
        if user_text.is_empty() {
            return;
        }

        self.generating.set(true);
        self.generate_btn.set_sensitive(false);
        self.spinner.set_visible(true);
        self.spinner.start();
        self.progress_label.set_label("Contacting model…");
        self.progress_label.set_visible(true);
        self.cancel_btn.set_visible(true);
        self.error_label.set_visible(false);
        self.result_group.set_visible(false);
        self.result_action_group.set_visible(false);
        self.refresh_focus();

        let idx = self.provider_combo.selected() as i32;
        let provider = if idx == 1 {
            "openai_compatible"
        } else {
            "ollama"
        };
        let mut model = self.model_row.text().trim().to_string();
        if model.is_empty() {
            model = "llama3.2".to_string();
        }
        let mut base_url = self.url_row.text().trim().to_string();
        if base_url.is_empty() {
            base_url = "http://localhost:11434/v1".to_string();
        }
        let api_key = self.key_row.text().trim().to_string();

        let sys_buf = self.system_prompt.buffer();
        let sys_start = sys_buf.start_iter();
        let sys_end = sys_buf.end_iter();
        let system_prompt_text = sys_buf.text(&sys_start, &sys_end, false).trim().to_string();
        let system_prompt = if system_prompt_text.is_empty() {
            DEFAULT_SYSTEM_PROMPT.to_string()
        } else {
            system_prompt_text.clone()
        };

        let include_history = self.history_switch.is_active();

        self.save_settings_to_global(
            provider,
            &model,
            &base_url,
            &api_key,
            include_history,
            &system_prompt_text,
        );

        let mut messages: Vec<(String, String)> = Vec::new();
        messages.push(("system".to_string(), system_prompt));

        if include_history {
            let sessions = self.store.load_sessions();
            let history_ctx = build_history_context(&sessions);
            messages.push((
                "user".to_string(),
                format!("Here is my training history for context:\n{}", history_ctx),
            ));
        }

        messages.push(("user".to_string(), user_text));

        // Worker thread: reqwest blocking (SSE streaming) with progress and
        // cancel support; results delivered to the main thread via a glib
        // channel (the standard gtk-rs pattern).
        enum GenEvent {
            Thinking { seconds: f64, chars: usize },
            Progress { chars: usize, elapsed: f64, tail: String },
            Finished(Result<String, crate::llm::LLMError>),
        }
        let (sender, receiver) = async_channel::unbounded::<GenEvent>();
        let main_context = gtk4::glib::MainContext::default();
        {
            let page_weak = self.self_weak.borrow().clone();
            main_context.spawn_local(async move {
                let debug = std::env::var("AI_DEBUG").map(|v| !v.is_empty()).unwrap_or(false);
                if debug {
                    println!("[ai-receiver] listening…");
                }
                while let Ok(event) = receiver.recv().await {
                    if debug {
                        match &event {
                            GenEvent::Thinking { seconds, chars } => {
                                println!("[ai-receiver] Thinking s={} chars={}", seconds, chars)
                            }
                            GenEvent::Progress { chars, .. } => {
                                println!("[ai-receiver] Progress chars={}", chars)
                            }
                            GenEvent::Finished(Ok(text)) => {
                                println!("[ai-receiver] Finished ok {} chars", text.len())
                            }
                            GenEvent::Finished(Err(e)) => {
                                println!("[ai-receiver] Finished err: {}", e)
                            }
                        }
                    }
                    let Some(p) = page_weak.upgrade() else { return };
                    match event {
                        GenEvent::Thinking { seconds, chars } => {
                            p.progress_label.set_label(&format!(
                                "Model is thinking… {:.0}s · {} reasoning tokens · Cancel to abort",
                                seconds, chars
                            ));
                        }
                        GenEvent::Progress { chars, elapsed, tail } => {
                            let tail = tail.chars().rev().take(60).collect::<String>().chars().rev().collect::<String>();
                            p.progress_label.set_label(&format!(
                                "Receiving… {} chars · {:.0}s · last: {}",
                                chars, elapsed, tail
                            ));
                        }
                        GenEvent::Finished(result) => match result {
                            Ok(response_text) => match parse_plan_response(&response_text) {
                                None => {
                                    p.on_generate_error(
                                        "AI returned invalid plan format. Try again or adjust your prompt.",
                                    );
                                }
                                Some(data) => {
                                    match p.parse_plan(&data) {
                                        None => p.on_generate_error(
                                            "Could not create a valid plan from the AI response.",
                                        ),
                                        Some(plan) => p.on_generate_success(&plan),
                                    }
                                }
                            },
                            Err(e) => {
                                p.on_generate_error(&e.to_string());
                            }
                        },
                    }
                }
            });
        }
        {
            let cancel_flag = Arc::new(AtomicBool::new(false));
            *self.cancel_flag.borrow_mut() = Some(Arc::clone(&cancel_flag));
            std::thread::spawn(move || {
                let debug = std::env::var("AI_DEBUG").map(|v| !v.is_empty()).unwrap_or(false);
                let message_refs: Vec<(&str, &str)> = messages
                    .iter()
                    .map(|(r, c)| (r.as_str(), c.as_str()))
                    .collect();
                if debug {
                    println!(
                        "[ai-worker] starting request model='{}' url='{}' messages={} key_len={}",
                        model,
                        base_url,
                        message_refs.len(),
                        api_key.len()
                    );
                }
                let started = std::time::Instant::now();
                let progress_sender = sender.clone();
                let mut last_emit = started - std::time::Duration::from_secs(1);
                let mut on_progress = move |p: crate::llm::StreamProgress| {
                    let now = std::time::Instant::now();
                    if !(now.duration_since(last_emit).as_millis() >= 200) {
                        return;
                    }
                    last_emit = now;
                    let _ = match p {
                        crate::llm::StreamProgress::Thinking { seconds, chars } => {
                            progress_sender.send_blocking(GenEvent::Thinking { seconds, chars })
                        }
                        crate::llm::StreamProgress::Receiving { chars, elapsed, tail } => {
                            progress_sender.send_blocking(GenEvent::Progress {
                                chars,
                                elapsed,
                                tail,
                            })
                        }
                    };
                };
                // A panic in the worker must never leave the spinner running
                // forever: catch it and report it as an error.
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                    chat_completion_stream(
                        &base_url,
                        &api_key,
                        &model,
                        &message_refs,
                        &cancel_flag,
                        &mut on_progress,
                    )
                }));
                let result = match result {
                    Ok(r) => r,
                    Err(_) => Err(crate::llm::LLMError(
                        "Internal error during the request (worker panicked).".to_string(),
                    )),
                };
                if debug {
                    match &result {
                        Ok(text) => println!("[ai-worker] finished: {} chars", text.len()),
                        Err(e) => println!("[ai-worker] finished with error: {}", e),
                    }
                }
                let sent = sender.send_blocking(GenEvent::Finished(result));
                if debug {
                    println!("[ai-worker] Finished send ok: {}", sent.is_ok());
                }
            });
        }
    }

    fn parse_plan(&self, data: &serde_json::Value) -> Option<TrainingPlan> {
        let name = data
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("AI Generated Plan")
            .to_string();

        let all_exercises = load_all_exercises();
        let name_to_key: std::collections::HashMap<String, String> = all_exercises
            .iter()
            .map(|e| (e.name.to_lowercase(), e.name.clone()))
            .collect();

        let mut exercises = Vec::new();
        if let Some(ex_list) = data.get("exercises").and_then(|e| e.as_array()) {
            for ex_data in ex_list {
                let ex_name = ex_data
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if ex_name.is_empty() {
                    continue;
                }

                let parse_i64 = |v: &Option<&serde_json::Value>| -> Option<i64> {
                    v.and_then(|v| {
                        v.as_i64().or_else(|| match v {
                            serde_json::Value::Number(n) => n.as_f64().map(|f| f as i64),
                            serde_json::Value::String(s) => s.parse().ok(),
                            _ => None,
                        })
                    })
                };

                let duration_seconds = parse_i64(&ex_data.get("duration_seconds")).filter(|d| *d > 0);
                let reps = parse_i64(&ex_data.get("reps")).filter(|r| *r > 0);
                let rest = parse_i64(&ex_data.get("rest_seconds"))
                    .map(|r| r.max(0))
                    .unwrap_or(30);
                let weight_kg = ex_data
                    .get("weight_kg")
                    .and_then(|v| match v {
                        serde_json::Value::Number(n) => n.as_f64(),
                        serde_json::Value::String(s) => s.parse().ok(),
                        _ => None,
                    })
                    .filter(|w| *w > 0.0);

                exercises.push(Exercise {
                    name: ex_name,
                    duration_seconds,
                    reps,
                    rest_seconds: rest,
                    weight_kg,
                    image_path: None,
                    sets: 1,
                });
            }
        }

        if exercises.is_empty() {
            return None;
        }

        let total_rounds = parse_top_i64(data, "total_rounds").map(|r| r.max(1)).unwrap_or(1);
        let rest_between_rounds = parse_top_i64(data, "rest_between_rounds_seconds")
            .map(|r| r.max(0))
            .unwrap_or(60);

        // Match images against the exercise database
        for ex in exercises.iter_mut() {
            let image = match_image_key(&ex.name, &name_to_key);
            if let Some(img) = image {
                ex.image_path = Some(img);
            }
        }

        Some(TrainingPlan {
            name,
            exercises,
            total_rounds,
            rest_between_rounds_seconds: rest_between_rounds,
            ..Default::default()
        })
    }

    fn on_generate_success(&self, plan: &TrainingPlan) {
        *self.generated_plan.borrow_mut() = Some(plan.clone());
        self.generating.set(false);
        self.generate_btn.set_sensitive(true);
        self.spinner.stop();
        self.spinner.set_visible(false);
        self.progress_label.set_visible(false);
        self.cancel_btn.set_visible(false);
        *self.cancel_flag.borrow_mut() = None;
        self.refresh_focus();

        let display_name = if plan.total_rounds > 1 {
            format!("{} ({} rounds)", plan.name, plan.total_rounds)
        } else {
            plan.name.clone()
        };
        self.result_name_label.set_label(&display_name);

        clear_box(&self.result_rows_box);

        for ex in &plan.exercises {
            let mut parts = Vec::new();
            if let Some(w) = ex.weight_kg {
                if w > 0.0 {
                    parts.push(format!("{}kg", fmt_float(w)));
                }
            }
            if let Some(d) = ex.duration_seconds {
                if d > 0 {
                    parts.push(format!("{}s", d));
                }
            }
            if let Some(r) = ex.reps {
                if r > 0 {
                    parts.push(format!("{} reps", r));
                }
            }
            let rest = if ex.rest_seconds > 0 {
                format!("[{}s rest]", ex.rest_seconds)
            } else {
                String::new()
            };
            let title = if !parts.is_empty() {
                format!("{} ({}) {}", ex.name, parts.join(", "), rest)
                    .trim()
                    .to_string()
            } else {
                format!("{} {}", ex.name, rest).trim().to_string()
            };
            let row = ActionRow::builder().title(&title).build();
            self.result_rows_box.append(&row);
        }

        self.result_group.set_visible(true);
        self.result_action_group.set_visible(true);
        self.refresh_focus();
    }

    fn on_generate_error(&self, error_msg: &str) {
        self.generating.set(false);
        self.generate_btn.set_sensitive(true);
        self.spinner.stop();
        self.spinner.set_visible(false);
        self.progress_label.set_visible(false);
        self.cancel_btn.set_visible(false);
        *self.cancel_flag.borrow_mut() = None;
        self.error_label.set_label(error_msg);
        self.error_label.set_visible(true);
        self.refresh_focus();
    }

    fn on_save_plan(&self) {
        let plan = self.generated_plan.borrow().clone();
        if let Some(plan) = plan {
            self.store.save_plan(&plan);
            crate::window::show_app_toast("Plan saved");
            *self.generated_plan.borrow_mut() = None;
            self.result_group.set_visible(false);
            self.result_action_group.set_visible(false);
            self.refresh_focus();
            if let Some(ref cb) = *self.on_plan_saved.borrow() {
                cb();
            }
        }
    }

    fn save_settings_to_global(
        &self,
        provider: &str,
        model: &str,
        base_url: &str,
        api_key: &str,
        include_history: bool,
        system_prompt: &str,
    ) {
        let mut settings = load_settings();
        settings.ai_provider = provider.to_string();
        settings.ai_model = model.to_string();
        settings.ai_base_url = base_url.to_string();
        settings.ai_api_key = api_key.to_string();
        settings.ai_include_history = include_history;
        settings.ai_system_prompt = if system_prompt == DEFAULT_SYSTEM_PROMPT {
            String::new()
        } else {
            system_prompt.to_string()
        };
        save_settings(&settings);
    }

    fn on_keyboard_typed(&self, text: &str) {
        let buf = self.user_prompt.buffer();
        if text == "\u{8}" {
            if let Some(m) = buf.mark("insert") {
                let mut iter = buf.iter_at_mark(&m);
                buf.backspace(&mut iter, true, true);
            }
        } else if text == "\n" {
            buf.insert_at_cursor("\n");
        } else {
            buf.insert_at_cursor(text);
        }
    }

    pub fn controller_select(&self) {
        if let Some(widget) = self.focus.borrow().current_widget() {
            let is_text = widget.as_ptr()
            == self.system_prompt.upcast_ref::<gtk4::Widget>().as_ptr()
            || widget.as_ptr() == self.user_prompt.upcast_ref::<gtk4::Widget>().as_ptr();
            if is_text {
                self.set_keyboard_visible(true);
            }
        }
    }

    pub fn get_controller_context(&self) -> String {
        if self.keyboard_is_visible() {
            "keyboard".to_string()
        } else {
            "ai_coach".to_string()
        }
    }

    fn refresh_focus(&self) {
        if self.keyboard_is_visible() {
            self.focus.borrow_mut().set_widgets(Vec::new());
            return;
        }
        let mut widgets: Vec<gtk4::Widget> = Vec::new();
        if self.generating.get() {
            // While a request runs, the only interactive control is Cancel.
            widgets.push(self.cancel_btn.clone().upcast());
            self.focus.borrow_mut().set_widgets(widgets);
            return;
        }
        if self.result_group.is_visible() {
            widgets.push(self.save_btn.clone().upcast());
            widgets.push(self.regen_btn.clone().upcast());
        } else {
            widgets.push(self.generate_btn.clone().upcast());
            if self.generating.get() {
                widgets.push(self.cancel_btn.clone().upcast());
            }
            widgets.push(self.provider_combo.clone().upcast());
            widgets.push(self.model_row.clone().upcast());
            if self.provider_combo.selected() == 1 {
                widgets.push(self.url_row.clone().upcast());
                widgets.push(self.key_row.clone().upcast());
            }
            widgets.push(self.history_switch.clone().upcast());
            widgets.push(self.system_prompt.clone().upcast());
            widgets.push(self.user_prompt.clone().upcast());
        }
        self.focus.borrow_mut().set_widgets(widgets);
    }

    pub fn controller_a(&self) {
        if self.keyboard_is_visible() {
            self.keyboard.on_confirm();
            return;
        }
        if self.result_group.is_visible() {
            self.on_save_plan();
            return;
        }
        let widget_opt = self.focus.borrow().current_widget();
        let Some(widget) = widget_opt else {
            self.on_generate();
            return;
        };
        let is_text = widget.as_ptr()
            == self.system_prompt.upcast_ref::<gtk4::Widget>().as_ptr()
            || widget.as_ptr() == self.user_prompt.upcast_ref::<gtk4::Widget>().as_ptr();
        if is_text {
            self.set_keyboard_visible(true);
        } else if let Some(row) = widget.downcast_ref::<ButtonRow>() {
            row.activate();
        } else if let Some(btn) = widget.downcast_ref::<gtk4::Button>() {
            btn.emit_clicked();
        } else if let Some(entry) = widget.downcast_ref::<EntryRow>() {
            entry.grab_focus();
        }
    }

    pub fn controller_b(&self) {
        if self.keyboard_is_visible() {
            self.keyboard.on_backspace();
        } else {
            self.set_keyboard_visible(false);
        }
    }

    pub fn controller_y(&self) {
        if self.keyboard_is_visible() {
            self.keyboard.on_toggle_shift();
        }
    }

    pub fn controller_start(&self) {
        if self.keyboard_is_visible() {
            self.set_keyboard_visible(false);
        }
    }

    pub fn controller_dpad_up(&self) {
        if self.keyboard_is_visible() {
            self.keyboard.on_dpad("dpad_up");
        } else {
            self.focus.borrow_mut().cycle(-1);
        }
    }

    pub fn controller_dpad_down(&self) {
        if self.keyboard_is_visible() {
            self.keyboard.on_dpad("dpad_down");
        } else {
            self.focus.borrow_mut().cycle(1);
        }
    }

    pub fn controller_dpad_left(&self) {
        if self.keyboard_is_visible() {
            self.keyboard.on_dpad("dpad_left");
        } else if let Some(widget) = self.focus.borrow().current_widget() {
            adjust_focused_widget(&widget, -1);
        }
    }

    pub fn controller_dpad_right(&self) {
        if self.keyboard_is_visible() {
            self.keyboard.on_dpad("dpad_right");
        } else if let Some(widget) = self.focus.borrow().current_widget() {
            adjust_focused_widget(&widget, 1);
        }
    }

    pub fn controller_back(&self) {
        if self.keyboard_is_visible() {
            self.set_keyboard_visible(false);
        }
    }

    pub fn update_fonts(&self, _width: i32, _height: i32) {}
}

fn parse_top_i64(data: &serde_json::Value, key: &str) -> Option<i64> {
    data.get(key).and_then(|v| match v {
        serde_json::Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
        serde_json::Value::String(s) => s.parse().ok(),
        _ => None,
    })
}

fn match_image_key(name: &str, name_to_key: &std::collections::HashMap<String, String>) -> Option<String> {
    let lower = name.to_lowercase().trim().to_string();
    if let Some(db_name) = name_to_key.get(&lower) {
        return Some(format!("bundled:{}", db_name.replace(' ', "_")));
    }
    for (key, db_name) in name_to_key {
        if lower.contains(key.as_str()) || key.contains(&lower) {
            return Some(format!("bundled:{}", db_name.replace(' ', "_")));
        }
    }
    None
}

fn clear_box(box_widget: &GtkBox) {
    let mut child = box_widget.first_child();
    while let Some(c) = child {
        child = c.next_sibling();
        box_widget.remove(&c);
    }
}

fn fmt_float(v: f64) -> String {
    format!("{}", v)
}