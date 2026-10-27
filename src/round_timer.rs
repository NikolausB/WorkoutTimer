use crate::controller_focus::ControllerFocusTracker;
use crate::models::{RoundConfig, TimerPresetEntry};
use crate::progress_ring::{ProgressRing, COLOR_DONE, COLOR_GO, COLOR_NEUTRAL, COLOR_REST};
use crate::settings::{load_settings, save_settings};
use crate::sound::SoundPlayer;
use crate::timer::TimerCore;
use crate::ui_scaling::{apply_scaling, LabelType};
use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Button, Label, ListBox, Orientation, ScrolledWindow};
use libadwaita::prelude::*;
use libadwaita::{ActionRow, Bin, Dialog, EntryRow, PreferencesGroup, SpinRow, SwitchRow};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Which phase the round timer is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Idle,
    Prepare,
    Warmup,
    Round,
    Rest,
    Done,
}

/// The built-in presets that ship with the app.
fn builtin_presets() -> Vec<TimerPresetEntry> {
    let simple =
        |name: &str, rounds: i64, round_seconds: i64, pause_seconds: i64| TimerPresetEntry {
            name: name.to_string(),
            config: RoundConfig {
                rounds,
                round_seconds,
                pause_seconds,
                prep_seconds: 0,
                warmup: false,
                per_round: None,
                title: String::new(),
            },
        };
    vec![
        simple("Boxing 3 × 3min", 3, 180, 60),
        simple("Tabata 8 × 20/10", 8, 20, 10),
        simple("HIIT 10 × 30/30", 10, 30, 30),
        simple("Endurance 5 × 5min", 5, 300, 60),
        simple("Pomodoro 25/5", 4, 1500, 300),
        TimerPresetEntry {
            name: "Pyramid 20/40/60".to_string(),
            config: RoundConfig {
                rounds: 3,
                round_seconds: 20,
                pause_seconds: 10,
                prep_seconds: 0,
                warmup: false,
                per_round: Some(
                    [(20, 10), (40, 10), (60, 0)]
                        .iter()
                        .map(|(work, rest)| crate::models::RoundSpec {
                            work_seconds: *work,
                            rest_seconds: *rest,
                        })
                        .collect(),
                ),
                title: String::new(),
            },
        },
    ]
}

pub struct RoundTimerPage {
    widget: Bin,
    config: RefCell<RoundConfig>,
    timer: Rc<TimerCore>,
    current_round: Rc<Cell<i64>>,
    is_pause: Rc<Cell<bool>>,
    total_rounds: Rc<Cell<i64>>,
    focus: Rc<RefCell<ControllerFocusTracker>>,
    sound: Rc<SoundPlayer>,

    phase: Cell<Phase>,
    completed_work_seconds: RefCell<i64>,
    rounds_spin: SpinRow,
    round_time_spin: SpinRow,
    pause_time_spin: SpinRow,
    prep_spin: SpinRow,
    warmup_switch: SwitchRow,
    title_entry: libadwaita::EntryRow,
    custom_switch: SwitchRow,
    custom_rounds_box: GtkBox,
    summary_label: Label,
    per_round: RefCell<Vec<crate::models::RoundSpec>>,
    ring: ProgressRing,
    round_label: Label,
    phase_label: Label,
    countdown_label: Label,
    start_btn: Button,
    pause_btn: Button,
    reset_btn: Button,
    skip_btn: Button,
    favorite_preset_btn: Button,
    presets_btn: Button,
    presets_dialog_registrar: RefCell<Option<PresetsDialogRegistrar>>,
}

/// Gamepad controller state for the open presets dialog: the dialog plus
/// the focus tracker over its rows and buttons. The window routes dpad/A/B
/// through this while the dialog is open.
pub(crate) type PresetsDialogRegistrar =
    Rc<dyn Fn(Option<Rc<PresetsDialogController>>)>;

pub(crate) struct PresetsDialogController {
    pub(crate) dialog: Dialog,
    pub(crate) tracker: Rc<RefCell<ControllerFocusTracker>>,
}

impl PresetsDialogController {
    /// Activate the focused widget (rows apply a preset, buttons click).
    pub(crate) fn activate_focused(&self) {
        let widget = self.tracker.borrow().current_widget();
        if let Some(widget) = widget {
            if let Some(row) = widget.downcast_ref::<libadwaita::ActionRow>() {
                libadwaita::prelude::ActionRowExt::activate(row);
            } else if let Some(btn) = widget.downcast_ref::<gtk4::Button>() {
                btn.emit_clicked();
            }
        }
    }
}

impl RoundTimerPage {
    pub fn new(sound: Rc<SoundPlayer>) -> Rc<Self> {
        let widget = Bin::new();

        let main_box = GtkBox::new(Orientation::Vertical, 12);
        main_box.set_vexpand(true);
        main_box.set_margin_top(24);
        main_box.set_margin_bottom(24);
        main_box.set_margin_start(24);
        main_box.set_margin_end(24);

        // ---- Action row on top (Start / Pause / Reset / Skip) ----
        let btn_box = GtkBox::new(Orientation::Horizontal, 12);
        btn_box.set_halign(Align::Center);

        let start_btn = Button::builder()
            .label("Start")
            .css_classes(["suggested-action"])
            .build();
        btn_box.append(&start_btn);

        let pause_btn = Button::builder()
            .label("Pause")
            .sensitive(false)
            .css_classes(["flat"])
            .build();
        btn_box.append(&pause_btn);

        let reset_btn = Button::builder()
            .label("Reset")
            .sensitive(false)
            .css_classes(["flat"])
            .build();
        btn_box.append(&reset_btn);

        let skip_btn = Button::builder()
            .label("Skip")
            .sensitive(false)
            .css_classes(["destructive-action"])
            .build();
        btn_box.append(&skip_btn);

        main_box.append(&btn_box);

        // ---- Round / phase labels (compact, right under the buttons) ----
        let round_label = Label::new(Some("Round 0 / 0"));
        round_label.add_css_class("title-2");
        round_label.set_halign(Align::Center);
        main_box.append(&round_label);

        let phase_label = Label::new(Some(""));
        phase_label.set_css_classes(&["heading", "phase-reps"]);
        phase_label.set_halign(Align::Center);
        main_box.append(&phase_label);

        let summary_label = Label::new(None);
        summary_label.add_css_class("dim-label");
        summary_label.set_halign(Align::Center);
        summary_label.set_visible(false);
        main_box.append(&summary_label);

        // ---- Circular ring with countdown in the middle ----
        let ring = ProgressRing::new(260);
        let ring_overlay = gtk4::Overlay::new();
        ring_overlay.set_child(Some(ring.widget()));
        let countdown_label = Label::new(Some("00:00"));
        countdown_label.set_halign(Align::Center);
        countdown_label.set_valign(Align::Center);
        ring_overlay.add_overlay(&countdown_label);
        ring_overlay.set_vexpand(false);
        ring_overlay.set_valign(Align::Center);
        main_box.append(&ring_overlay);

        let config_group = PreferencesGroup::builder().title("Configuration").build();

        let rounds_spin = Self::add_spin_row(&config_group, "Rounds", 1.0, 50.0, 10.0, 1.0);
        let round_time_spin =
            Self::add_spin_row(&config_group, "Round duration (seconds)", 10.0, 3600.0, 180.0, 10.0);
        let pause_time_spin =
            Self::add_spin_row(&config_group, "Pause duration (seconds)", 0.0, 600.0, 60.0, 5.0);
        let prep_spin =
            Self::add_spin_row(&config_group, "Prepare countdown (seconds)", 0.0, 60.0, 0.0, 5.0);

        let warmup_switch = SwitchRow::builder()
            .title("Warm-up round")
            .subtitle("One extra round at the pause length before round 1")
            .build();
        config_group.add(&warmup_switch);

        let title_entry = libadwaita::EntryRow::builder()
            .title("Activity")
            .show_apply_button(false)
            .build();
        config_group.add(&title_entry);

        // Custom rounds (ladder) editor
        let custom_switch = SwitchRow::builder()
            .title("Custom rounds")
            .subtitle("Different work/rest time per round")
            .build();
        config_group.add(&custom_switch);
        let custom_rounds_box = GtkBox::new(Orientation::Vertical, 6);
        custom_rounds_box.set_margin_start(12);
        custom_rounds_box.set_margin_end(12);
        custom_rounds_box.set_margin_bottom(6);
        config_group.add(&custom_rounds_box);

        main_box.append(&config_group);

        // Quick presets: the favorite as a pill + a submenu with all presets.
        let preset_box = GtkBox::new(Orientation::Horizontal, 8);
        preset_box.set_halign(Align::Center);
        preset_box.set_margin_top(4);
        let favorite_preset_btn = Button::builder().css_classes(["pill"]).build();
        favorite_preset_btn.set_visible(false);
        preset_box.append(&favorite_preset_btn);
        let presets_btn = Button::builder()
            .label("Presets…")
            .css_classes(["pill"])
            .build();
        preset_box.append(&presets_btn);
        main_box.append(&preset_box);

        // Scrollable so the page never overflows on short windows.
        let scrolled = ScrolledWindow::new();
        scrolled.set_vexpand(true);
        scrolled.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
        scrolled.set_child(Some(&main_box));
        widget.set_child(Some(&scrolled));

        let timer = Rc::new(TimerCore::new());
        let current_round = Rc::new(Cell::new(0i64));
        let is_pause = Rc::new(Cell::new(false));
        let total_rounds = Rc::new(Cell::new(0i64));


        let saved_config = load_settings().timer_config;
        let initial_config = saved_config.clone().unwrap_or_default();

        let page = Rc::new(Self {
            widget,
            config: RefCell::new(initial_config.clone()),
            timer: Rc::clone(&timer),
            current_round: Rc::clone(&current_round),
            is_pause: Rc::clone(&is_pause),
            total_rounds: Rc::clone(&total_rounds),
            focus: Rc::new(RefCell::new(ControllerFocusTracker::new())),
            sound,
            phase: Cell::new(Phase::Idle),
            completed_work_seconds: RefCell::new(0),
            rounds_spin: rounds_spin.clone(),
            round_time_spin: round_time_spin.clone(),
            pause_time_spin: pause_time_spin.clone(),
            prep_spin: prep_spin.clone(),
            warmup_switch: warmup_switch.clone(),
            title_entry: title_entry.clone(),
            custom_switch: custom_switch.clone(),
            custom_rounds_box: custom_rounds_box.clone(),
            summary_label: summary_label.clone(),
            per_round: RefCell::new(Vec::new()),
            ring,
            round_label: round_label.clone(),
            phase_label: phase_label.clone(),
            countdown_label: countdown_label.clone(),
            start_btn: start_btn.clone(),
            pause_btn: pause_btn.clone(),
            reset_btn: reset_btn.clone(),
            skip_btn: skip_btn.clone(),
            favorite_preset_btn: favorite_preset_btn.clone(),
            presets_btn: presets_btn.clone(),
            presets_dialog_registrar: RefCell::new(None),
        });

        // The favorite pill applies the favorite preset.
        {
            let page_weak = Rc::downgrade(&page);
            favorite_preset_btn.connect_clicked(move |_| {
                if let Some(p) = page_weak.upgrade() {
                    let settings = load_settings();
                    if let Some(preset) = p.find_preset(&settings, &settings.favorite_preset) {
                        p.apply_preset(&preset.config);
                    }
                }
            });
        }

        // The Presets button opens the preset submenu dialog.
        {
            let page_weak = Rc::downgrade(&page);
            presets_btn.connect_clicked(move |_| {
                if let Some(p) = page_weak.upgrade() {
                    p.show_presets_dialog();
                }
            });
        }

        // Reflect the persisted favorite on the pill.
        page.refresh_favorite_pill();

        // Apply the restored configuration to the widgets.
        {
            let page_weak = Rc::downgrade(&page);
            let config = initial_config.clone();
            gtk4::glib::idle_add_local_once(move || {
                if let Some(p) = page_weak.upgrade() {
                    p.apply_config_to_widgets(&config);
                    p.custom_rounds_box.set_visible(p.custom_switch.is_active());
                }
            });
        }

        {
            let page_weak = Rc::downgrade(&page);
            page.custom_switch.connect_active_notify(move |_| {
                if let Some(p) = page_weak.upgrade() {
                    let active = p.custom_switch.is_active();
                    p.custom_rounds_box.set_visible(active);
                    p.rounds_spin.set_sensitive(!active);
                    p.round_time_spin.set_sensitive(!active);
                    if active && p.per_round.borrow().is_empty() {
                        p.per_round.borrow_mut().push(crate::models::RoundSpec {
                            work_seconds: p.round_time_spin.value() as i64,
                            rest_seconds: p.pause_time_spin.value() as i64,
                        });
                    }
                    p.rebuild_custom_rounds();
                }
            });
        }

        {
            let page_weak = Rc::downgrade(&page);
            timer.on_tick.borrow_mut().replace(Box::new(move |remaining, total| {
                if let Some(p) = page_weak.upgrade() {
                    p.on_timer_tick(remaining, total);
                }
            }));
        }

        {
            let page_weak = Rc::downgrade(&page);
            timer.on_finished.borrow_mut().replace(Box::new(move || {
                if let Some(p) = page_weak.upgrade() {
                    p.on_timer_finished();
                }
            }));
        }

        {
            let page_weak = Rc::downgrade(&page);
            start_btn.connect_clicked(move |_| {
                if let Some(p) = page_weak.upgrade() {
                    p.on_start_clicked();
                }
            });
        }
        {
            let page_weak = Rc::downgrade(&page);
            pause_btn.connect_clicked(move |_| {
                if let Some(p) = page_weak.upgrade() {
                    p.on_pause_clicked();
                }
            });
        }
        {
            let page_weak = Rc::downgrade(&page);
            reset_btn.connect_clicked(move |_| {
                if let Some(p) = page_weak.upgrade() {
                    p.on_reset_clicked();
                }
            });
        }
        {
            let page_weak = Rc::downgrade(&page);
            skip_btn.connect_clicked(move |_| {
                if let Some(p) = page_weak.upgrade() {
                    p.on_skip_clicked();
                }
            });
        }

        // Full initial focus list: every config row must be reachable with
        // the gamepad right away, not only after the timer ran once.
        page.rebuild_focus_list();

        page
    }

    pub fn widget(&self) -> &Bin {
        &self.widget
    }

    /// Window hook so controller events reach the presets dialog while it
    /// is open; `None` releases the registration when it closes.
    pub fn set_presets_dialog_registrar(
        &self,
        cb: PresetsDialogRegistrar,
    ) {
        *self.presets_dialog_registrar.borrow_mut() = Some(cb);
    }

    /// Find a preset by name (builtin or user-saved).
    fn find_preset(
        &self,
        settings: &crate::settings::AppSettings,
        name: &str,
    ) -> Option<TimerPresetEntry> {
        if name.is_empty() {
            return None;
        }
        builtin_presets()
            .into_iter()
            .chain(settings.timer_presets.iter().cloned())
            .find(|p| p.name == name)
    }

    /// Apply a preset config to the widgets and persist it.
    fn apply_preset(self: &Rc<Self>, config: &RoundConfig) {
        if !self.rounds_spin.is_sensitive() {
            return;
        }
        self.apply_config_to_widgets(config);
        self.custom_rounds_box
            .set_visible(self.custom_switch.is_active());
        self.sync_config_from_widgets();
    }

    /// Show/hide the favorite pill to match the persisted favorite.
    fn refresh_favorite_pill(&self) {
        let settings = load_settings();
        let exists = self
            .find_preset(&settings, &settings.favorite_preset)
            .is_some();
        self.favorite_preset_btn.set_visible(exists);
        if exists {
            self.favorite_preset_btn
                .set_label(&settings.favorite_preset);
        }
    }

    /// Set (or clear with an empty name) the favorite preset and persist it.
    fn set_favorite_preset(&self, name: &str) {
        let mut settings = load_settings();
        settings.favorite_preset = name.to_string();
        save_settings(&settings);
        self.refresh_favorite_pill();
    }

    /// The presets submenu: all builtin + custom presets, favorite toggle,
    /// delete for custom ones, and adding a new preset from the current config.
    fn show_presets_dialog(self: &Rc<Self>) {
        let dialog = Dialog::new();
        dialog.set_title("Timer Presets");
        dialog.set_content_width(480);
        dialog.set_content_height(520);
        dialog.set_follows_content_size(false);

        // The save dialog can only be presented once this one has fully
        // closed (Adw.Dialog close is asynchronous).
        let requested_save = Rc::new(Cell::new(false));
        {
            let page_weak = Rc::downgrade(self);
            let requested_save = Rc::clone(&requested_save);
            dialog.connect_closed(move |_| {
                if requested_save.get() {
                    requested_save.set(false);
                    if let Some(p) = page_weak.upgrade() {
                        p.show_save_preset_dialog();
                    }
                }
            });
        }

        let content = GtkBox::new(Orientation::Vertical, 12);
        content.set_margin_top(12);
        content.set_margin_bottom(12);
        content.set_margin_start(12);
        content.set_margin_end(12);

        let scrolled = ScrolledWindow::new();
        scrolled.set_vexpand(true);
        scrolled.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);

        let list_box = GtkBox::new(Orientation::Vertical, 18);
        list_box.set_margin_top(6);
        list_box.set_margin_bottom(6);
        scrolled.set_child(Some(&list_box));
        content.append(&scrolled);

        let settings = load_settings();
        let favorite = settings.favorite_preset.clone();
        let builtins = builtin_presets();
        let customs: Vec<TimerPresetEntry> = settings.timer_presets.clone();

        let section_label = |text: &str| {
            let lbl = Label::new(Some(text));
            lbl.add_css_class("heading");
            lbl.set_halign(Align::Start);
            lbl.set_margin_start(6);
            lbl
        };

        list_box.append(&section_label("Built-in presets"));

        let builtin_list = ListBox::builder()
            .css_classes(["boxed-list"])
            .selection_mode(gtk4::SelectionMode::None)
            .build();
        list_box.append(&builtin_list);

        let custom_list = ListBox::builder()
            .css_classes(["boxed-list"])
            .selection_mode(gtk4::SelectionMode::None)
            .build();

        // Preset rows in visual order, for the gamepad focus tracker.
        let mut preset_rows: Vec<ActionRow> = Vec::new();

        if customs.is_empty() {
            let empty = Label::new(Some(
                "No custom presets yet \u{2014} save your current setup below.",
            ));
            empty.add_css_class("dim-label");
            empty.set_halign(Align::Start);
            empty.set_margin_start(6);
            list_box.append(&empty);
        } else {
            list_box.append(&section_label("My presets"));
            list_box.append(&custom_list);
        }

        for preset in builtins.iter().chain(customs.iter()) {
            let row = ActionRow::builder()
                .title(&preset.name)
                .subtitle(&preset_summary(&preset.config))
                .activatable(true)
                .build();
            preset_rows.push(row.clone());

            let star_icon = if preset.name == favorite {
                "starred-symbolic"
            } else {
                "non-starred-symbolic"
            };
            let star = Button::builder()
                .icon_name(star_icon)
                .css_classes(["flat"])
                .valign(Align::Center)
                .tooltip_text("Favorite")
                .build();
            {
                let page_weak = Rc::downgrade(self);
                let name = preset.name.clone();
                let is_favorite = preset.name == favorite;
                let dlg = dialog.clone();
                star.connect_clicked(move |_| {
                    if let Some(p) = page_weak.upgrade() {
                        if is_favorite {
                            p.set_favorite_preset("");
                        } else {
                            p.set_favorite_preset(&name);
                        }
                        dlg.close();
                    }
                });
            }
            row.add_suffix(&star);

            let is_custom = settings.timer_presets.iter().any(|p| p.name == preset.name);
            if is_custom {
                let delete = Button::builder()
                    .icon_name("user-trash-symbolic")
                    .css_classes(["flat"])
                    .valign(Align::Center)
                    .tooltip_text("Delete preset")
                    .build();
                {
                    let page_weak = Rc::downgrade(self);
                    let name = preset.name.clone();
                    let dlg = dialog.clone();
                    delete.connect_clicked(move |_| {
                        if let Some(p) = page_weak.upgrade() {
                            let mut s = load_settings();
                            s.timer_presets.retain(|p2| p2.name != name);
                            if s.favorite_preset == name {
                                s.favorite_preset.clear();
                            }
                            save_settings(&s);
                            p.refresh_favorite_pill();
                            dlg.close();
                        }
                    });
                }
                row.add_suffix(&delete);
            }

            {
                let page_weak = Rc::downgrade(self);
                let config = preset.config.clone();
                let dlg = dialog.clone();
                row.connect_activated(move |_| {
                    if let Some(p) = page_weak.upgrade() {
                        p.apply_preset(&config);
                        dlg.close();
                    }
                });
            }
            if is_custom {
                custom_list.append(&row);
            } else {
                builtin_list.append(&row);
            }
        }

        let add_btn = Button::builder()
            .label("Save current setup as preset…")
            .css_classes(["suggested-action"])
            .halign(Align::Center)
            .build();
        {
            let dlg = dialog.clone();
            let requested_save = Rc::clone(&requested_save);
            add_btn.connect_clicked(move |_| {
                requested_save.set(true);
                dlg.close();
            });
        }
        content.append(&add_btn);

        let back_btn = Button::builder()
            .label("Back")
            .halign(Align::Center)
            .css_classes(["flat"])
            .build();
        {
            let dlg = dialog.clone();
            back_btn.connect_clicked(move |_| {
                dlg.close();
            });
        }
        content.append(&back_btn);

        dialog.set_child(Some(&content));

        // Register the dialog with the window so gamepad input is routed to
        // it while it is open (dpad cycles, A activates, B closes).
        {
            let registrar = self.presets_dialog_registrar.borrow().clone();
            if let Some(cb) = registrar {
                let mut tracker = ControllerFocusTracker::new();
                let mut focus_widgets: Vec<gtk4::Widget> = Vec::new();
                for row in &preset_rows {
                    focus_widgets.push(row.clone().upcast());
                }
                focus_widgets.push(add_btn.clone().upcast());
                focus_widgets.push(back_btn.clone().upcast());
                tracker.set_widgets(focus_widgets);
                let controller = Rc::new(PresetsDialogController {
                    dialog: dialog.clone(),
                    tracker: Rc::new(RefCell::new(tracker)),
                });
                cb(Some(Rc::clone(&controller)));
                let cleanup: Rc<dyn Fn()> = {
                    let cb = Rc::clone(&cb);
                    Rc::new(move || cb(None))
                };
                dialog.connect_closed(move |_| {
                    cleanup();
                });
            }
        }

        if let Some(root) = self
            .widget
            .root()
            .and_then(|r| r.dynamic_cast_ref::<gtk4::Window>().cloned())
        {
            dialog.present(Some(&root));
        }
    }

    /// Save the current widget state as a named custom preset.
    fn save_preset(self: &Rc<Self>, name: &str) {
        self.sync_config_from_widgets();
        let config = self.config.borrow().clone();
        let mut settings = load_settings();
        settings.timer_presets.retain(|p| p.name != name);
        settings.timer_presets.push(TimerPresetEntry {
            name: name.to_string(),
            config,
        });
        save_settings(&settings);
    }

    /// Ask for a name and save the current timer setup as a custom preset.
    fn show_save_preset_dialog(self: &Rc<Self>) {
        let dialog = Dialog::new();
        dialog.set_title("Save Preset");
        dialog.set_content_width(360);
        dialog.set_follows_content_size(true);

        let content = GtkBox::new(Orientation::Vertical, 12);
        content.set_margin_top(12);
        content.set_margin_bottom(12);
        content.set_margin_start(12);
        content.set_margin_end(12);

        let name_entry = EntryRow::builder()
            .title("Preset name")
            .show_apply_button(false)
            .build();
        content.append(&name_entry);

        let save_btn = Button::builder()
            .label("Save")
            .css_classes(["suggested-action"])
            .halign(Align::Center)
            .build();
        {
            let page_weak = Rc::downgrade(self);
            let entry = name_entry.clone();
            let dlg = dialog.clone();
            save_btn.connect_clicked(move |_| {
                if let Some(p) = page_weak.upgrade() {
                    let name = entry.text().trim().to_string();
                    if !name.is_empty() {
                        p.save_preset(&name);
                    }
                    dlg.close();
                }
            });
        }
        content.append(&save_btn);

        dialog.set_child(Some(&content));
        if let Some(root) = self
            .widget
            .root()
            .and_then(|r| r.dynamic_cast_ref::<gtk4::Window>().cloned())
        {
            dialog.present(Some(&root));
        }
    }

    fn add_spin_row(
        group: &PreferencesGroup,
        title: &str,
        lower: f64,
        upper: f64,
        value: f64,
        step: f64,
    ) -> SpinRow {
        let adj = gtk4::Adjustment::new(value, lower, upper, step, step * 10.0, 0.0);
        let row = SpinRow::new(Some(&adj), 1.0, 0);
        row.set_title(title);
        group.add(&row);
        row
    }

    /// Rebuild the custom-rounds editor rows.
    fn rebuild_custom_rounds(self: &Rc<Self>) {
        let mut child = self.custom_rounds_box.first_child();
        while let Some(c) = child {
            child = c.next_sibling();
            self.custom_rounds_box.remove(&c);
        }

        let specs = self.per_round.borrow().clone();
        for (idx, spec) in specs.iter().enumerate() {
            let row = GtkBox::new(Orientation::Horizontal, 6);

            let work_adj = gtk4::Adjustment::new(
                spec.work_seconds as f64,
                5.0,
                3600.0,
                5.0,
                60.0,
                0.0,
            );
            let work_spin = SpinRow::new(Some(&work_adj), 1.0, 0);
            work_spin.set_title(&format!("Round {}", idx + 1));
            work_spin.set_subtitle("work");
            {
                let page_weak = Rc::downgrade(self);
                let spin = work_spin.clone();
                work_adj.connect_value_changed(move |_| {
                    if let Some(p) = page_weak.upgrade() {
                        let value = spin.value() as i64;
                        let mut specs = p.per_round.borrow_mut();
                        if idx < specs.len() {
                            specs[idx].work_seconds = value;
                        }
                    }
                });
            }
            row.append(&work_spin);

            let rest_adj = gtk4::Adjustment::new(
                spec.rest_seconds as f64,
                0.0,
                600.0,
                5.0,
                30.0,
                0.0,
            );
            let rest_spin = SpinRow::new(Some(&rest_adj), 1.0, 0);
            rest_spin.set_title("rest");
            {
                let page_weak = Rc::downgrade(self);
                let spin = rest_spin.clone();
                rest_adj.connect_value_changed(move |_| {
                    if let Some(p) = page_weak.upgrade() {
                        let value = spin.value() as i64;
                        let mut specs = p.per_round.borrow_mut();
                        if idx < specs.len() {
                            specs[idx].rest_seconds = value;
                        }
                    }
                });
            }
            row.append(&rest_spin);

            let remove_btn = Button::builder()
                .icon_name("list-remove-symbolic")
                .css_classes(["flat"])
                .valign(Align::Center)
                .tooltip_text("Remove round")
                .build();
            {
                let page_weak = Rc::downgrade(self);
                remove_btn.connect_clicked(move |_| {
                    if let Some(p) = page_weak.upgrade() {
                        let mut specs = p.per_round.borrow_mut();
                        if idx < specs.len() {
                            specs.remove(idx);
                        }
                        drop(specs);
                        p.rebuild_custom_rounds();
                    }
                });
            }
            row.append(&remove_btn);

            self.custom_rounds_box.append(&row);
        }

        // Add button
        let add_btn = Button::builder()
            .label("Add round")
            .css_classes(["flat"])
            .halign(Align::Start)
            .build();
        {
            let page_weak = Rc::downgrade(self);
            add_btn.connect_clicked(move |_| {
                if let Some(p) = page_weak.upgrade() {
                    p.per_round.borrow_mut().push(crate::models::RoundSpec {
                        work_seconds: 30,
                        rest_seconds: 15,
                    });
                    p.rebuild_custom_rounds();
                }
            });
        }
        self.custom_rounds_box.append(&add_btn);

        // The per-round rows (and their buttons) are part of the focus list.
        self.rebuild_focus_list();
    }

    /// Read the current widget state into the config and persist it.
    fn sync_config_from_widgets(&self) {
        let mut config = self.config.borrow_mut();
        config.rounds = self.rounds_spin.value() as i64;
        config.round_seconds = self.round_time_spin.value() as i64;
        config.pause_seconds = self.pause_time_spin.value() as i64;
        config.prep_seconds = self.prep_spin.value() as i64;
        config.warmup = self.warmup_switch.is_active();
        config.title = self.title_entry.text().trim().to_string();
        config.per_round = if self.custom_switch.is_active() {
            let specs = self.per_round.borrow().clone();
            if specs.is_empty() {
                None
            } else {
                Some(specs)
            }
        } else {
            None
        };
        drop(config);

        let mut settings = load_settings();
        settings.timer_config = Some(self.config.borrow().clone());
        save_settings(&settings);
    }

    /// Apply a config to the widgets.
    fn apply_config_to_widgets(self: &Rc<Self>, config: &RoundConfig) {
        self.rounds_spin.set_value(config.rounds as f64);
        self.round_time_spin.set_value(config.round_seconds as f64);
        self.pause_time_spin.set_value(config.pause_seconds as f64);
        self.prep_spin.set_value(config.prep_seconds as f64);
        self.warmup_switch.set_active(config.warmup);
        self.title_entry.set_text(&config.title);
        if let Some(specs) = &config.per_round {
            *self.per_round.borrow_mut() = specs.clone();
            self.custom_switch.set_active(true);
        } else {
            self.custom_switch.set_active(false);
        }
        self.rebuild_custom_rounds();
    }

    fn on_start_clicked(&self) {
        if self.timer.is_running.get() {
            return;
        }

        if self.current_round.get() == 0 {
            self.sync_config_from_widgets();
            let config = self.config.borrow().clone();

            // Custom rounds define their own length.
            let total = match &config.per_round {
                Some(specs) => specs.len().max(1) as i64,
                None => config.rounds.max(1),
            };
            self.total_rounds.set(total);
            self.current_round.set(1);
            self.is_pause.set(false);
            self.summary_label.set_visible(false);
            self.update_round_label();

            // Prepare countdown, then warm-up, then the real rounds.
            if config.prep_seconds > 0 {
                self.phase.set(Phase::Prepare);
                self.set_phase_display("PREPARE", "phase-reps", COLOR_REST);
                self.round_label.set_label("Get ready");
                self.timer.start(config.prep_seconds);
            } else if config.warmup {
                self.phase.set(Phase::Warmup);
                self.set_phase_display("WARM-UP", "phase-rest", COLOR_REST);
                self.round_label.set_label("Warm-up");
                self.timer.start(config.pause_seconds.max(10));
            } else {
                self.phase.set(Phase::Round);
                self.start_round();
            }
        } else {
            self.timer.resume();
        }

        self.update_buttons_running(true);
    }

    fn update_round_label(&self) {
        let config = self.config.borrow();
        if config.per_round.is_some() {
            self.round_label.set_label(&format!(
                "Round {} / {}",
                self.current_round.get(),
                self.total_rounds.get()
            ));
        } else {
            self.round_label.set_label(&format!(
                "Round {} / {}",
                self.current_round.get(),
                self.total_rounds.get()
            ));
        }
    }

    /// Work seconds of the current round (custom rounds win).
    fn current_work_seconds(&self) -> i64 {
        let config = self.config.borrow();
        if let Some(specs) = &config.per_round {
            let idx = (self.current_round.get() - 1).max(0) as usize;
            if let Some(spec) = specs.get(idx) {
                return spec.work_seconds;
            }
        }
        config.round_seconds
    }

    /// Rest seconds after the current round (custom rounds win).
    fn current_rest_seconds(&self) -> i64 {
        let config = self.config.borrow();
        if let Some(specs) = &config.per_round {
            let idx = (self.current_round.get() - 1).max(0) as usize;
            if let Some(spec) = specs.get(idx) {
                return spec.rest_seconds;
            }
        }
        config.pause_seconds
    }

    fn start_round(&self) {
        self.phase.set(Phase::Round);
        self.update_round_label();
        self.set_phase_display("FIGHT", "phase-go", COLOR_GO);
        let settings = load_settings();
        self.sound
            .play_sound(settings.get_sound("round_start_sound").as_deref());
        self.timer.start(self.current_work_seconds());
    }

    fn start_pause(&self) {
        self.set_phase_display("PAUSE", "phase-rest", COLOR_REST);
        let rest = self.current_rest_seconds();
        if rest > 0 {
            let settings = load_settings();
            self.sound
                .play_sound(settings.get_sound("round_end_sound").as_deref());
            self.timer.start(rest);
        } else {
            self.advance_round();
        }
    }

    fn advance_round(&self) {
        self.current_round.set(self.current_round.get() + 1);
        if self.current_round.get() > self.total_rounds.get() {
            self.on_training_complete();
        } else {
            self.start_round();
        }
    }

    fn on_timer_finished(&self) {
        let settings = load_settings();
        self.sound
            .play_sound(settings.get_sound("countdown_tick_sound").as_deref());

        match self.phase.get() {
            Phase::Prepare => {
                // Prepare is over: warm-up (if enabled) or the first round.
                if self.config.borrow().warmup {
                    self.phase.set(Phase::Warmup);
                    self.set_phase_display("WARM-UP", "phase-rest", COLOR_REST);
                    self.round_label.set_label("Warm-up");
                    self.timer.start(self.config.borrow().pause_seconds.max(10));
                } else {
                    self.phase.set(Phase::Round);
                    self.start_round();
                }
            }
            Phase::Warmup => {
                self.phase.set(Phase::Round);
                self.start_round();
            }
            Phase::Rest => {
                self.is_pause.set(false);
                self.phase.set(Phase::Round);
                self.advance_round();
            }
            _ => {
                *self.completed_work_seconds.borrow_mut() += self.current_work_seconds();
                if self.current_round.get() >= self.total_rounds.get() {
                    self.on_training_complete();
                } else if self.current_rest_seconds() > 0 {
                    self.is_pause.set(true);
                    self.phase.set(Phase::Rest);
                    self.start_pause();
                } else {
                    self.phase.set(Phase::Round);
                    self.advance_round();
                }
            }
        }
    }

    fn on_training_complete(&self) {
        self.phase.set(Phase::Done);
        self.set_phase_display("COMPLETE!", "phase-done", COLOR_DONE);
        self.ring.set_progress(1.0);
        self.countdown_label.set_label("00:00");

        // Summary: rounds completed and total work time.
        let rounds = self.total_rounds.get();
        let work_secs: i64 = self.completed_work_seconds.borrow().clone();
        let title = {
            let config = self.config.borrow();
            if config.title.is_empty() {
                String::new()
            } else {
                format!("{} · ", config.title)
            }
        };
        self.summary_label.set_label(&format!(
            "{}{} rounds · {} min work",
            title,
            rounds,
            work_secs / 60
        ));
        self.summary_label.set_visible(true);

        self.current_round.set(0);
        self.update_buttons_running(false);
        let settings = load_settings();
        self.sound
            .play_sound(settings.get_sound("training_complete_sound").as_deref());
        if let Some(root) = self
            .widget
            .root()
            .and_then(|r| r.dynamic_cast_ref::<gtk4::Window>().cloned())
        {
            crate::dialogs::send_notification(&root, "Round Timer", "All rounds complete!");
        }
    }

    fn on_timer_tick(&self, remaining: f64, total: i64) {
        self.countdown_label.set_label(&format_time(remaining));
        if total > 0 {
            self.ring.set_progress(remaining / total as f64);
        }
    }

    fn set_phase_display(&self, text: &str, css_class: &str, color: (f64, f64, f64)) {
        self.phase_label.set_label(text);
        self.phase_label.set_css_classes(&["heading", css_class]);
        self.ring.set_color(color);
    }

    fn on_pause_clicked(&self) {
        if self.timer.is_running.get() {
            self.timer.pause();
            self.pause_btn.set_label("Resume");
            self.ring.set_dimmed(true);
            self.phase_label.add_css_class("dim-label");
        } else {
            self.timer.resume();
            self.pause_btn.set_label("Pause");
            self.ring.set_dimmed(false);
            self.phase_label.remove_css_class("dim-label");
        }
    }

    fn on_reset_clicked(&self) {
        self.timer.stop();
        self.current_round.set(0);
        self.is_pause.set(false);
        self.phase.set(Phase::Idle);
        *self.completed_work_seconds.borrow_mut() = 0;
        self.countdown_label.set_label("00:00");
        self.round_label.set_label("Round 0 / 0");
        self.set_phase_display("", "phase-reps", COLOR_NEUTRAL);
        self.ring.set_progress(1.0);
        self.ring.set_dimmed(false);
        self.summary_label.set_visible(false);
        self.update_buttons_running(false);
    }

    fn on_skip_clicked(&self) {
        self.timer.stop();
        let settings = load_settings();
        self.sound
            .play_sound(settings.get_sound("countdown_tick_sound").as_deref());

        // Skipping during prepare/warm-up jumps straight into round 1.
        match self.phase.get() {
            Phase::Prepare | Phase::Warmup => {
                self.phase.set(Phase::Round);
                self.start_round();
                return;
            }
            Phase::Rest => {
                self.is_pause.set(false);
                self.phase.set(Phase::Round);
                self.advance_round();
                return;
            }
            _ => {}
        }

        *self.completed_work_seconds.borrow_mut() += self.current_work_seconds();
        if self.current_round.get() >= self.total_rounds.get() {
            self.on_training_complete();
        } else if self.current_rest_seconds() > 0 {
            self.is_pause.set(true);
            self.phase.set(Phase::Rest);
            self.start_pause();
        } else {
            self.phase.set(Phase::Round);
            self.advance_round();
        }
    }

    /// Collect the focusable widgets in visual order: configuration rows,
    /// per-round custom rows, preset buttons and the action buttons.
    /// Sensitive-only, so running/disabled widgets drop out automatically.
    fn rebuild_focus_list(&self) {
        let mut widgets: Vec<gtk4::Widget> = Vec::new();
        for w in [
            self.rounds_spin.clone().upcast::<gtk4::Widget>(),
            self.round_time_spin.clone().upcast::<gtk4::Widget>(),
            self.pause_time_spin.clone().upcast::<gtk4::Widget>(),
            self.prep_spin.clone().upcast::<gtk4::Widget>(),
            self.warmup_switch.clone().upcast::<gtk4::Widget>(),
            self.title_entry.clone().upcast::<gtk4::Widget>(),
            self.custom_switch.clone().upcast::<gtk4::Widget>(),
        ] {
            if w.is_sensitive() {
                widgets.push(w);
            }
        }
        // Custom (per-round) rows: work/rest spin rows, remove and add
        // buttons, in the order they appear in the box.
        if self.custom_switch.is_active() && self.custom_rounds_box.is_visible() {
            let mut child = self.custom_rounds_box.first_child();
            while let Some(c) = child {
                child = c.next_sibling();
                let mut row_child = c.first_child();
                while let Some(rc) = row_child {
                    row_child = rc.next_sibling();
                    if rc.is_sensitive() {
                        widgets.push(rc);
                    }
                }
            }
        }
        for btn in [&self.favorite_preset_btn, &self.presets_btn] {
            if btn.is_sensitive() && btn.is_visible() {
                widgets.push(btn.clone().upcast());
            }
        }
        for w in [
            self.start_btn.clone().upcast::<gtk4::Widget>(),
            self.pause_btn.clone().upcast::<gtk4::Widget>(),
            self.reset_btn.clone().upcast::<gtk4::Widget>(),
            self.skip_btn.clone().upcast::<gtk4::Widget>(),
        ] {
            if w.is_sensitive() {
                widgets.push(w);
            }
        }
        self.focus.borrow_mut().set_widgets(widgets);
    }

    fn update_buttons_running(&self, running: bool) {
        self.pause_btn.set_sensitive(running);
        self.reset_btn
            .set_sensitive(running || self.current_round.get() > 0);
        self.skip_btn.set_sensitive(running);
        if !running {
            self.pause_btn.set_label("Pause");
        }
        let config_editable = !running && self.current_round.get() == 0;
        self.rounds_spin.set_sensitive(config_editable);
        self.round_time_spin.set_sensitive(config_editable);
        self.pause_time_spin.set_sensitive(config_editable);

        self.rebuild_focus_list();
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn countdown_label(&self) -> gtk4::Label {
        self.countdown_label.clone()
    }

    /// The gamepad focus list, in visual order (used by the tests to
    /// verify every config row is reachable).
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn focus_widgets(&self) -> Vec<gtk4::Widget> {
        self.focus.borrow().widgets()
    }

    pub fn update_fonts(&self, width: i32, height: i32) {
        apply_scaling(
            &[
                (LabelType::Info, &self.round_label),
                (LabelType::Info, &self.phase_label),
            ],
            width,
            height,
        );

        // The ring shrinks on short windows so the page keeps fitting; the
        // countdown follows the ring size.
        if height > 0 {
            let size = ((height as f64 * 0.34) as i32).clamp(170, 300);
            self.ring.set_size(size);
            crate::ui_scaling::set_label_size(&self.countdown_label, (size as f64 * 0.20) as i32);
        }
    }

    pub fn controller_dpad_up(&self) {
        self.focus.borrow_mut().cycle(-1);
    }

    pub fn controller_dpad_down(&self) {
        self.focus.borrow_mut().cycle(1);
    }

    pub fn controller_dpad_left(&self) {
        if let Some(widget) = self.focus.borrow().current_widget() {
            if let Some(spin) = widget.downcast_ref::<SpinRow>() {
                let adj = spin.adjustment();
                spin.set_value(spin.value() - adj.step_increment());
            }
        }
    }

    pub fn controller_dpad_right(&self) {
        if let Some(widget) = self.focus.borrow().current_widget() {
            if let Some(spin) = widget.downcast_ref::<SpinRow>() {
                let adj = spin.adjustment();
                spin.set_value(spin.value() + adj.step_increment());
            } else if widget.downcast_ref::<Button>().is_some() {
                let buttons = [
                    &self.start_btn,
                    &self.pause_btn,
                    &self.reset_btn,
                    &self.skip_btn,
                ];
                if let Some(pos) = buttons
                    .iter()
                    .position(|b| b.upcast_ref::<gtk4::Widget>().as_ptr() == widget.as_ptr())
                {
                    let next = (pos + 1) % buttons.len();
                    self.focus.borrow_mut().move_to_widget(buttons[next].upcast_ref());
                }
            }
        }
    }

    pub fn controller_start(&self) {
        if self.timer.is_running.get() {
            self.on_pause_clicked();
        } else {
            self.on_start_clicked();
        }
    }

    pub fn controller_x(&self) {
        self.on_reset_clicked();
    }

    pub fn controller_back(&self) {
        self.on_skip_clicked();
    }

    pub fn controller_a(&self) {
        // A activates the focused widget: opens the presets menu, clicks
        // buttons and toggles switches. Start/pause lives on the Start
        // button instead, so A never races the timer.
        let widget = self.focus.borrow().current_widget();
        let Some(widget) = widget else { return };
        if let Some(btn) = widget.downcast_ref::<gtk4::Button>() {
            btn.emit_clicked();
        } else if let Some(switch) = widget.downcast_ref::<SwitchRow>() {
            switch.set_active(!switch.is_active());
        } else if let Some(entry) = widget.downcast_ref::<libadwaita::EntryRow>() {
            entry.grab_focus();
        }
    }
}

fn format_time(seconds: f64) -> String {
    let total = (seconds.max(0.0)) as i64;
    format!("{:02}:{:02}", total / 60, total % 60)
}

/// Short human-readable summary of a preset config for the dialog rows.
fn preset_summary(config: &RoundConfig) -> String {
    match &config.per_round {
        Some(specs) => format!(
            "{} rounds \u{00b7} {}",
            specs.len(),
            specs
                .iter()
                .map(|s| format!("{}/{}", s.work_seconds, s.rest_seconds))
                .collect::<Vec<_>>()
                .join(" ")
        ),
        None => format!(
            "{} rounds \u{00b7} {}/{}",
            config.rounds, config.round_seconds, config.pause_seconds
        ),
    }
}

