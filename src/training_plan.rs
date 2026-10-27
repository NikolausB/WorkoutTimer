use crate::controller_focus::{adjust_focused_widget, ControllerFocusTracker};
use crate::csv_io::{export_plans_json, import_plans_json};
use crate::data_store::DataStore;
use crate::models::{Exercise, ExerciseLog, TrainingPlan, TrainingSession};
use crate::progress_ring::{
    ProgressRing, COLOR_BREAK, COLOR_GO, COLOR_NEUTRAL, COLOR_REST,
};
use crate::settings::load_settings;
use crate::sound::SoundPlayer;
use crate::timer::TimerCore;
use crate::ui_scaling::{apply_scaling, LabelType};
use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, Label, ListBox, Orientation, ProgressBar, Revealer,
    ScrolledWindow,
};
use libadwaita::prelude::*;
use libadwaita::{
    ActionRow, AlertDialog, Bin, ButtonRow, Clamp, EntryRow, ExpanderRow, NavigationPage,
    NavigationView, PreferencesGroup, SpinRow, SwitchRow,
};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use chrono::Local;

#[derive(Clone, Copy, PartialEq)]
pub enum Phase {
    Exercise,
    Rest,
    RoundBreak,
}

impl Phase {
    fn as_str(&self) -> &'static str {
        match self {
            Phase::Exercise => "exercise",
            Phase::Rest => "rest",
            Phase::RoundBreak => "round_break",
        }
    }
}

struct EditorRowWidgets {
    row: ExpanderRow,
    name_entry: EntryRow,
    dur_spin: SpinRow,
    reps_spin: SpinRow,
    sets_spin: SpinRow,
    weight_spin: SpinRow,
    rest_spin: SpinRow,
    remove_btn: Button,
    up_btn: Button,
    down_btn: Button,
}

pub struct TrainingPlanPage {
    widget: Bin,
    store: Rc<DataStore>,
    sound: Rc<SoundPlayer>,
    nav: NavigationView,
    plans: RefCell<Vec<TrainingPlan>>,

    // List page widgets
    pub list_page: NavigationPage,
    list_stack: gtk4::Stack,
    plan_list_box: ListBox,
    new_plan_btn: Button,
    export_btn: Button,
    import_btn: Button,
    empty_new_btn: Button,
    empty_templates_btn: Button,
    star_buttons: RefCell<HashMap<String, gtk4::Widget>>,

    // Editor page widgets
    pub editor_page: NavigationPage,
    plan_name_entry: EntryRow,
    favorite_switch: SwitchRow,
    total_rounds_spin: SpinRow,
    rest_between_rounds_spin: SpinRow,
    exercises_group: PreferencesGroup,
    editor_stats_group: PreferencesGroup,
    editor_stats_sessions: ActionRow,
    editor_stats_time: ActionRow,
    editor_stats_last: ActionRow,
    add_exercise_btn: Button,
    browse_exercise_btn: Button,
    run_btn: ButtonRow,
    save_row: ButtonRow,
    delete_btn: ButtonRow,
    editor_rows: RefCell<Vec<Rc<EditorRowWidgets>>>,
    editor_exercises: RefCell<Vec<Exercise>>,
    editing_plan_id: RefCell<Option<String>>,

    // Runner page widgets
    pub runner_page: NavigationPage,
    runner_stop_btn: Button,
    runner_plan_label: Label,
    runner_progress_label: Label,
    runner_workout_bar: ProgressBar,
    runner_exercise_label: Label,
    runner_phase_label: Label,
    runner_countdown: Label,
    runner_next_label: Label,
    runner_ring: ProgressRing,
    reps_revealer: Revealer,
    reps_spin: SpinRow,
    reps_done_btn: Button,
    runner_pause_btn: Button,
    runner_add30_btn: Button,
    runner_skip_btn: Button,

    // Summary page
    pub summary_page: NavigationPage,
    summary_plan_name: Label,
    summary_info: Label,
    summary_exercises_box: GtkBox,

    // State
    timer: Rc<TimerCore>,
    running_session: RefCell<Option<TrainingSession>>,
    running_plan: RefCell<Option<TrainingPlan>>,
    current_exercise_idx: Cell<usize>,
    current_round: Cell<i64>,
    current_set: Cell<i64>,
    total_rounds: Cell<i64>,
    rest_between_rounds: Cell<i64>,
    phase: Cell<Phase>,
    exercise_start_time: Cell<i64>,
    exercise_start_dt: RefCell<Option<chrono::DateTime<Local>>>,

    // Focus trackers
    list_focus: RefCell<ControllerFocusTracker>,
    editor_focus: RefCell<ControllerFocusTracker>,
    runner_focus: RefCell<ControllerFocusTracker>,

    // Weak self-reference for dialog callbacks (avoids Rc cycles)
    self_weak: RefCell<std::rc::Weak<TrainingPlanPage>>,

    // Window hooks: transient dialog registration + font refresh.
    dialog_registrar: RefCell<Option<Rc<dyn Fn(Option<(Rc<AlertDialog>, String)>)>>>,
    font_refresh_cb: RefCell<Option<Rc<dyn Fn()>>>,
}

impl TrainingPlanPage {
    pub fn new(store: Rc<DataStore>, sound: Rc<SoundPlayer>) -> Rc<Self> {
        let widget = Bin::new();
        let nav = NavigationView::new();

        // ---- List page ----
        let list_box = GtkBox::new(Orientation::Vertical, 12);
        list_box.set_margin_top(24);
        list_box.set_margin_bottom(24);
        list_box.set_margin_start(24);
        list_box.set_margin_end(24);

        let new_plan_btn = Button::builder()
            .label("New Plan")
            .css_classes(["suggested-action"])
            .halign(Align::Start)
            .build();
        list_box.append(&new_plan_btn);

        let list_stack = gtk4::Stack::new();
        list_stack.set_vhomogeneous(true);
        list_stack.set_vexpand(true);

        let scrolled = ScrolledWindow::new();
        let plan_list_box = ListBox::new();
        plan_list_box.add_css_class("boxed-list");
        plan_list_box.set_activate_on_single_click(true);
        scrolled.set_child(Some(&plan_list_box));
        list_stack.add_named(&scrolled, Some("list"));

        // Empty state: instead of a plain label, offer the two ways to start.
        let empty_box = GtkBox::new(Orientation::Vertical, 12);
        empty_box.set_vexpand(true);
        empty_box.set_valign(Align::Center);
        empty_box.set_halign(Align::Center);
        let empty_label = Label::new(Some("No plans yet"));
        empty_label.add_css_class("title-3");
        empty_box.append(&empty_label);
        let empty_hint = Label::new(Some(
            "Create a plan from scratch, or start from one of the templates.",
        ));
        empty_hint.add_css_class("dim-label");
        empty_box.append(&empty_hint);
        let empty_buttons = GtkBox::new(Orientation::Horizontal, 12);
        empty_buttons.set_halign(Align::Center);
        let empty_new_btn = Button::builder()
            .label("New Plan")
            .css_classes(["suggested-action"])
            .build();
        let empty_templates_btn = Button::builder().label("Browse Templates").build();
        empty_buttons.append(&empty_new_btn);
        empty_buttons.append(&empty_templates_btn);
        empty_box.append(&empty_buttons);
        list_stack.add_named(&empty_box, Some("empty"));

        list_box.append(&list_stack);

        let io_box = GtkBox::new(Orientation::Horizontal, 6);
        io_box.set_halign(Align::Start);
        let export_btn = Button::builder().label("Export").halign(Align::Start).build();
        io_box.append(&export_btn);
        let import_btn = Button::builder().label("Import").halign(Align::Start).build();
        io_box.append(&import_btn);
        list_box.append(&io_box);

        let list_page = NavigationPage::builder().title("Training Plans").build();
        list_page.set_child(Some(&list_box));
        nav.add(&list_page);

        // ---- Editor page ----
        let editor_toolbar = libadwaita::ToolbarView::new();
        let editor_header = libadwaita::HeaderBar::new();
        editor_header.set_show_start_title_buttons(false);
        editor_header.set_show_end_title_buttons(false);
        let editor_save_btn = Button::builder()
            .label("Save")
            .css_classes(["suggested-action"])
            .build();
        editor_header.pack_end(&editor_save_btn);
        editor_toolbar.add_top_bar(&editor_header);

        let editor_scrolled = ScrolledWindow::new();
        editor_scrolled.set_vexpand(true);
        let editor_clamp = Clamp::new();
        let editor_box = GtkBox::new(Orientation::Vertical, 12);
        editor_box.set_margin_top(12);
        editor_box.set_margin_bottom(12);
        editor_box.set_margin_start(12);
        editor_box.set_margin_end(12);

        let plan_name_entry = EntryRow::builder().title("Plan Name").build();
        let name_group = PreferencesGroup::new();
        name_group.add(&plan_name_entry);

        let favorite_switch = SwitchRow::builder()
            .title("Favorite")
            .subtitle("Show this plan at the top of the list")
            .build();
        name_group.add(&favorite_switch);

        let rounds_adj = gtk4::Adjustment::new(1.0, 1.0, 20.0, 1.0, 1.0, 0.0);
        let total_rounds_spin = SpinRow::new(Some(&rounds_adj), 1.0, 0);
        total_rounds_spin.set_title("Total Rounds");
        total_rounds_spin.set_subtitle("How many times to repeat all exercises");
        name_group.add(&total_rounds_spin);

        let rest_rounds_adj = gtk4::Adjustment::new(60.0, 0.0, 600.0, 5.0, 5.0, 0.0);
        let rest_between_rounds_spin = SpinRow::new(Some(&rest_rounds_adj), 1.0, 0);
        rest_between_rounds_spin.set_title("Rest Between Rounds (seconds)");
        rest_between_rounds_spin.set_subtitle("Pause between rounds when total rounds > 1");
        name_group.add(&rest_between_rounds_spin);

        editor_box.append(&name_group);

        let add_exercise_btn = Button::builder().label("Add Exercise").halign(Align::Start).build();
        let browse_exercise_btn = Button::builder()
            .label("Browse Exercises")
            .halign(Align::Start)
            .build();
        let add_box = GtkBox::new(Orientation::Horizontal, 12);
        add_box.append(&add_exercise_btn);
        add_box.append(&browse_exercise_btn);
        editor_box.append(&add_box);

        let exercises_group = PreferencesGroup::new();
        editor_box.append(&exercises_group);

        let stats_group = PreferencesGroup::builder()
            .title("Statistics")
            .description("Based on your recorded training history")
            .build();
        let stats_sessions_row = ActionRow::builder().title("Sessions").build();
        let stats_time_row = ActionRow::builder().title("Total Time").build();
        let stats_last_row = ActionRow::builder().title("Last Done").build();
        stats_group.add(&stats_sessions_row);
        stats_group.add(&stats_time_row);
        stats_group.add(&stats_last_row);
        stats_group.set_visible(false);
        editor_box.append(&stats_group);

        let actions_group = PreferencesGroup::new();
        let run_btn = ButtonRow::builder().title("Start Training").build();
        run_btn.add_css_class("suggested-action");
        actions_group.add(&run_btn);
        let save_row = ButtonRow::builder().title("Save Plan").build();
        actions_group.add(&save_row);
        let delete_btn = ButtonRow::builder()
            .title("Delete Plan")
            .css_classes(["destructive-action"])
            .build();
        actions_group.add(&delete_btn);
        editor_box.append(&actions_group);

        editor_clamp.set_child(Some(&editor_box));
        editor_scrolled.set_child(Some(&editor_clamp));
        editor_toolbar.set_content(Some(&editor_scrolled));

        let editor_page = NavigationPage::builder().title("Edit Plan").build();
        editor_page.set_child(Some(&editor_toolbar));

        // ---- Runner page ----
        let runner_toolbar = libadwaita::ToolbarView::new();
        let runner_header = libadwaita::HeaderBar::new();
        runner_header.set_show_start_title_buttons(false);
        runner_header.set_show_end_title_buttons(false);
        let runner_stop_btn = Button::builder().label("Stop").build();
        runner_header.pack_end(&runner_stop_btn);
        runner_toolbar.add_top_bar(&runner_header);

        let runner_box = GtkBox::new(Orientation::Vertical, 12);
        runner_box.set_vexpand(true);
        runner_box.set_margin_top(12);
        runner_box.set_margin_bottom(12);
        runner_box.set_margin_start(16);
        runner_box.set_margin_end(16);

        // Controls at the top: they must always be visible, even on short
        // windows (the ring and the exercise name can take the rest).
        let runner_pause_btn = Button::builder().label("Pause").build();
        let runner_add30_btn = Button::builder()
            .label("+30s")
            .tooltip_text("Add 30 seconds to the current timer")
            .css_classes(["flat"])
            .sensitive(false)
            .build();
        let runner_skip_btn = Button::builder()
            .label("Skip")
            .css_classes(["destructive-action"])
            .build();
        let btn_box = GtkBox::new(Orientation::Horizontal, 12);
        btn_box.set_halign(Align::Center);
        btn_box.append(&runner_pause_btn);
        btn_box.append(&runner_add30_btn);
        btn_box.append(&runner_skip_btn);
        runner_box.append(&btn_box);

        let runner_plan_label = Label::new(None);
        runner_plan_label.add_css_class("title-2");
        runner_plan_label.set_valign(Align::Start);
        runner_box.append(&runner_plan_label);

        let runner_progress_label = Label::new(Some(""));
        runner_progress_label.add_css_class("dim-label");
        runner_box.append(&runner_progress_label);

        let runner_workout_bar = ProgressBar::new();
        runner_workout_bar.add_css_class("runner-progress-bar");
        runner_workout_bar.set_show_text(false);
        runner_box.append(&runner_workout_bar);

        let runner_exercise_label = Label::new(None);
        runner_exercise_label.set_vexpand(false);
        runner_exercise_label.set_valign(Align::Center);
        runner_exercise_label.set_halign(Align::Center);
        runner_exercise_label.set_justify(gtk4::Justification::Center);
        runner_exercise_label.set_wrap(true);
        runner_exercise_label.add_css_class("title-1");
        runner_box.append(&runner_exercise_label);

        // Circular progress ring with the countdown in the middle
        let runner_ring = ProgressRing::new(300);
        let ring_overlay = gtk4::Overlay::new();
        ring_overlay.set_child(Some(runner_ring.widget()));
        let runner_countdown = Label::new(Some("00:00"));
        runner_countdown.set_halign(Align::Center);
        runner_countdown.set_valign(Align::Center);
        ring_overlay.add_overlay(&runner_countdown);
        ring_overlay.set_valign(Align::Center);
        runner_box.append(&ring_overlay);

        let runner_phase_label = Label::new(None);
        runner_phase_label.set_css_classes(&["heading", "phase-reps"]);
        runner_phase_label.set_halign(Align::Center);
        runner_phase_label.set_margin_top(6);
        runner_box.append(&runner_phase_label);

        let runner_next_label = Label::new(None);
        runner_next_label.add_css_class("dim-label");
        let next_card = GtkBox::new(Orientation::Vertical, 0);
        next_card.add_css_class("card");
        next_card.set_margin_top(6);
        next_card.set_margin_start(32);
        next_card.set_margin_end(32);
        next_card.set_margin_bottom(6);
        next_card.append(&runner_next_label);
        runner_box.append(&next_card);

        let reps_box = GtkBox::new(Orientation::Vertical, 8);
        let rep_adj = gtk4::Adjustment::new(0.0, 0.0, 9999.0, 1.0, 1.0, 0.0);
        let reps_spin = SpinRow::new(Some(&rep_adj), 1.0, 0);
        reps_spin.set_title("Reps completed");
        reps_box.append(&reps_spin);
        let reps_done_btn = Button::builder()
            .label("Done")
            .css_classes(["suggested-action"])
            .build();
        reps_box.append(&reps_done_btn);
        let reps_revealer = Revealer::new();
        reps_revealer.set_child(Some(&reps_box));
        reps_revealer.set_transition_type(gtk4::RevealerTransitionType::None);
        reps_revealer.set_reveal_child(false);
        runner_box.append(&reps_revealer);

        // Scrollable so the runner never overflows on short windows.
        let runner_scrolled = ScrolledWindow::new();
        runner_scrolled.set_vexpand(true);
        runner_scrolled.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
        runner_scrolled.set_child(Some(&runner_box));
        runner_toolbar.set_content(Some(&runner_scrolled));

        let runner_page = NavigationPage::builder().title("Training").build();
        runner_page.set_child(Some(&runner_toolbar));

        // ---- Summary page ----
        let summary_toolbar = libadwaita::ToolbarView::new();
        let summary_header = libadwaita::HeaderBar::new();
        summary_header.set_show_start_title_buttons(false);
        summary_header.set_show_end_title_buttons(false);
        summary_toolbar.add_top_bar(&summary_header);

        let summary_scrolled = ScrolledWindow::new();
        summary_scrolled.set_vexpand(true);
        let summary_clamp = Clamp::new();
        let summary_box = GtkBox::new(Orientation::Vertical, 12);
        summary_box.set_margin_top(24);
        summary_box.set_margin_bottom(24);
        summary_box.set_margin_start(24);
        summary_box.set_margin_end(24);

        let summary_title = Label::new(Some("Training Complete!"));
        summary_title.add_css_class("title-1");
        summary_box.append(&summary_title);

        let summary_plan_name = Label::new(None);
        summary_plan_name.add_css_class("title-2");
        summary_box.append(&summary_plan_name);

        let summary_info = Label::new(None);
        summary_info.add_css_class("dim-label");
        summary_box.append(&summary_info);

        let summary_exercises_box = GtkBox::new(Orientation::Vertical, 6);
        let ex_scrolled = ScrolledWindow::new();
        ex_scrolled.set_vexpand(true);
        ex_scrolled.set_child(Some(&summary_exercises_box));
        summary_box.append(&ex_scrolled);

        summary_clamp.set_child(Some(&summary_box));
        summary_scrolled.set_child(Some(&summary_clamp));
        summary_toolbar.set_content(Some(&summary_scrolled));

        let summary_page = NavigationPage::builder().title("Summary").build();
        summary_page.set_child(Some(&summary_toolbar));

        nav.add(&editor_page);
        nav.add(&runner_page);
        nav.add(&summary_page);

        widget.set_child(Some(&nav));

        let timer = Rc::new(TimerCore::new());

        let page = Rc::new(Self {
            widget,
            store: Rc::clone(&store),
            sound,
            nav: nav.clone(),
            plans: RefCell::new(Vec::new()),
            list_page: list_page.clone(),
            list_stack: list_stack.clone(),
            plan_list_box: plan_list_box.clone(),
            new_plan_btn: new_plan_btn.clone(),
            export_btn: export_btn.clone(),
            import_btn: import_btn.clone(),
            empty_new_btn: empty_new_btn.clone(),
            empty_templates_btn: empty_templates_btn.clone(),
            star_buttons: RefCell::new(HashMap::new()),
            editor_page: editor_page.clone(),
            plan_name_entry: plan_name_entry.clone(),
            favorite_switch: favorite_switch.clone(),
            total_rounds_spin: total_rounds_spin.clone(),
            rest_between_rounds_spin: rest_between_rounds_spin.clone(),
            exercises_group: exercises_group.clone(),
            editor_stats_group: stats_group.clone(),
            editor_stats_sessions: stats_sessions_row.clone(),
            editor_stats_time: stats_time_row.clone(),
            editor_stats_last: stats_last_row.clone(),
            add_exercise_btn: add_exercise_btn.clone(),
            browse_exercise_btn: browse_exercise_btn.clone(),
            run_btn: run_btn.clone(),
            save_row: save_row.clone(),
            delete_btn: delete_btn.clone(),
            editor_rows: RefCell::new(Vec::new()),
            editor_exercises: RefCell::new(Vec::new()),
            editing_plan_id: RefCell::new(None),
            runner_page: runner_page.clone(),
            runner_stop_btn: runner_stop_btn.clone(),
            runner_plan_label: runner_plan_label.clone(),
            runner_progress_label: runner_progress_label.clone(),
            runner_workout_bar: runner_workout_bar.clone(),
            runner_exercise_label: runner_exercise_label.clone(),
            runner_phase_label: runner_phase_label.clone(),
            runner_countdown: runner_countdown.clone(),
            runner_next_label: runner_next_label.clone(),
            runner_ring,
            reps_revealer: reps_revealer.clone(),
            reps_spin: reps_spin.clone(),
            reps_done_btn: reps_done_btn.clone(),
            runner_pause_btn: runner_pause_btn.clone(),
            runner_add30_btn: runner_add30_btn.clone(),
            runner_skip_btn: runner_skip_btn.clone(),
            summary_page: summary_page.clone(),
            summary_plan_name: summary_plan_name.clone(),
            summary_info: summary_info.clone(),
            summary_exercises_box: summary_exercises_box.clone(),
            timer: Rc::clone(&timer),
            running_session: RefCell::new(None),
            running_plan: RefCell::new(None),
            current_exercise_idx: Cell::new(0),
            current_round: Cell::new(1),
            current_set: Cell::new(1),
            total_rounds: Cell::new(1),
            rest_between_rounds: Cell::new(0),
            phase: Cell::new(Phase::Exercise),
            exercise_start_time: Cell::new(0),
            exercise_start_dt: RefCell::new(None),
            list_focus: RefCell::new(ControllerFocusTracker::new()),
            editor_focus: RefCell::new(ControllerFocusTracker::new()),
            runner_focus: RefCell::new(ControllerFocusTracker::new()),
            self_weak: RefCell::new(std::rc::Weak::new()),
            dialog_registrar: RefCell::new(None),
            font_refresh_cb: RefCell::new(None),
        });
        *page.self_weak.borrow_mut() = Rc::downgrade(&page);
        page.connect_signals(Rc::downgrade(&page));
        page.refresh_plans();
        page
    }

    pub fn widget(&self) -> &Bin {
        &self.widget
    }

    /// The first stored plan (used by the debug screenshot mode).
    pub fn first_plan(&self) -> Option<TrainingPlan> {
        self.store.load_plans().into_iter().next()
    }

    /// Stop a running workout without saving the session (debug mode).
    pub fn abort_training(&self) {
        self.timer.stop();
        *self.running_session.borrow_mut() = None;
        *self.running_plan.borrow_mut() = None;
        self.show_list();
    }

    /// Window hook so controller events reach transient AlertDialogs.
    pub fn set_dialog_registrar(
        &self,
        cb: Rc<dyn Fn(Option<(Rc<AlertDialog>, String)>)>,
    ) {
        *self.dialog_registrar.borrow_mut() = Some(cb);
    }

    /// Window hook to re-apply scaled fonts when a nav sub-page opens
    /// (the runner lives inside the plans tab, below window-level wiring).
    pub fn set_font_refresh_callback(&self, cb: Rc<dyn Fn()>) {
        *self.font_refresh_cb.borrow_mut() = Some(cb);
    }

    fn register_dialog(&self, dlg: Option<(Rc<AlertDialog>, String)>) {
        let cb = self.dialog_registrar.borrow().clone();
        if let Some(cb) = cb {
            cb(dlg);
        }
    }

    fn refresh_fonts(&self) {
        if let Some(cb) = self.font_refresh_cb.borrow().clone() {
            cb();
        }
    }

    pub fn nav(&self) -> &NavigationView {
        &self.nav
    }

    pub fn visible_page(&self) -> &'static str {
        if let Some(page) = self.nav.visible_page() {
            let runner = &self.runner_page;
            let editor = &self.editor_page;
            let summary = &self.summary_page;
            let list = &self.list_page;
            if page == *runner {
                return "runner";
            } else if page == *editor {
                return "editor";
            } else if page == *summary {
                return "summary";
            } else if page == *list {
                return "list";
            }
        }
        ""
    }

    pub fn on_page_visible(&self) {
        if self.visible_page() == "list" {
            self.focus_list_if_visible();
        }
    }

    pub fn open_plan(&self, plan: &TrainingPlan) {
        self.show_editor(Some(plan));
    }

    pub fn page_weak(&self) -> std::rc::Weak<TrainingPlanPage> {
        self.self_weak.borrow().clone()
    }

    // ---------- Signals ----------

    fn connect_signals(&self, page_weak: std::rc::Weak<TrainingPlanPage>) {
        // Timer tick
        {
            let page_weak = page_weak.clone();
            self.timer.on_tick.borrow_mut().replace(Box::new(
                move |remaining, total| {
                    if let Some(p) = page_weak.upgrade() {
                        p.on_runner_tick(remaining, total);
                    }
                },
            ));
        }

        // Timer finished
        {
            let page_weak = page_weak.clone();
            self.timer
                .on_finished
                .borrow_mut()
                .replace(Box::new(move || {
                    if let Some(p) = page_weak.upgrade() {
                        p.on_timer_finished();
                    }
                }));
        }

        // New plan button
        {
            let page_weak = page_weak.clone();
            self.new_plan_btn.connect_clicked(move |_| {
                if let Some(p) = page_weak.upgrade() {
                    p.show_editor(None);
                }
            });
        }

        // Empty state buttons
        {
            let page_weak = page_weak.clone();
            self.empty_new_btn.connect_clicked(move |_| {
                if let Some(p) = page_weak.upgrade() {
                    p.show_editor(None);
                }
            });
        }
        {
            let page_weak = page_weak.clone();
            self.empty_templates_btn.connect_clicked(move |_| {
                if let Some(p) = page_weak.upgrade() {
                    p.show_template_picker();
                }
            });
        }

        // Plan activated
        {
            let page_weak = page_weak.clone();
            self.plan_list_box
                .connect_row_activated(move |_lb, row| {
                    let Some(p) = page_weak.upgrade() else { return };
                    let idx = row.index();
                    let plans = p.plans.borrow();
                    if idx >= 0 && (idx as usize) < plans.len() {
                        let plan = plans[idx as usize].clone();
                        drop(plans);
                        p.show_editor(Some(&plan));
                    }
                });
        }

        // Export/Import
        {
            let page_weak = page_weak.clone();
            self.export_btn.connect_clicked(move |_| {
                if let Some(p) = page_weak.upgrade() {
                    p.on_export_plans();
                }
            });
        }
        {
            let page_weak = page_weak.clone();
            self.import_btn.connect_clicked(move |_| {
                if let Some(p) = page_weak.upgrade() {
                    p.on_import_plans();
                }
            });
        }

        // Favorite switch
        {
            let page_weak = page_weak.clone();
            self.favorite_switch
                .connect_notify_local(Some("active"), move |sw, _| {
                    let Some(p) = page_weak.upgrade() else { return };
                    let opt = p.editing_plan_id.borrow().clone();
                    let Some(plan_id) = opt else { return };
                    if let Some(mut plan) = p.store.get_plan(&plan_id) {
                        let new_val = sw.is_active();
                        if plan.is_favorite != new_val {
                            plan.is_favorite = new_val;
                            p.store.save_plan(&plan);
                        }
                    }
                });
        }

        // Add exercise buttons
        {
            let page_weak = page_weak.clone();
            self.add_exercise_btn.connect_clicked(move |_| {
                if let Some(p) = page_weak.upgrade() {
                    p.add_exercise(None);
                }
            });
        }
        {
            let page_weak = page_weak.clone();
            self.browse_exercise_btn.connect_clicked(move |_| {
                if let Some(p) = page_weak.upgrade() {
                    p.on_browse_add_exercise();
                }
            });
        }

        // Save / Delete / Run rows + buttons
        {
            let page_weak = page_weak.clone();
            self.save_row.connect_activated(move |_| {
                if let Some(p) = page_weak.upgrade() {
                    p.on_save_plan();
                }
            });
        }
        {
            let page_weak = page_weak.clone();
            self.run_btn.connect_activated(move |_| {
                if let Some(p) = page_weak.upgrade() {
                    p.on_run_plan();
                }
            });
        }
        {
            let page_weak = page_weak.clone();
            self.delete_btn.connect_activated(move |_| {
                if let Some(p) = page_weak.upgrade() {
                    p.on_confirm_delete_plan();
                }
            });
        }

        // Runner buttons
        {
            let page_weak = page_weak.clone();
            self.runner_stop_btn.connect_clicked(move |_| {
                if let Some(p) = page_weak.upgrade() {
                    p.confirm_stop_workout();
                }
            });
        }
        {
            let page_weak = page_weak.clone();
            self.runner_pause_btn.connect_clicked(move |_| {
                if let Some(p) = page_weak.upgrade() {
                    p.on_runner_pause();
                }
            });
        }
        {
            let page_weak = page_weak.clone();
            self.runner_add30_btn.connect_clicked(move |_| {
                if let Some(p) = page_weak.upgrade() {
                    p.on_runner_add30();
                }
            });
        }
        {
            let page_weak = page_weak.clone();
            self.runner_skip_btn.connect_clicked(move |_| {
                if let Some(p) = page_weak.upgrade() {
                    p.on_runner_skip();
                }
            });
        }
        {
            let page_weak = page_weak.clone();
            self.reps_done_btn.connect_clicked(move |_| {
                if let Some(p) = page_weak.upgrade() {
                    p.log_exercise(true);
                }
            });
        }
        // Editor header save button
    }

    // ---------- Plans list ----------

    pub fn refresh_plans(&self) {
        // Always pop index 0: after removing a row the remaining rows
        // shift down, so advancing the index instead would skip every
        // second row and leave stale duplicates behind.
        while let Some(row) = self.plan_list_box.row_at_index(0) {
            self.plan_list_box.remove(&row);
        }
        self.star_buttons.borrow_mut().clear();

        let mut plans = self.store.load_plans();
        plans.sort_by(|a, b| {
            let fa = if a.is_favorite { 0 } else { 1 };
            let fb = if b.is_favorite { 0 } else { 1 };
            fa.cmp(&fb).then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        self.plans.replace(plans.clone());

        if plans.is_empty() {
            self.list_stack.set_visible_child_name("empty");
            self.focus_list_if_visible();
            return;
        }

        for plan in &plans {
            let rounds_str = if plan.total_rounds > 1 {
                format!(" ({} rounds)", plan.total_rounds)
            } else {
                String::new()
            };
            let rbr_str =
                if plan.total_rounds > 1 && plan.rest_between_rounds_seconds > 0 {
                    format!(", {}s rest between rounds", plan.rest_between_rounds_seconds)
                } else {
                    String::new()
                };
            let row = ActionRow::builder()
                .title(&plan.name)
                .subtitle(format!(
                    "{} exercises{}{}",
                    plan.exercises.len(),
                    rounds_str,
                    rbr_str
                ))
                .activatable(true)
                .build();

            let icon_name = if plan.is_favorite {
                "starred-symbolic"
            } else {
                "non-starred-symbolic"
            };
            let star_btn = Button::builder()
                .icon_name(icon_name)
                .css_classes(["flat"])
                .tooltip_text("Toggle favorite")
                .build();
            star_btn.set_valign(Align::Center);
            let store = Rc::clone(&self.store);
            let plan_id = plan.id.clone();
            let page_weak = self.self_weak.borrow().clone();
            star_btn.connect_clicked(move |_| {
                let Some(p) = page_weak.upgrade() else { return };
                if let Some(mut plan) = store.get_plan(&plan_id) {
                    plan.is_favorite = !plan.is_favorite;
                    store.save_plan(&plan);
                }
                // Refresh immediately so the star icon and the
                // favorites-first order update (they used to lag until the
                // next reload), and keep focus on the same plan row.
                p.refresh_plans();
                p.focus_plan_row(&plan_id);
            });
            row.add_suffix(&star_btn);
            self.star_buttons
                .borrow_mut()
                .insert(plan.id.clone(), star_btn.upcast());

            let dup_btn = Button::builder()
                .icon_name("edit-copy-symbolic")
                .css_classes(["flat"])
                .tooltip_text("Duplicate plan")
                .build();
            dup_btn.set_valign(Align::Center);
            {
                let page_weak = self.self_weak.borrow().clone();
                let plan_clone = plan.clone();
                dup_btn.connect_clicked(move |_| {
                    if let Some(p) = page_weak.upgrade() {
                        p.duplicate_plan(&plan_clone);
                    }
                });
            }
            row.add_suffix(&dup_btn);

            // "Progress this plan": create a copy with increased loads based on
            // the last session of this plan.
            {
                let plan_clone = plan.clone();
                let page_weak = self.self_weak.borrow().clone();
                let prog_btn = Button::builder()
                    .icon_name("media-play-forward-symbolic")
                    .css_classes(["flat"])
                    .tooltip_text("Progress this plan (more reps/weight)")
                    .build();
                prog_btn.set_valign(Align::Center);
                prog_btn.connect_clicked(move |_| {
                    if let Some(p) = page_weak.upgrade() {
                        p.progress_plan(&plan_clone);
                    }
                });
                row.add_suffix(&prog_btn);
            }

            self.plan_list_box.append(&row);
        }

        self.list_stack.set_visible_child_name("list");
        self.focus_list_if_visible();
    }

    fn focus_list_if_visible(&self) {
        if self.visible_page() != "list" {
            return;
        }
        let mut widgets: Vec<gtk4::Widget> = Vec::new();
        widgets.push(self.new_plan_btn.clone().upcast());
        // Visual order: the plan rows sit between "New Plan" (top) and the
        // Export/Import buttons (bottom).
        if !self.plans.borrow().is_empty() {
            let mut idx = 0;
            while let Some(row) = self.plan_list_box.row_at_index(idx) {
                widgets.push(row.upcast());
                idx += 1;
            }
        }
        widgets.push(self.export_btn.clone().upcast());
        widgets.push(self.import_btn.clone().upcast());
        self.list_focus.borrow_mut().set_widgets(widgets);
        self.list_focus.borrow_mut().focus_first();
    }

    /// Move the list focus onto the row of the given plan (after a
    /// refresh re-sorted the list).
    fn focus_plan_row(&self, plan_id: &str) {
        if self.visible_page() != "list" {
            return;
        }
        let idx = self.plans.borrow().iter().position(|p| p.id == plan_id);
        if let Some(idx) = idx {
            if let Some(row) = self.plan_list_box.row_at_index(idx as i32) {
                let row = row.upcast::<gtk4::Widget>();
                self.list_focus.borrow_mut().move_to_widget(&row);
            }
        }
    }

    /// The plan under the list focus (rows and `plans` share the sorted
    /// order), for the controller shortcut buttons.
    fn focused_list_plan(&self) -> Option<TrainingPlan> {
        let widget = self.list_focus.borrow().current_widget()?;
        let row = widget.downcast_ref::<ActionRow>()?;
        let idx = row.index();
        let plans = self.plans.borrow();
        if idx >= 0 && (idx as usize) < plans.len() {
            Some(plans[idx as usize].clone())
        } else {
            None
        }
    }

    /// Duplicate a plan (row copy button and the Y shortcut).
    fn duplicate_plan(&self, plan: &TrainingPlan) {
        let mut new_plan = plan.clone();
        new_plan.id = uuid::Uuid::new_v4().to_string();
        new_plan.name = format!("{} (copy)", new_plan.name);
        new_plan.is_favorite = false;
        self.store.save_plan(&new_plan);
        self.refresh_plans();
        self.focus_plan_row(&new_plan.id);
        crate::window::show_app_toast(&format!("Duplicated \"{}\"", plan.name));
    }

    /// "Progress this plan": create a copy with increased loads based on
    /// the last session of this plan (row button and the R2 shortcut).
    fn progress_plan(&self, plan: &TrainingPlan) {
        let sessions = self.store.load_sessions();
        let Some(last) = crate::analytics::last_session_for_plan(&sessions, plan) else {
            crate::window::show_app_toast(
                "Train this plan once to unlock automatic progression",
            );
            return;
        };
        let new_plan = crate::analytics::progressed_plan(plan, last);
        self.store.save_plan(&new_plan);
        self.refresh_plans();
        self.focus_plan_row(&new_plan.id);
        let store = Rc::clone(&self.store);
        let new_plan_clone = new_plan.clone();
        crate::window::show_app_toast_action(
            &format!("Created \"{}\"", new_plan.name),
            "Undo",
            Rc::new(move || {
                store.delete_plan(&new_plan_clone.id);
                crate::window::refresh_all_pages();
            }),
        );
    }

    // ---------- Editor ----------

    pub fn show_editor(&self, plan: Option<&TrainingPlan>) {
        self.clear_editor_rows();
        self.editor_exercises.borrow_mut().clear();

        match plan {
            Some(plan) => {
                *self.editing_plan_id.borrow_mut() = Some(plan.id.clone());
                self.plan_name_entry.set_text(&plan.name);
                self.favorite_switch.set_active(plan.is_favorite);
                self.total_rounds_spin
                    .set_value(plan.total_rounds.max(1) as f64);
                self.rest_between_rounds_spin
                    .set_value(plan.rest_between_rounds_seconds.max(0) as f64);
                for e in &plan.exercises {
                    self.editor_exercises.borrow_mut().push(e.clone());
                }
                self.editor_page.set_title("Edit Plan");
            }
            None => {
                *self.editing_plan_id.borrow_mut() = None;
                self.plan_name_entry.set_text("");
                self.favorite_switch.set_active(false);
                self.total_rounds_spin.set_value(1.0);
                self.rest_between_rounds_spin.set_value(60.0);
                self.editor_page.set_title("New Plan");
            }
        }

        let exercises = self.editor_exercises.borrow().clone();
        for ex in exercises {
            self.append_exercise_row(ex);
        }

        self.update_editor_stats();

        self.nav.push(&self.editor_page);
        self.refresh_editor_focus();
        self.editor_focus.borrow_mut().focus_first();
    }

    /// Per-plan stats from recorded history (hidden for unsaved/new plans).
    fn update_editor_stats(&self) {
        let Some(plan_id) = self.editing_plan_id.borrow().clone() else {
            self.editor_stats_group.set_visible(false);
            return;
        };
        let plan_name = self.plan_name_entry.text().to_string();
        let sessions: Vec<_> = self
            .store
            .load_sessions()
            .into_iter()
            .filter(|s| {
                s.plan_id == plan_id || (s.plan_id.is_empty() && s.plan_name == plan_name)
            })
            .collect();

        if sessions.is_empty() {
            self.editor_stats_group.set_visible(false);
            return;
        }

        let total_secs: i64 = sessions.iter().map(|s| s.total_actual_seconds).sum();
        let total_h = total_secs / 3600;
        let total_m = (total_secs % 3600) / 60;
        let total_str = if total_h > 0 {
            format!("{}h {}m", total_h, total_m)
        } else {
            format!("{}m", total_m)
        };
        let last = sessions.iter().map(|s| s.started_at).max();

        self.editor_stats_sessions
            .set_subtitle(&format!("{}", sessions.len()));
        self.editor_stats_time.set_subtitle(&total_str);
        self.editor_stats_last.set_subtitle(
            &last
                .map(|d| d.format("%d.%m.%Y").to_string())
                .unwrap_or_else(|| "-".to_string()),
        );
        self.editor_stats_group.set_visible(true);
    }

    fn clear_editor_rows(&self) {
        let rows: Vec<Rc<EditorRowWidgets>> = self.editor_rows.borrow().clone();
        for r in rows {
            self.exercises_group.remove(&r.row);
        }
        self.editor_rows.borrow_mut().clear();
    }

    fn append_exercise_row(&self, exercise: Exercise) {
        let row = ExpanderRow::builder()
            .title(display_title(&exercise))
            .build();
        row.set_expanded(false);

        let name_entry = EntryRow::builder().title("Name").build();
        name_entry.set_text(&exercise.name);
        row.add_row(&name_entry);

        let dur_adj = gtk4::Adjustment::new(
            exercise.duration_seconds.unwrap_or(0) as f64,
            0.0,
            3600.0,
            5.0,
            10.0,
            0.0,
        );
        let dur_spin = SpinRow::new(Some(&dur_adj), 1.0, 0);
        dur_spin.set_title("Duration (seconds)");
        row.add_row(&dur_spin);

        let reps_adj = gtk4::Adjustment::new(
            exercise.reps.unwrap_or(0) as f64,
            0.0,
            9999.0,
            1.0,
            10.0,
            0.0,
        );
        let reps_spin = SpinRow::new(Some(&reps_adj), 1.0, 0);
        reps_spin.set_title("Reps");
        row.add_row(&reps_spin);

        let sets_adj = gtk4::Adjustment::new(
            exercise.sets.max(1) as f64,
            1.0,
            20.0,
            1.0,
            10.0,
            0.0,
        );
        let sets_spin = SpinRow::new(Some(&sets_adj), 1.0, 0);
        sets_spin.set_title("Sets");
        sets_spin.set_subtitle("Number of times to repeat this exercise");
        row.add_row(&sets_spin);

        let weight_adj = gtk4::Adjustment::new(
            exercise.weight_kg.unwrap_or(0.0),
            0.0,
            999.0,
            0.5,
            1.0,
            0.0,
        );
        let weight_spin = SpinRow::new(Some(&weight_adj), 1.0, 1);
        weight_spin.set_title("Weight (kg)");
        row.add_row(&weight_spin);

        let rest_adj =
            gtk4::Adjustment::new(exercise.rest_seconds as f64, 0.0, 600.0, 5.0, 10.0, 0.0);
        let rest_spin = SpinRow::new(Some(&rest_adj), 1.0, 0);
        rest_spin.set_title("Rest after (seconds)");
        row.add_row(&rest_spin);

        let reorder_box = GtkBox::new(Orientation::Horizontal, 6);
        reorder_box.set_margin_top(6);
        let up_btn = Button::builder()
            .icon_name("go-up-symbolic")
            .css_classes(["flat"])
            .build();
        reorder_box.append(&up_btn);
        let down_btn = Button::builder()
            .icon_name("go-down-symbolic")
            .css_classes(["flat"])
            .build();
        reorder_box.append(&down_btn);
        row.add_row(&reorder_box);

        let remove_btn = Button::builder()
            .label("Remove")
            .css_classes(["destructive-action"])
            .build();
        remove_btn.set_margin_top(6);
        row.add_row(&remove_btn);

        let widgets = Rc::new(EditorRowWidgets {
            row: row.clone(),
            name_entry: name_entry.clone(),
            dur_spin: dur_spin.clone(),
            reps_spin: reps_spin.clone(),
            sets_spin: sets_spin.clone(),
            weight_spin: weight_spin.clone(),
            rest_spin: rest_spin.clone(),
            remove_btn: remove_btn.clone(),
            up_btn: up_btn.clone(),
            down_btn: down_btn.clone(),
        });

        let page_weak = self.page_weak();
        let w = Rc::clone(&widgets);

        // Sync signals
        name_entry.connect_changed(move |_| {
            if let Some(p) = page_weak.upgrade() {
                p.sync_exercise_from_row(&w);
                p.refresh_editor_focus();
            }
        });
        for spin in [&dur_spin, &reps_spin, &sets_spin, &weight_spin, &rest_spin] {
            let page_weak2 = self.page_weak();
            let w2 = Rc::clone(&widgets);
            spin.connect_changed(move |_| {
                if let Some(p) = page_weak2.upgrade() {
                    p.sync_exercise_from_row(&w2);
                    p.refresh_editor_focus();
                }
            });
        }

        {
            let page_weak = self.page_weak();
            let w = Rc::clone(&widgets);
            remove_btn.connect_clicked(move |_| {
                if let Some(p) = page_weak.upgrade() {
                    p.remove_exercise_row(&w);
                }
            });
        }
        {
            let page_weak = self.page_weak();
            let w = Rc::clone(&widgets);
            up_btn.connect_clicked(move |_| {
                if let Some(p) = page_weak.upgrade() {
                    p.move_exercise(&w, -1);
                }
            });
        }
        {
            let page_weak = self.page_weak();
            let w = Rc::clone(&widgets);
            down_btn.connect_clicked(move |_| {
                if let Some(p) = page_weak.upgrade() {
                    p.move_exercise(&w, 1);
                }
            });
        }
        {
            let page_weak = self.page_weak();
            row.connect_notify_local(Some("expanded"), move |_, _| {
                if let Some(p) = page_weak.upgrade() {
                    p.refresh_editor_focus();
                }
            });
        }

        self.editor_rows.borrow_mut().push(widgets);
        self.exercises_group.add(&row);
    }

    fn row_index(&self, w: &EditorRowWidgets) -> Option<usize> {
        self.editor_rows
            .borrow()
            .iter()
            .position(|r| r.row.as_ptr() == w.row.as_ptr())
    }

    fn sync_exercise_from_row(&self, w: &Rc<EditorRowWidgets>) {
        let Some(idx) = self.row_index(w) else { return };
        let mut exercises = self.editor_exercises.borrow_mut();
        let Some(exercise) = exercises.get_mut(idx) else { return };
        exercise.name = w.name_entry.text().to_string();
        let dur_val = w.dur_spin.value() as i64;
        exercise.duration_seconds = if dur_val > 0 { Some(dur_val) } else { None };
        let reps_val = w.reps_spin.value() as i64;
        exercise.reps = if reps_val > 0 { Some(reps_val) } else { None };
        exercise.sets = (w.sets_spin.value() as i64).max(1);
        let weight_val = w.weight_spin.value();
        exercise.weight_kg = if weight_val > 0.0 { Some(weight_val) } else { None };
        exercise.rest_seconds = w.rest_spin.value() as i64;
        let title = display_title(exercise);
        drop(exercises);
        w.row.set_title(&title);
    }

    fn remove_exercise_row(&self, w: &Rc<EditorRowWidgets>) {
        if let Some(idx) = self.row_index(w) {
            self.editor_exercises.borrow_mut().remove(idx);
            self.editor_rows.borrow_mut().remove(idx);
        }
        self.exercises_group.remove(&w.row);
        self.refresh_editor_focus();
    }

    fn move_exercise(&self, w: &Rc<EditorRowWidgets>, direction: i32) {
        let Some(idx) = self.row_index(w) else { return };
        let new_idx = idx as i64 + direction as i64;
        if new_idx < 0 || new_idx as usize >= self.editor_rows.borrow().len() {
            return;
        }
        let new_idx = new_idx as usize;
        self.sync_all_from_rows();
        self.editor_exercises.borrow_mut().swap(idx, new_idx);
        self.rebuild_exercise_group();
    }

    fn sync_all_from_rows(&self) {
        let rows: Vec<Rc<EditorRowWidgets>> = self.editor_rows.borrow().clone();
        let mut exercises = self.editor_exercises.borrow_mut();
        for (i, w) in rows.iter().enumerate() {
            if let Some(exercise) = exercises.get_mut(i) {
                exercise.name = w.name_entry.text().to_string();
                let dur_val = w.dur_spin.value() as i64;
                exercise.duration_seconds = if dur_val > 0 { Some(dur_val) } else { None };
                let reps_val = w.reps_spin.value() as i64;
                exercise.reps = if reps_val > 0 { Some(reps_val) } else { None };
                exercise.sets = (w.sets_spin.value() as i64).max(1);
                let weight_val = w.weight_spin.value();
                exercise.weight_kg = if weight_val > 0.0 { Some(weight_val) } else { None };
                exercise.rest_seconds = w.rest_spin.value() as i64;
            }
        }
    }

    fn rebuild_exercise_group(&self) {
        self.clear_editor_rows();
        let exercises = self.editor_exercises.borrow().clone();
        for exercise in exercises {
            self.append_exercise_row(exercise);
        }
        self.refresh_editor_focus();
    }

    fn refresh_editor_focus(&self) {
        let mut widgets: Vec<gtk4::Widget> = Vec::new();
        widgets.push(self.plan_name_entry.clone().upcast());
        widgets.push(self.favorite_switch.clone().upcast());
        widgets.push(self.total_rounds_spin.clone().upcast());
        widgets.push(self.rest_between_rounds_spin.clone().upcast());
        widgets.push(self.add_exercise_btn.clone().upcast());
        widgets.push(self.browse_exercise_btn.clone().upcast());
        for row in self.editor_rows.borrow().iter() {
            widgets.push(row.row.clone().upcast());
            if row.row.is_expanded() {
                widgets.push(row.name_entry.clone().upcast());
                widgets.push(row.dur_spin.clone().upcast());
                widgets.push(row.reps_spin.clone().upcast());
                widgets.push(row.sets_spin.clone().upcast());
                widgets.push(row.weight_spin.clone().upcast());
                widgets.push(row.rest_spin.clone().upcast());
                widgets.push(row.remove_btn.clone().upcast());
                widgets.push(row.up_btn.clone().upcast());
                widgets.push(row.down_btn.clone().upcast());
            }
        }
        widgets.push(self.run_btn.clone().upcast());
        widgets.push(self.save_row.clone().upcast());
        widgets.push(self.delete_btn.clone().upcast());
        self.editor_focus.borrow_mut().set_widgets(widgets);
    }

    fn on_save_plan(&self) {
        let name = self.plan_name_entry.text().trim().to_string();
        if name.is_empty() {
            return;
        }

        self.sync_all_from_rows();

        let exercises: Vec<Exercise> = self
            .editor_exercises
            .borrow()
            .iter()
            .filter(|e| !e.name.trim().is_empty())
            .cloned()
            .collect();
        let total_rounds = self.total_rounds_spin.value() as i64;
        let rest_between = self.rest_between_rounds_spin.value() as i64;
        let is_favorite = self.favorite_switch.is_active();

        if let Some(plan_id) = self.editing_plan_id.borrow().as_ref().cloned() {
                if let Some(mut plan) = self.store.get_plan(&plan_id) {
                    plan.name = name;
                    plan.is_favorite = is_favorite;
                    plan.exercises = exercises;
                    plan.total_rounds = total_rounds;
                    plan.rest_between_rounds_seconds = rest_between;
                    self.store.save_plan(&plan);
                    crate::window::show_app_toast("Plan saved");
                }
        } else {
            let plan = TrainingPlan {
                name,
                is_favorite,
                total_rounds,
                rest_between_rounds_seconds: rest_between,
                exercises,
                ..Default::default()
            };
            self.store.save_plan(&plan);
            crate::window::show_app_toast("Plan saved");
            *self.editing_plan_id.borrow_mut() = Some(plan.id.clone());
        }
    }

    /// Delete the plan being edited. The deletion happens immediately and can
    /// be undone from the toast (no confirmation dialog needed).
    fn on_confirm_delete_plan(&self) {
        let plan_id = self.editing_plan_id.borrow().clone();
        let Some(plan_id) = plan_id else { return };
        let Some(plan) = self.store.get_plan(&plan_id) else { return };

        self.store.delete_plan(&plan_id);
        *self.editing_plan_id.borrow_mut() = None;
        self.show_list();

        let store = Rc::clone(&self.store);
        crate::window::show_app_toast_action(
            &format!("\"{}\" deleted", plan.name),
            "Undo",
            Rc::new(move || {
                store.save_plan(&plan);
                crate::window::refresh_all_pages();
            }),
        );
    }

    /// Show the bundled starter plans; picking one adds it to the user's plans.
    fn show_template_picker(&self) {
        let templates = crate::image_utils::load_default_plans();
        if templates.is_empty() {
            crate::window::show_app_toast("No templates available");
            return;
        }

        let window = libadwaita::Window::builder()
            .title("Plan Templates")
            .modal(true)
            .default_width(520)
            .default_height(560)
            .build();

        let header = libadwaita::HeaderBar::new();
        let cancel_btn = gtk4::Button::builder()
            .label("Cancel")
            .css_classes(["flat"])
            .build();
        header.pack_start(&cancel_btn);

        let list_box = gtk4::ListBox::new();
        list_box.add_css_class("boxed-list");
        list_box.set_selection_mode(gtk4::SelectionMode::None);

        for template in templates {
            let row = ActionRow::builder()
                .title(&template.name)
                .subtitle(format!(
                    "{} exercises · {} rounds",
                    template.exercises.len(),
                    template.total_rounds.max(1)
                ))
                .activatable(true)
                .build();
            let store = Rc::clone(&self.store);
            let plan = template.clone();
            let window_clone = window.clone();
            row.connect_activated(move |_| {
                let mut new_plan = plan.clone();
                new_plan.id = uuid::Uuid::new_v4().to_string();
                new_plan.created = chrono::Local::now();
                new_plan.is_favorite = false;
                store.save_plan(&new_plan);
                window_clone.close();
                crate::window::refresh_all_pages();
                crate::window::show_app_toast(&format!("Added \"{}\"", new_plan.name));
            });
            list_box.append(&row);
        }

        let scrolled = gtk4::ScrolledWindow::new();
        scrolled.set_child(Some(&list_box));
        scrolled.set_vexpand(true);

        let toolbar = libadwaita::ToolbarView::new();
        toolbar.add_top_bar(&header);
        toolbar.set_content(Some(&scrolled));
        window.set_content(Some(&toolbar));

        {
            let window_clone = window.clone();
            cancel_btn.connect_clicked(move |_| {
                window_clone.close();
            });
        }

        if let Some(root) = self.widget.root() {
            if let Some(parent) = root.dynamic_cast_ref::<gtk4::Window>() {
                window.set_transient_for(Some(parent));
            }
        }
        window.present();
    }

    fn self_weak_holder(&self) -> std::rc::Weak<TrainingPlanPage> {
        self.page_weak()
    }

    fn show_list(&self) {
        while self.nav.visible_page().is_some() && self.visible_page() != "list" {
            self.nav.pop();
        }
        self.refresh_plans();
    }

    fn on_run_plan(&self) {
        let name = self.plan_name_entry.text().trim().to_string();
        if name.is_empty() || self.editor_exercises.borrow().is_empty() {
            return;
        }
        self.on_save_plan();

        let mut plan = self
            .editing_plan_id
            .borrow()
            .as_ref()
            .and_then(|id| self.store.get_plan(id));
        if plan.is_none() {
            plan = self.store.load_plans().into_iter().find(|p| p.name == name);
        }

        if let Some(p) = plan {
            if !p.exercises.is_empty() {
                self.start_training(&p);
            }
        }
    }

    fn on_browse_add_exercise(&self) {
        let page_weak_holder = self.self_weak_holder();
        let on_selected = move |name: &str, _image_key: Option<&str>| {
            if let Some(p) = page_weak_holder.upgrade() {
                p.add_exercise(None);
                // Set the name on just-appended exercise
                let mut exercises = p.editor_exercises.borrow_mut();
                if let Some(last) = exercises.last_mut() {
                    last.name = name.to_string();
                }
                drop(exercises);
                // Rebuild display titles
                let rows = p.editor_rows.borrow();
                if let Some(last_row) = rows.last() {
                    let exercises = p.editor_exercises.borrow_mut();
                    if let Some(idx) = exercises.len().checked_sub(1) {
                        let title = display_title(&exercises[idx]);
                        drop(exercises);
                        last_row.row.set_title(&title);
                    }
                }
                drop(rows);
            }
        };
        let picker = crate::exercise_picker::ExercisePicker::new(on_selected);
        picker.present_over_window();
    }

        fn on_export_plans(&self) {
        let Some(root_widget) = self.widget.root().and_then(|r| r.dynamic_cast_ref::<gtk4::Window>().cloned()) else { return };
        let store = Rc::clone(&self.store);
        crate::dialogs::save_dialog(
            &root_widget,
            "Export Training Plans",
            "training-plans.json",
            "*.json",
            "JSON files",
            move |path| {
                if let Some(path) = path {
                    if let Err(e) = export_plans_json(&store.load_plans(), &path) {
                        eprintln!("Export failed: {}", e);
                    } else {
                        crate::window::show_app_toast("Plans exported");
                    }
                }
            },
        );
    }

    fn on_import_plans(&self) {
        let Some(root_widget) = self.widget.root().and_then(|r| r.dynamic_cast_ref::<gtk4::Window>().cloned()) else { return };
        let store = Rc::clone(&self.store);
        let page_weak = self.self_weak.borrow().clone();
        crate::dialogs::open_dialog(
            &root_widget,
            "Import Training Plans",
            Some(("*.json", "JSON files")),
            move |path| {
                if let Some(path) = path {
                    match import_plans_json(&path) {
                        Ok(imported) => {
                            if !imported.is_empty() {
                                let existing_ids: Vec<String> =
                                    store.load_plans().iter().map(|p| p.id.clone()).collect();
                                let new_plans: Vec<TrainingPlan> = imported
                                    .into_iter()
                                    .filter(|p| !existing_ids.contains(&p.id))
                                    .collect();
                                if !new_plans.is_empty() {
                                    let count = new_plans.len();
                                    let mut plans = store.load_plans();
                                    plans.extend(new_plans);
                                    store.save_plans(&plans);
                                    crate::window::show_app_toast(&format!(
                                        "Imported {} plans",
                                        count
                                    ));
                                } else {
                                    crate::window::show_app_toast("Nothing new to import");
                                }
                            } else {
                                crate::window::show_app_toast("Imported file contains no plans");
                            }
                        }
                        Err(e) => {
                            crate::window::show_app_toast(&format!("Import failed: {}", e));
                        }
                    }
                }
                if let Some(p) = page_weak.upgrade() {
                    p.refresh_plans();
                }
            },
        );
    }

    fn add_exercise(&self, exercise: Option<Exercise>) {
        let ex = exercise.unwrap_or(Exercise {
            name: String::new(),
            duration_seconds: Some(30),
            rest_seconds: 30,
            ..Default::default()
        });
        self.editor_exercises.borrow_mut().push(ex.clone());
        self.append_exercise_row(ex);
    }

    // ---------- Runner ----------

    pub(crate) fn start_training(&self, plan: &TrainingPlan) {
        *self.running_plan.borrow_mut() = Some(plan.clone());
        self.current_exercise_idx.set(0);
        self.current_round.set(1);
        self.current_set.set(1);
        self.total_rounds.set(plan.total_rounds.max(1));
        self.rest_between_rounds
            .set(plan.rest_between_rounds_seconds.max(0));
        self.phase.set(Phase::Exercise);

        *self.running_session.borrow_mut() = Some(TrainingSession {
            plan_id: plan.id.clone(),
            plan_name: plan.name.clone(),
            total_planned_seconds: plan.total_planned_seconds(),
            total_rounds: plan.total_rounds.max(1),
            rest_between_rounds_seconds: plan.rest_between_rounds_seconds.max(0),
            ..Default::default()
        });

        self.update_runner_plan_label();
        self.runner_page.set_title(&plan.name);
        self.runner_ring.set_color(COLOR_NEUTRAL);
        self.runner_ring.set_progress(1.0);
        self.runner_ring.set_dimmed(false);
        self.runner_workout_bar.set_fraction(0.0);
        self.runner_add30_btn.set_sensitive(false);
        self.nav.push(&self.runner_page);
        self.refresh_runner_focus();
        self.runner_focus.borrow_mut().focus_first();
        self.start_current_exercise();
        // The runner lives inside the plans tab: fonts must be re-applied
        // now that it becomes visible (sized labels for countdown etc.).
        self.refresh_fonts();
    }

    fn update_runner_plan_label(&self) {
        let plan_name = self
            .running_plan
            .borrow()
            .as_ref()
            .map(|p| p.name.clone())
            .unwrap_or_default();
        if self.total_rounds.get() > 1 {
            self.runner_plan_label.set_label(&format!(
                "{} -- Round {}/{}",
                plan_name,
                self.current_round.get(),
                self.total_rounds.get()
            ));
        } else {
            self.runner_plan_label.set_label(&plan_name);
        }
        self.update_runner_progress_label();
    }

    /// "Round 2/3 · Exercise 5/12" summary label.
    fn update_runner_progress_label(&self) {
        let plan = self.running_plan.borrow();
        let Some(plan) = plan.as_ref() else { return };
        let n = plan.exercises.len();
        let mut parts = Vec::new();
        if self.total_rounds.get() > 1 {
            parts.push(format!("Round {}/{}", self.current_round.get(), self.total_rounds.get()));
        }
        if n > 0 {
            parts.push(format!(
                "Exercise {}/{}",
                self.current_exercise_idx.get() + 1,
                n
            ));
        }
        self.runner_progress_label.set_label(&parts.join(" · "));
        self.update_workout_bar_static();
    }

    /// Overall workout progress fraction from the exercise index alone.
    fn update_workout_bar_static(&self) {
        let units_total = self.workout_units_total() as f64;
        if units_total <= 0.0 {
            self.runner_workout_bar.set_fraction(0.0);
            return;
        }
        let plan = self.running_plan.borrow();
        let Some(plan) = plan.as_ref() else { return };
        let n = plan.exercises.len();
        if n == 0 {
            return;
        }
        let done = (self.current_round.get() as usize - 1) * n + self.current_exercise_idx.get();
        self.runner_workout_bar.set_fraction(done as f64 / units_total);
    }

    fn workout_units_total(&self) -> usize {
        let plan = self.running_plan.borrow();
        let Some(plan) = plan.as_ref() else { return 0 };
        plan.exercises.len().saturating_mul(self.total_rounds.get() as usize)
    }

    /// Updates countdown label, phase ring and interpolated workout bar on each tick.
    fn on_runner_tick(&self, remaining: f64, total: i64) {
        let mins = (remaining.max(0.0) as i64) / 60;
        let secs = (remaining.max(0.0) as i64) % 60;
        self.runner_countdown.set_label(&format!("{:02}:{:02}", mins, secs));
        if total > 0 {
            self.runner_ring.set_progress(remaining / total as f64);
        }
        self.refresh_workout_bar(remaining);
        self.sync_add30();
    }

    fn refresh_workout_bar(&self, remaining: f64) {
        let plan = self.running_plan.borrow();
        let Some(plan) = plan.as_ref() else { return };
        let n = plan.exercises.len();
        let units_total = self.workout_units_total() as f64;
        if n == 0 || units_total <= 0.0 {
            return;
        }
        let idx = self.current_exercise_idx.get();
        let ex = &plan.exercises[idx];
        let (elapsed_in_unit, unit_total): (f64, f64) = match self.phase.get() {
            Phase::Exercise if ex.is_timed() => {
                let d = ex.duration_seconds.unwrap_or(0) as f64;
                (d - remaining, d + ex.rest_seconds as f64)
            }
            Phase::Rest => {
                let d = ex.duration_seconds.unwrap_or(0) as f64;
                let r = ex.rest_seconds as f64;
                (d + (r - remaining), d + r)
            }
            _ => (0.0, 0.0),
        };
        let mut done = ((self.current_round.get() as usize - 1) * n + idx) as f64;
        if self.phase.get() == Phase::Rest {
            done += 1.0;
        }
        if unit_total > 0.0 {
            done += (elapsed_in_unit / unit_total).clamp(0.0, 1.0);
        }
        self.runner_workout_bar
            .set_fraction((done / units_total).clamp(0.0, 1.0));
    }

    /// Sets phase text, its color class and the ring color in one go.
    fn set_phase_display(&self, text: &str, css_class: &str, color: (f64, f64, f64)) {
        self.runner_phase_label.set_label(text);
        self.runner_phase_label
            .set_css_classes(&["heading", css_class]);
        self.runner_ring.set_color(color);
    }

    fn start_current_exercise(&self) {
        let Some(plan) = self.running_plan.borrow().clone() else { return };
        if self.current_exercise_idx.get() >= plan.exercises.len() {
            self.finish_training();
            return;
        }

        let ex = &plan.exercises[self.current_exercise_idx.get()];
        self.phase.set(Phase::Exercise);
        self.refresh_runner_hints();

        let mut exercise_label = ex.name.clone();
        if let Some(w) = ex.weight_kg {
            if w > 0.0 {
                exercise_label += &format!("  ({}kg)", fmt_float(w));
            }
        }
        if ex.sets > 1 {
            exercise_label += &format!("  (Set {}/{})", self.current_set.get(), ex.sets);
        }
        self.runner_exercise_label.set_label(&exercise_label);

        let next_label = self.compute_next_label();
        self.runner_next_label.set_label(&next_label);

        let now = Local::now();
        self.exercise_start_time.set(now.timestamp());
        *self.exercise_start_dt.borrow_mut() = Some(now);

        if ex.is_timed() {
            self.reps_revealer.set_reveal_child(false);
            self.set_phase_display("GO!", "phase-go", COLOR_GO);
            let settings = load_settings();
            self.sound
                .play_sound(settings.get_sound("round_start_sound").as_deref());
            self.timer.start(ex.duration_seconds.unwrap());
            self.sync_add30();
        } else {
            self.reps_revealer.set_reveal_child(true);
            self.set_phase_display("Reps", "phase-reps", COLOR_NEUTRAL);
            self.runner_ring.set_progress(1.0);
            self.reps_spin.set_value(ex.reps.unwrap_or(0) as f64);
            self.runner_countdown
                .set_label(&format!("{}", ex.reps.unwrap_or(0)));
        }
    }

    fn compute_next_label(&self) -> String {
        let Some(plan) = self.running_plan.borrow().clone() else {
            return String::new();
        };
        let ex = &plan.exercises[self.current_exercise_idx.get()];
        if ex.sets > 1 && self.current_set.get() < ex.sets {
            return format!("Next: Set {}/{}", self.current_set.get() + 1, ex.sets);
        }
        let next_idx = self.current_exercise_idx.get() + 1;
        if next_idx < plan.exercises.len() {
            return format!("Next: {}", plan.exercises[next_idx].name);
        }
        if self.current_round.get() < self.total_rounds.get() {
            return format!(
                "Next: Round {}: {}",
                self.current_round.get() + 1,
                plan.exercises[0].name
            );
        }
        "Last exercise!".to_string()
    }

    fn start_rest(&self) {
        let Some(plan) = self.running_plan.borrow().clone() else { return };
        let ex = &plan.exercises[self.current_exercise_idx.get()];
        if ex.rest_seconds > 0 {
            self.phase.set(Phase::Rest);
            self.refresh_runner_hints();
            self.set_phase_display("REST", "phase-rest", COLOR_REST);
            self.update_workout_bar_static();
            let settings = load_settings();
            self.sound
                .play_sound(settings.get_sound("round_end_sound").as_deref());
            self.timer.start(ex.rest_seconds);
            self.sync_add30();
        } else {
            self.advance_exercise();
        }
    }

    fn advance_exercise(&self) {
        let Some(plan) = self.running_plan.borrow().clone() else { return };
        let ex = &plan.exercises[self.current_exercise_idx.get()];
        if ex.sets > 1 && self.current_set.get() < ex.sets {
            self.current_set.set(self.current_set.get() + 1);
            self.start_current_exercise();
            return;
        }

        self.current_set.set(1);
        self.current_exercise_idx.set(self.current_exercise_idx.get() + 1);
        if self.current_exercise_idx.get() >= plan.exercises.len() {
            if self.current_round.get() < self.total_rounds.get() {
                if self.rest_between_rounds.get() > 0 {
                    self.start_round_break();
                } else {
                    self.current_round.set(self.current_round.get() + 1);
                    self.current_exercise_idx.set(0);
                    self.update_runner_plan_label();
                    let settings = load_settings();
                    self.sound
                        .play_sound(settings.get_sound("round_start_sound").as_deref());
                    self.start_current_exercise();
                }
            } else {
                self.finish_training();
            }
        } else {
            self.start_current_exercise();
        }
    }

    fn start_round_break(&self) {
        self.phase.set(Phase::RoundBreak);
        self.refresh_runner_hints();
        self.current_round.set(self.current_round.get() + 1);
        self.current_exercise_idx.set(0);
        self.current_set.set(1);
        self.update_runner_plan_label();
        self.runner_exercise_label.set_label("Round Break");
        self.set_phase_display(
            &format!("Next: Round {}", self.current_round.get()),
            "phase-break",
            COLOR_BREAK,
        );
        self.update_workout_bar_static();
        self.reps_revealer.set_reveal_child(false);

        let Some(plan) = self.running_plan.borrow().clone() else { return };
        self.runner_next_label
            .set_label(&format!("Next: {}", plan.exercises[0].name));
        let settings = load_settings();
        self.sound
            .play_sound(settings.get_sound("round_end_sound").as_deref());
        self.timer.start(self.rest_between_rounds.get());
        self.sync_add30();
    }

    fn finish_training(&self) {
        self.timer.stop();
        let session_opt = self.running_session.borrow_mut().take();
        if let Some(mut s) = session_opt {
            s.finished_at = Some(Local::now());
            s.total_actual_seconds = s.compute_total_actual_seconds();
            self.store.save_session(&s.clone());
            let settings = load_settings();
            self.sound
                .play_sound(settings.get_sound("training_complete_sound").as_deref());
            if let Some(root) = self
                .widget
                .root()
                .and_then(|r| r.dynamic_cast_ref::<gtk4::Window>().cloned())
            {
                crate::dialogs::send_notification(
                    &root,
                    "Training Complete",
                    &format!("\"{}\" finished", s.plan_name),
                );
            }
            self.populate_summary(&s);
            self.summary_page.set_title("Complete");
            self.nav.pop_to_page(&self.runner_page);
            self.nav.push(&self.summary_page);
            *self.running_plan.borrow_mut() = None;
        } else {
            self.show_list();
        }
    }

    fn on_timer_finished(&self) {
        if self.running_plan.borrow().is_none() {
            return;
        }
        if self.phase.get() == Phase::RoundBreak {
            let settings = load_settings();
            self.sound
                .play_sound(settings.get_sound("round_start_sound").as_deref());
            self.start_current_exercise();
            return;
        }

        let settings = load_settings();
        self.sound
            .play_sound(settings.get_sound("exercise_complete_sound").as_deref());

        if self.phase.get() == Phase::Exercise {
            self.log_exercise(true);
        } else if self.phase.get() == Phase::Rest {
            let plan = self.running_plan.borrow().clone().unwrap();
            let ex = &plan.exercises[self.current_exercise_idx.get()];
            let mut session = self.running_session.borrow_mut();
            if let Some(ref mut s) = session.as_mut() {
                if let Some(last) = s.exercises.last_mut() {
                    last.actual_rest_seconds = Some(ex.rest_seconds);
                }
            }
            drop(session);
            self.advance_exercise();
        }
    }

    fn log_exercise(&self, completed: bool) {
        let Some(plan) = self.running_plan.borrow().clone() else { return };
        let ex = &plan.exercises[self.current_exercise_idx.get()];
        let now = Local::now();
        let started = self.exercise_start_dt.borrow().clone().unwrap_or(now);
        let elapsed = now.timestamp() - self.exercise_start_time.get();
        let ex_log = ExerciseLog {
            exercise_name: ex.name.clone(),
            planned_duration_seconds: ex.duration_seconds,
            actual_duration_seconds: Some(elapsed),
            planned_reps: ex.reps,
            actual_reps: if ex.is_timed() {
                None
            } else {
                Some(self.reps_spin.value() as i64)
            },
            planned_weight_kg: ex.weight_kg,
            actual_weight_kg: ex.weight_kg,
            rest_seconds: ex.rest_seconds,
            actual_rest_seconds: None,
            completed,
            round_number: self.current_round.get(),
            started_at: Some(started),
            finished_at: Some(now),
            set_number: self.current_set.get(),
        };
        self.running_session
            .borrow_mut()
            .as_mut()
            .unwrap()
            .exercises
            .push(ex_log);
        self.reps_revealer.set_reveal_child(false);
        self.start_rest();
    }

    /// The pause/resume button label ("Pause" or "Resume") — used by the
    /// ui tests to observe the runner's paused state.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn runner_pause_label(&self) -> String {
        self.runner_pause_btn
            .label()
            .map(|s| s.to_string())
            .unwrap_or_default()
    }

    /// The plan ids in the order the list shows them (favorites first) —
    /// used by the ui tests to verify the favorites-first sorting.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn plans_order(&self) -> Vec<String> {
        self.plans.borrow().iter().map(|p| p.id.clone()).collect()
    }

    /// Number of rows in the plans list — used by the ui tests to
    /// verify that a refresh does not leave stale duplicate rows.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn plans_row_count(&self) -> usize {
        let mut n = 0;
        while self.plan_list_box.row_at_index(n as i32).is_some() {
            n += 1;
        }
        n
    }

    /// The gamepad focus list of the runner, in visual order — used by
    /// the ui tests to verify the +30s button is reachable.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn runner_focus_widgets(&self) -> Vec<gtk4::Widget> {
        self.runner_focus.borrow().widgets()
    }

    /// Keep the +30s button in sync with the running timer. The gamepad
    /// focus tracker drops insensitive widgets, so whenever the button
    /// flips the focus list must be rebuilt — otherwise +30s stays
    /// unreachable for the rest of the workout.
    fn sync_add30(&self) {
        let on = self.timer.is_running.get();
        if self.runner_add30_btn.is_sensitive() != on {
            self.runner_add30_btn.set_sensitive(on);
            self.refresh_runner_focus();
        }
    }

    fn on_runner_pause(&self) {
        if self.timer.is_running.get() {
            self.timer.pause();
            self.runner_pause_btn.set_label("Resume");
            self.runner_ring.set_dimmed(true);
            self.runner_phase_label.add_css_class("dim-label");
        } else {
            self.timer.resume();
            self.runner_pause_btn.set_label("Pause");
            self.runner_ring.set_dimmed(false);
            self.runner_phase_label.remove_css_class("dim-label");
        }
        self.sync_add30();
    }

    fn on_runner_add30(&self) {
        if !self.timer.is_running.get() {
            return;
        }
        self.timer.add_seconds(30);
        // Refresh ring + workout bar immediately instead of waiting for the tick
        let remaining = self.timer.remaining_seconds.get();
        let total = self.timer.total_seconds.get();
        self.on_runner_tick(remaining, total);
    }

    fn on_runner_skip(&self) {
        self.timer.stop();
        self.sync_add30();
        if self.running_plan.borrow().is_none() {
            return;
        }
        if self.phase.get() == Phase::RoundBreak {
            self.start_current_exercise();
            return;
        }
        if self.phase.get() == Phase::Exercise {
            self.log_exercise(false);
        } else if self.phase.get() == Phase::Rest {
            let plan = self.running_plan.borrow().clone().unwrap();
            let ex = &plan.exercises[self.current_exercise_idx.get()];
            let remaining = self.timer.remaining_seconds.get();
            let elapsed = if remaining > 0.0 {
                ex.rest_seconds - remaining as i64
            } else {
                0
            };
            let mut session = self.running_session.borrow_mut();
            if let Some(ref mut s) = session.as_mut() {
                if let Some(last) = s.exercises.last_mut() {
                    last.actual_rest_seconds = Some(elapsed.max(0));
                }
            }
            drop(session);
            self.advance_exercise();
        }
    }

    fn populate_summary(&self, session: &TrainingSession) {
        self.summary_plan_name.set_label(&session.plan_name);

        let date_str = session.started_at.format("%Y-%m-%d %H:%M").to_string();
        let finished_str = session
            .finished_at
            .map(|d| d.format("%H:%M").to_string())
            .unwrap_or_else(|| "?".to_string());
        let planned_min = session.total_planned_seconds / 60;
        let planned_sec = session.total_planned_seconds % 60;
        let actual_min = session.total_actual_seconds / 60;
        let actual_sec = session.total_actual_seconds % 60;
        self.summary_info.set_label(&format!(
            "{} - {} | Actual: {:02}:{:02} / Planned: {:02}:{:02}",
            date_str, finished_str, actual_min, actual_sec, planned_min, planned_sec
        ));

        clear_box(&self.summary_exercises_box);

        let group = PreferencesGroup::new();
        let show_rounds = session.total_rounds > 1;
        let mut group_title = String::from("Exercises");
        if show_rounds {
            let rbr_str = if session.rest_between_rounds_seconds > 0 {
                format!(", {}s rest between", session.rest_between_rounds_seconds)
            } else {
                String::new()
            };
            group_title = format!("Exercises ({} rounds{})", session.total_rounds, rbr_str);
        }
        group.set_title(&group_title);

        for ex_log in &session.exercises {
            let mut title = ex_log.exercise_name.clone();
            if show_rounds && ex_log.round_number > 1 {
                title = format!("R{}: {}", ex_log.round_number, ex_log.exercise_name);
            }
            if ex_log.set_number > 1 {
                title += &format!(" (Set {})", ex_log.set_number);
            }
            let row = ActionRow::builder().title(&title).build();

            let mut details = Vec::new();
            if let Some(planned) = ex_log.planned_duration_seconds {
                details.push(format!(
                    "Time: {} / {}",
                    fmt_dur(ex_log.actual_duration_seconds),
                    fmt_dur(Some(planned))
                ));
            } else if let Some(planned) = ex_log.planned_reps {
                let actual = ex_log
                    .actual_reps
                    .map(|x| x.to_string())
                    .unwrap_or_else(|| "?".to_string());
                details.push(format!("Reps: {} / {}", actual, planned));
            }
            if let Some(w) = ex_log.planned_weight_kg {
                if w > 0.0 {
                    details.push(format!("Weight: {}kg", fmt_float(w)));
                }
            }
            details.push(format!(
                "Rest: {} / {}",
                fmt_dur(ex_log.actual_rest_seconds),
                fmt_dur(Some(ex_log.rest_seconds))
            ));
            details.push(fmt_exercise_time(ex_log));

            row.set_subtitle(&details.join(" | "));

            if !ex_log.completed {
                row.add_css_class("error");
            }

            let status_label = Label::new(if ex_log.completed {
                Some("Done")
            } else {
                Some("Skipped")
            });
            row.add_suffix(&status_label);

            group.add(&row);
        }

        self.summary_exercises_box.append(&group);
    }

    /// The runner hint bar is phase-dependent ([A] Done vs. rest/skip);
    /// refresh it whenever the phase changes.
    fn refresh_runner_hints(&self) {
        if let Some(window) = crate::window::current_window() {
            window.update_hints_visibility();
        }
    }

    fn confirm_stop_workout(&self) {
        let dialog = Rc::new(
            AlertDialog::builder()
                .heading("End Training?")
                .body("You can view a summary of your completed exercises.")
                .build(),
        );
        dialog.add_response("cancel", "Cancel");
        dialog.add_response("end", "End Training");
        dialog.set_response_appearance("end", libadwaita::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));

        let page_rc_holder = self.self_weak_holder();
        dialog.connect_response(None, move |_dlg, response| {
            if response == "end" {
                if let Some(p) = page_rc_holder.upgrade() {
                    p.finalize_training_stop();
                }
            }
        });
        self.track_dialog(&dialog, "end");
        self.present_dialog(&dialog);
    }

    /// Route controller events to the dialog and unregister it once closed.
    fn track_dialog(&self, dialog: &Rc<AlertDialog>, confirm_id: &str) {
        self.register_dialog(Some((dialog.clone(), confirm_id.to_string())));
        let cleanup: Rc<dyn Fn()> = Rc::new({
            let registrar = self.dialog_registrar.borrow().clone();
            move || {
                if let Some(cb) = registrar.clone() {
                    cb(None);
                }
            }
        });
        let cleanup_response = Rc::clone(&cleanup);
        dialog.connect_response(None, move |_dlg, _response| {
            cleanup_response();
        });
        let cleanup_closed = Rc::clone(&cleanup);
        dialog.connect_closed(move |_dlg| {
            cleanup_closed();
        });
    }

    fn present_dialog(&self, dialog: &Rc<AlertDialog>) {
        if let Some(root_widget) = self.widget.root() {
            if let Some(window) = root_widget.dynamic_cast_ref::<gtk4::Window>() {
                dialog.present(Some(window));
            }
        }
    }

    fn finalize_training_stop(&self) {
        self.timer.stop();
        let has_running = {
            let session = self.running_session.borrow();
            session
                .as_ref()
                .map(|s| s.finished_at.is_none())
                .unwrap_or(false)
        };
        if has_running {
            let session_opt = self.running_session.borrow_mut().take();
            if let Some(mut s) = session_opt {
                s.finished_at = Some(Local::now());
                s.total_actual_seconds = s.compute_total_actual_seconds();
                self.store.save_session(&s.clone());
                let settings = load_settings();
                self.sound
                    .play_sound(settings.get_sound("training_complete_sound").as_deref());
                self.populate_summary(&s);
                self.summary_page.set_title("Complete");
                self.nav.pop_to_page(&self.runner_page);
                self.nav.push(&self.summary_page);
                *self.running_plan.borrow_mut() = None;
                self.refresh_fonts();
            }
        } else {
            self.show_list();
        }
    }

    // ---------- Controller API ----------

    pub fn get_controller_context(&self) -> String {
        match self.visible_page() {
            "runner" => "runner_exercise".to_string(),
            "list" => "list".to_string(),
            "editor" => "editor".to_string(),
            "summary" => "runner_summary".to_string(),
            _ => String::new(),
        }
    }

    pub fn get_controller_sub_key(&self) -> String {
        if self.visible_page() == "runner" {
            self.phase.get().as_str().to_string()
        } else {
            String::new()
        }
    }

    pub fn controller_a(&self) {
        match self.visible_page() {
            "runner" => {
                // A activates whatever runner button has focus (Pause,
                // +30s, Skip, Done). The widget is cloned out of the
                // borrow first: clicking rebuilds focus lists.
                let focused = self.runner_focus.borrow().current_widget();
                if let Some(widget) = focused {
                    if let Some(btn) = widget.downcast_ref::<Button>() {
                        if btn.is_sensitive() {
                            btn.emit_clicked();
                            return;
                        }
                    }
                }
                if self.phase.get() == Phase::Exercise {
                    if let Some(plan) = self.running_plan.borrow().clone() {
                        let ex = &plan.exercises[self.current_exercise_idx.get()];
                        if !ex.is_timed() {
                            self.log_exercise(true);
                        }
                    }
                } else {
                    self.on_runner_skip();
                }
            }
            "summary" => self.show_list(),
            "list" => {
                let widget_opt = self.list_focus.borrow().current_widget();
                let Some(widget) = widget_opt else { return };
                if widget.as_ptr() == self.new_plan_btn.upcast_ref::<gtk4::Widget>().as_ptr() {
                    self.new_plan_btn.emit_clicked();
                } else if widget.as_ptr() == self.export_btn.upcast_ref::<gtk4::Widget>().as_ptr() {
                    self.export_btn.emit_clicked();
                } else if widget.as_ptr() == self.import_btn.upcast_ref::<gtk4::Widget>().as_ptr() {
                    self.import_btn.emit_clicked();
                } else if let Some(row) = widget.downcast_ref::<ActionRow>() {
                    let idx = row.index();
                    let plans = self.plans.borrow();
                    if idx >= 0 && (idx as usize) < plans.len() {
                        let plan = plans[idx as usize].clone();
                        drop(plans);
                        self.show_editor(Some(&plan));
                    }
                }
            }
            "editor" => {
                // Clone the widget out before acting: activating rows or
                // buttons emits notify signals whose handlers rebuild the
                // focus list, and holding the borrow here would panic.
                let widget = self.editor_focus.borrow().current_widget();
                if let Some(widget) = widget {
                    if let Some(expander) = widget.downcast_ref::<ExpanderRow>() {
                        expander.set_expanded(!expander.is_expanded());
                    } else if let Some(btn) = widget.downcast_ref::<Button>() {
                        btn.emit_clicked();
                    } else if let Some(row) = widget.downcast_ref::<ButtonRow>() {
                        row.activate();
                    } else {
                        widget.grab_focus();
                    }
                }
            }
            _ => {}
        }
    }

    pub fn controller_b(&self) {
        match self.visible_page() {
            "runner" => self.confirm_stop_workout(),
            "summary" | "editor" => self.show_list(),
            _ => {}
        }
    }

    pub fn controller_start(&self) {
        match self.visible_page() {
            "runner" => self.on_runner_pause(),
            "editor" => self.on_run_plan(),
            _ => {}
        }
    }

    pub fn controller_y(&self) {
        match self.visible_page() {
            "runner" => {
                if self.phase.get() == Phase::Exercise || self.phase.get() == Phase::Rest {
                    self.on_runner_skip();
                }
            }
            "list" => {
                if let Some(plan) = self.focused_list_plan() {
                    self.duplicate_plan(&plan);
                }
            }
            _ => {}
        }
    }

    /// R2 on the plans list: progress the focused plan (gamepad shortcut
    /// for the per-row "progress" button).
    pub fn controller_r2(&self) {
        if self.visible_page() == "list" {
            if let Some(plan) = self.focused_list_plan() {
                self.progress_plan(&plan);
            }
        }
    }

    pub fn controller_back(&self) {
        self.controller_b();
    }

    pub fn controller_dpad_up(&self) {
        match self.visible_page() {
            "editor" => self.editor_focus.borrow_mut().cycle(-1),
            "list" => self.list_focus.borrow_mut().cycle(-1),
            "runner" => self.runner_focus.borrow_mut().cycle(-1),
            _ => {}
        }
    }

    /// Focusable widgets of the workout runner (in visual order).
    fn refresh_runner_focus(&self) {
        let mut widgets: Vec<gtk4::Widget> = Vec::new();
        widgets.push(self.runner_pause_btn.clone().upcast());
        widgets.push(self.runner_add30_btn.clone().upcast());
        widgets.push(self.runner_skip_btn.clone().upcast());
        widgets.push(self.reps_spin.clone().upcast());
        widgets.push(self.reps_done_btn.clone().upcast());
        self.runner_focus.borrow_mut().set_widgets(widgets);
    }

    pub fn controller_dpad_down(&self) {
        match self.visible_page() {
            "editor" => self.editor_focus.borrow_mut().cycle(1),
            "list" => self.list_focus.borrow_mut().cycle(1),
            "runner" => self.runner_focus.borrow_mut().cycle(1),
            _ => {}
        }
    }

    pub fn controller_dpad_left(&self) {
        if self.visible_page() == "runner" {
            if let Some(widget) = self.runner_focus.borrow().current_widget() {
                if widget.downcast_ref::<SpinRow>().is_some() {
                    let val = (self.reps_spin.value() as i64 - 1).max(0);
                    self.reps_spin.set_value(val as f64);
                }
            }
            return;
        }
        if self.visible_page() != "editor" {
            return;
        }
        let widget = self.editor_focus.borrow().current_widget();
        if let Some(widget) = widget {
            adjust_focused_widget(&widget, -1);
        }
    }

    pub fn controller_dpad_right(&self) {
        if self.visible_page() == "runner" {
            if let Some(widget) = self.runner_focus.borrow().current_widget() {
                if widget.downcast_ref::<SpinRow>().is_some() {
                    let val = self.reps_spin.value() as i64 + 1;
                    self.reps_spin.set_value(val as f64);
                }
            }
            return;
        }
        if self.visible_page() != "editor" {
            return;
        }
        let widget = self.editor_focus.borrow().current_widget();
        if let Some(widget) = widget {
            adjust_focused_widget(&widget, 1);
        }
    }

    pub fn controller_x(&self) {
        if self.visible_page() == "editor" {
            // Same as controller_a: drop the borrow before mutating the row,
            // the notify handler rebuilds the focus list synchronously.
            let widget = self.editor_focus.borrow().current_widget();
            if let Some(widget) = widget {
                if let Some(expander) = widget.downcast_ref::<ExpanderRow>() {
                    expander.set_expanded(!expander.is_expanded());
                } else {
                    let parent = widget.ancestor(libadwaita::ExpanderRow::static_type());
                    if let Some(parent) = parent {
                        if let Some(expander) =
                            parent.dynamic_cast_ref::<libadwaita::ExpanderRow>()
                        {
                            expander.set_expanded(false);
                        }
                    }
                }
            }
        } else if self.visible_page() == "list" {
            let widget = self.list_focus.borrow().current_widget();
            if let Some(widget) = widget {
                if let Some(row) = widget.downcast_ref::<ActionRow>() {
                    let idx = row.index();
                    let plan_id = {
                        let plans = self.plans.borrow();
                        if idx >= 0 && (idx as usize) < plans.len() {
                            plans[idx as usize].id.clone()
                        } else {
                            return;
                        }
                    };
                    let star_btn = self.star_buttons.borrow().get(&plan_id).cloned();
                    if let Some(btn) = star_btn.and_then(|b| b.downcast_ref::<Button>().cloned()) {
                        btn.emit_clicked();
                    }
                }
            }
        }
    }

    pub fn update_fonts(&self, width: i32, height: i32) {
        apply_scaling(
            &[
                (LabelType::Exercise, &self.runner_exercise_label),
                (LabelType::Info, &self.runner_plan_label),
                (LabelType::Info, &self.runner_phase_label),
                (LabelType::Info, &self.runner_next_label),
            ],
            width,
            height,
        );

        // The ring adapts to the window height and the countdown follows it,
        // so the runner always fits on short screens.
        if height > 0 {
            let size = ((height as f64 * 0.30) as i32).clamp(160, 300);
            self.runner_ring.set_size(size);
            crate::ui_scaling::set_label_size(&self.runner_countdown, (size as f64 * 0.20) as i32);
        }
    }
}



fn display_title(exercise: &Exercise) -> String {
    let mut parts = Vec::new();
    if let Some(w) = exercise.weight_kg {
        if w > 0.0 {
            parts.push(format!("{}kg", fmt_float(w)));
        }
    }
    if let Some(d) = exercise.duration_seconds {
        if d > 0 {
            parts.push(format!("{}s", d));
        }
    }
    if let Some(r) = exercise.reps {
        if r > 0 {
            parts.push(format!("{} reps", r));
        }
    }
    if exercise.sets > 1 {
        parts.push(format!("{} sets", exercise.sets));
    }
    let rest = if exercise.rest_seconds > 0 {
        format!("[{}s rest]", exercise.rest_seconds)
    } else {
        String::new()
    };
    let name = if exercise.name.trim().is_empty() {
        "New Exercise"
    } else {
        exercise.name.as_str()
    };
    if !parts.is_empty() {
        format!("{} ({}) {}", name, parts.join(", "), rest)
            .trim()
            .to_string()
    } else {
        format!("{} {}", name, rest).trim().to_string()
    }
}

fn clear_box(box_widget: &GtkBox) {
    let mut child = box_widget.first_child();
    while let Some(c) = child {
        child = c.next_sibling();
        box_widget.remove(&c);
    }
}

fn fmt_dur(seconds: Option<i64>) -> String {
    match seconds {
        None => "--:--".to_string(),
        Some(s) => {
            let s = s.max(0);
            format!("{:02}:{:02}", s / 60, s % 60)
        }
    }
}

fn fmt_float(v: f64) -> String {
    format!("{}", v)
}

fn fmt_exercise_time(ex_log: &ExerciseLog) -> String {
    if let (Some(start), Some(finish)) = (ex_log.started_at, ex_log.finished_at) {
        format!(
            "{} - {} ({})",
            start.format("%H:%M"),
            finish.format("%H:%M"),
            fmt_dur(ex_log.actual_duration_seconds)
        )
    } else {
        format!("Duration: {}", fmt_dur(ex_log.actual_duration_seconds))
    }
}
