use crate::controller_focus::ControllerFocusTracker;
use crate::settings::{load_settings, save_settings, AppSettings};
use crate::sound::AVAILABLE_SOUNDS as SOUND_KEYS;
use crate::window::show_app_toast;
use gtk4::prelude::*;
use gtk4::{Align, Button};
use libadwaita::prelude::*;
use libadwaita::{ButtonRow, ComboRow, PreferencesGroup, PreferencesPage, SpinRow, SwitchRow};
use std::cell::RefCell;
use std::rc::Rc;

const SOUND_DISPLAY_NAMES: &[(&str, &str)] = &[
    ("beep", "Beep"),
    ("round_start", "Ding (Round Start)"),
    ("round_end", "Bell (Round End)"),
    ("exercise_complete", "Chime (Exercise Complete)"),
    ("training_complete", "Fanfare (Training Complete)"),
    ("none", "None (Silent)"),
];

const EVENT_KEYS: &[&str] = &[
    "round_start_sound",
    "round_end_sound",
    "countdown_tick_sound",
    "exercise_complete_sound",
    "training_complete_sound",
];

const EVENT_LABELS: &[(&str, &str)] = &[
    ("round_start_sound", "Round Start Sound"),
    ("round_end_sound", "Round End / Pause Sound"),
    ("countdown_tick_sound", "Countdown Tick Sound"),
    ("exercise_complete_sound", "Exercise Complete Sound"),
    ("training_complete_sound", "Training Complete Sound"),
];

const EVENT_SUBTITLES: &[(&str, &str)] = &[
    ("round_start_sound", "Played when a round or exercise begins"),
    ("round_end_sound", "Played when a round ends or rest starts"),
    ("countdown_tick_sound", "Played on timer countdown transitions"),
    ("exercise_complete_sound", "Played when a single exercise is completed"),
    (
        "training_complete_sound",
        "Played when the entire training session finishes",
    ),
];

fn sound_key_to_index(key: &str) -> u32 {
    for (i, (k, _)) in SOUND_DISPLAY_NAMES.iter().enumerate() {
        if *k == key {
            return i as u32;
        }
    }
    0
}

fn index_to_sound_key(index: u32) -> String {
    SOUND_DISPLAY_NAMES
        .get(index as usize)
        .map(|(k, _)| k.to_string())
        .unwrap_or_else(|| "beep".to_string())
}

pub struct PreferencesDialog {
    dialog: libadwaita::PreferencesDialog,
    settings: RefCell<AppSettings>,
    focus: RefCell<ControllerFocusTracker>,
    color_scheme_combo: ComboRow,
    sound_switch: SwitchRow,
    volume_spin: SpinRow,
    combo_rows: Vec<(String, ComboRow)>,
    home_switch: SwitchRow,
    timer_switch: SwitchRow,
    workout_switch: SwitchRow,
    ai_switch: SwitchRow,
    gamepad_switch: SwitchRow,
    hints_switch: SwitchRow,
    deck_combo: ComboRow,
    export_backup_row: ButtonRow,
    restore_backup_row: ButtonRow,
}

impl PreferencesDialog {
    pub fn new(parent: &gtk4::Window) -> Rc<Self> {
        let dialog = libadwaita::PreferencesDialog::builder()
            .title("Preferences")
            .search_enabled(false)
            .build();
        dialog.set_content_width(560);
        dialog.set_content_height(640);

        let settings = load_settings();

        let page = PreferencesPage::builder().title("General").build();

        // ---- Appearance group ----
        let appearance_group = PreferencesGroup::builder()
            .title("Appearance")
            .description("Application look and feel")
            .build();

        let color_scheme_combo = ComboRow::new();
        color_scheme_combo.set_title("Color Scheme");
        color_scheme_combo.set_subtitle("Follow the system or force light/dark appearance");
        let color_items = gtk4::StringList::new(&["Follow System", "Light", "Dark"]);
        color_scheme_combo.set_model(Some(&color_items));
        let idx = match settings.color_scheme.as_str() {
            "light" => 1,
            "dark" => 2,
            _ => 0,
        };
        color_scheme_combo.set_selected(idx);
        appearance_group.add(&color_scheme_combo);

        page.add(&appearance_group);

        // ---- Sound group ----
        let sound_group = PreferencesGroup::builder()
            .title("Sounds")
            .description("Choose sounds played during training events")
            .build();

        let sound_switch = SwitchRow::builder().title("Sound Enabled").build();
        sound_switch.set_active(settings.sound_enabled);
        sound_group.add(&sound_switch);

        let volume_adj = gtk4::Adjustment::new(
            settings.sound_volume * 100.0,
            0.0,
            100.0,
            5.0,
            10.0,
            0.0,
        );
        let volume_spin = SpinRow::new(Some(&volume_adj), 1.0, 0);
        volume_spin.set_title("Volume");
        volume_spin.set_subtitle("Sound playback volume");
        volume_spin.set_value(settings.sound_volume * 100.0);
        volume_spin.set_sensitive(settings.sound_enabled);
        sound_group.add(&volume_spin);

        let options: Vec<&str> = SOUND_KEYS
            .iter()
            .copied()
            .chain(std::iter::once("none"))
            .collect();
        let display: Vec<String> = options
            .iter()
            .map(|s| {
                SOUND_DISPLAY_NAMES
                    .iter()
                    .find(|(k, _)| k == s)
                    .map(|(_, d)| d.to_string())
                    .unwrap_or_else(|| s.to_string())
            })
            .collect();
        let string_list = gtk4::StringList::new(&display.iter().map(|s| s.as_str()).collect::<Vec<_>>());

        let mut combo_rows: Vec<(String, ComboRow)> = Vec::new();

        let settings_snapshot = settings.clone();
        for event_key in EVENT_KEYS {
            let title = EVENT_LABELS
                .iter()
                .find(|(k, _)| k == event_key)
                .map(|(_, t)| *t)
                .unwrap_or(event_key);
            let subtitle = EVENT_SUBTITLES
                .iter()
                .find(|(k, _)| k == event_key)
                .map(|(_, s)| *s)
                .unwrap_or("");

            let row = ComboRow::new();
            row.set_title(title);
            row.set_subtitle(subtitle);
            row.set_model(Some(&string_list));

            let current: Option<String> = settings_snapshot.get_event_sound(event_key);
            row.set_selected(sound_key_to_index(current.as_deref().unwrap_or("beep")));
            row.set_sensitive(settings_snapshot.sound_enabled);

            let preview_btn = Button::builder()
                .icon_name("media-playback-start-symbolic")
                .css_classes(["flat"])
                .build();
            preview_btn.set_valign(Align::Center);
            preview_btn.set_sensitive(settings_snapshot.sound_enabled);

            row.add_suffix(&preview_btn);
            sound_group.add(&row);
            combo_rows.push((event_key.to_string(), row));
        }

        page.add(&sound_group);

        // ---- Tabs group ----
        let tabs_group = PreferencesGroup::builder()
            .title("Tabs")
            .description("Choose which tabs are shown in the main view")
            .build();

        let home_switch = SwitchRow::builder()
            .title("Show Home Page")
            .subtitle("Landing page with recent and recommended workouts")
            .build();
        home_switch.set_active(settings.show_home_page);
        tabs_group.add(&home_switch);

        let timer_switch = SwitchRow::builder()
            .title("Show Round Timer")
            .subtitle("Configurable round timer with pause periods")
            .build();
        timer_switch.set_active(settings.show_timer_page);
        tabs_group.add(&timer_switch);

        let workout_switch = SwitchRow::builder()
            .title("Show Training Plans")
            .subtitle("Training plan builder and runner")
            .build();
        workout_switch.set_active(settings.show_workout_page);
        tabs_group.add(&workout_switch);

        let ai_switch = SwitchRow::builder()
            .title("Show AI Coach")
            .subtitle("AI-powered training plan generator (requires network access)")
            .build();
        ai_switch.set_active(settings.show_ai_page);
        tabs_group.add(&ai_switch);

        page.add(&tabs_group);

        // ---- Controller group ----
        let controller_group = PreferencesGroup::builder()
            .title("Controller and Deck Mode")
            .description("Settings for gamepad and Steam Deck compatibility")
            .build();

        let gamepad_switch = SwitchRow::builder()
            .title("Enable Gamepad")
            .subtitle("Support for controllers and Steam Deck input")
            .build();
        gamepad_switch.set_active(settings.gamepad_enabled);
        controller_group.add(&gamepad_switch);

        let hints_switch = SwitchRow::builder()
            .title("Show Button Hints")
            .subtitle("Display controller button hints during workouts")
            .build();
        hints_switch.set_active(settings.gamepad_hints);
        controller_group.add(&hints_switch);

        let deck_combo = ComboRow::new();
        deck_combo.set_title("Deck Mode");
        deck_combo.set_subtitle("Auto-detect or force Steam Deck UI scaling");
        let deck_items = gtk4::StringList::new(&["Auto", "On", "Off"]);
        deck_combo.set_model(Some(&deck_items));
        let deck_idx = match settings.deck_mode.as_str() {
            "on" => 1,
            "off" => 2,
            _ => 0,
        };
        deck_combo.set_selected(deck_idx);
        controller_group.add(&deck_combo);

        page.add(&controller_group);

        // ---- Backup group ----
        let backup_group = PreferencesGroup::builder()
            .title("Backup")
            .description("Export or restore all data (plans, history, settings, custom exercises)")
            .build();

        let export_backup_row = ButtonRow::builder().title("Export Backup…").build();
        let restore_backup_row = ButtonRow::builder().title("Restore Backup…").build();
        backup_group.add(&export_backup_row);
        backup_group.add(&restore_backup_row);

        page.add(&backup_group);

        dialog.add(&page);

        let prefs = Rc::new(Self {
            dialog,
            settings: RefCell::new(settings),
            focus: RefCell::new(ControllerFocusTracker::new()),
            color_scheme_combo,
            sound_switch,
            volume_spin,
            combo_rows,
            home_switch,
            timer_switch,
            workout_switch,
            ai_switch,
            gamepad_switch,
            hints_switch,
            deck_combo,
            export_backup_row,
            restore_backup_row,
        });

        // Backup export/restore
        {
            let parent = parent.clone();
            let row = prefs.export_backup_row.clone();
            row.connect_activated(move |_| {
                let store = Rc::new(crate::data_store::DataStore::new());
                crate::dialogs::save_dialog(
                    &parent,
                    "Export Backup",
                    "training-backup.json",
                    "*.json",
                    "JSON files",
                    move |path| {
                        if let Some(path) = path {
                            match crate::backup::export_backup(&store, &path) {
                                Ok(()) => crate::window::show_app_toast("Backup exported"),
                                Err(e) => {
                                    crate::window::show_app_toast(&format!("Backup failed: {}", e))
                                }
                            }
                        }
                    },
                );
            });
        }
        {
            let parent = parent.clone();
            let row = prefs.restore_backup_row.clone();
            row.connect_activated(move |_| {
                let store = Rc::new(crate::data_store::DataStore::new());
                crate::dialogs::open_dialog(
                    &parent,
                    "Restore Backup",
                    Some(("*.json", "JSON files")),
                    move |path| {
                        if let Some(path) = path {
                            match crate::backup::import_backup(&store, &path) {
                                Ok(summary) => {
                                    crate::window::show_app_toast(&format!(
                                        "Restored: {} plans, {} sessions, {} exercises",
                                        summary.plans_added,
                                        summary.sessions_added,
                                        summary.user_exercises_added
                                    ));
                                    let settings = load_settings();
                                    apply_color_scheme(&settings.color_scheme);
                                    if let Some(mw) = crate::window::current_window() {
                                        mw.refresh_all_pages();
                                    }
                                }
                                Err(e) => {
                                    crate::window::show_app_toast(&format!("Restore failed: {}", e))
                                }
                            }
                        }
                    },
                );
            });
        }

        // Sound enabled toggle
        {
            let prefs_weak = Rc::downgrade(&prefs);
            prefs.sound_switch.connect_notify_local(Some("active"), move |sw, _| {
                let Some(p) = prefs_weak.upgrade() else { return };
                active_toggle(&p, sw.is_active());
            });
        }

        // Volume applies live
        {
            let prefs_weak = Rc::downgrade(&prefs);
            prefs.volume_spin.connect_value_notify(move |spin| {
                if let Some(p) = prefs_weak.upgrade() {
                    p.settings.borrow_mut().sound_volume = (spin.value() / 100.0).clamp(0.0, 1.0);
                }
            });
        }

        // Sound events
        for (event_key, row) in prefs.combo_rows.clone() {
            let key = event_key.clone();
            let prefs_weak = Rc::downgrade(&prefs);
            row.connect_selected_notify(move |cr| {
                let Some(p) = prefs_weak.upgrade() else { return };
                let new_key = index_to_sound_key(cr.selected());
                p.settings.borrow_mut().set_event_sound(&key, &new_key);
            });
        }

        // Live color scheme preview
        {
            let prefs_weak = Rc::downgrade(&prefs);
            prefs.color_scheme_combo.connect_selected_notify(move |cr| {
                let Some(p) = prefs_weak.upgrade() else { return };
                let scheme = ["default", "light", "dark"][cr.selected() as usize];
                p.settings.borrow_mut().color_scheme = scheme.to_string();
                apply_color_scheme(scheme);
            });
        }

        // Save when the dialog closes (no explicit save button needed with
        // Adw.PreferencesDialog; changes apply to the in-memory snapshot live).
        {
            let prefs_weak = Rc::downgrade(&prefs);
            prefs.dialog.connect_closed(move |_| {
                let Some(p) = prefs_weak.upgrade() else { return };
                p.on_save_clicked();
            });
        }

        prefs
    }

    pub fn widget(&self) -> &libadwaita::PreferencesDialog {
        &self.dialog
    }

    pub fn present(&self, parent: &gtk4::Window) {
        self.dialog.present(Some(parent));
        self.refresh_focus();
        self.focus.borrow_mut().focus_first();
    }

    fn on_save_clicked(&self) {
        let mut s = self.settings.borrow_mut();
        s.sound_enabled = self.sound_switch.is_active();
        s.sound_volume = (self.volume_spin.value() / 100.0).clamp(0.0, 1.0);
        s.show_home_page = self.home_switch.is_active();
        s.show_timer_page = self.timer_switch.is_active();
        s.show_workout_page = self.workout_switch.is_active();
        s.show_ai_page = self.ai_switch.is_active();
        s.gamepad_enabled = self.gamepad_switch.is_active();
        s.gamepad_hints = self.hints_switch.is_active();
        s.deck_mode = ["auto", "on", "off"][self.deck_combo.selected() as usize].to_string();
        s.color_scheme = ["default", "light", "dark"][self.color_scheme_combo.selected() as usize]
            .to_string();
        // Sound per-event values are already updated live by the combo callbacks
        save_settings(&s);
        apply_color_scheme(&s.color_scheme);
        show_app_toast("Preferences saved");
    }

    // ---- Controller API ----

    pub fn controller_dpad_up(&self) {
        self.focus.borrow_mut().cycle(-1);
    }

    pub fn controller_dpad_down(&self) {
        self.focus.borrow_mut().cycle(1);
    }

    pub fn controller_dpad_left(&self) {
        self.adjust_focused(-1);
    }

    pub fn controller_dpad_right(&self) {
        self.adjust_focused(1);
    }

    fn adjust_focused(&self, delta: i32) {
        if let Some(widget) = self.focus.borrow().current_widget() {
            if let Some(combo) = widget.downcast_ref::<ComboRow>() {
                if let Some(model) = combo.model() {
                    let n = model.n_items() as i32;
                    let new_idx = combo.selected() as i32 + delta;
                    if new_idx >= 0 && new_idx < n {
                        combo.set_selected(new_idx as u32);
                    }
                }
            } else if let Some(sw) = widget.downcast_ref::<SwitchRow>() {
                sw.set_active(!sw.is_active());
            } else if let Some(spin) = widget.downcast_ref::<SpinRow>() {
                let adj = spin.adjustment();
                let new = (spin.value() + delta as f64 * adj.step_increment())
                    .clamp(adj.lower(), adj.upper());
                spin.set_value(new);
            }
        }
    }

    pub fn controller_a(&self) {
        if let Some(widget) = self.focus.borrow().current_widget() {
            if let Some(sw) = widget.downcast_ref::<SwitchRow>() {
                sw.set_active(!sw.is_active());
            } else if let Some(combo) = widget.downcast_ref::<ComboRow>() {
                if let Some(model) = combo.model() {
                    if model.n_items() > 0 {
                        combo.set_selected((combo.selected() + 1) % model.n_items());
                    }
                }
            } else if let Some(row) = widget.downcast_ref::<ButtonRow>() {
                row.activate();
            }
        }
    }

    pub fn controller_b(&self) {
        self.dialog.close();
    }

    pub fn controller_start(&self) {
        self.on_save_clicked();
    }

    fn refresh_focus(&self) {
        let mut widgets: Vec<gtk4::Widget> = Vec::new();
        widgets.push(self.sound_switch.clone().upcast());
        for (_, row) in &self.combo_rows {
            widgets.push(row.clone().upcast());
        }
        widgets.push(self.home_switch.clone().upcast());
        widgets.push(self.timer_switch.clone().upcast());
        widgets.push(self.workout_switch.clone().upcast());
        widgets.push(self.ai_switch.clone().upcast());
        widgets.push(self.gamepad_switch.clone().upcast());
        widgets.push(self.hints_switch.clone().upcast());
        widgets.push(self.deck_combo.clone().upcast());
        widgets.push(self.color_scheme_combo.clone().upcast());
        widgets.push(self.volume_spin.clone().upcast());
        widgets.push(self.export_backup_row.clone().upcast());
        widgets.push(self.restore_backup_row.clone().upcast());
        self.focus.borrow_mut().set_widgets(widgets);
    }
}

fn active_toggle(prefs: &PreferencesDialog, active_v: bool) {
    for (_, row) in &prefs.combo_rows {
        row.set_sensitive(active_v);
    }
    prefs.volume_spin.set_sensitive(active_v);
}

pub fn apply_color_scheme(scheme: &str) {
    use libadwaita as adw;
    let style = adw::StyleManager::default();
    let color_scheme = match scheme {
        "light" => adw::ColorScheme::ForceLight,
        "dark" => adw::ColorScheme::ForceDark,
        _ => adw::ColorScheme::Default,
    };
    style.set_color_scheme(color_scheme);
}

trait EventSoundAccess {
    fn get_event_sound(&self, event_key: &str) -> Option<String>;
    fn set_event_sound(&mut self, event_key: &str, sound: &str);
}

impl EventSoundAccess for AppSettings {
    fn get_event_sound(&self, event_key: &str) -> Option<String> {
        self.get_sound(event_key)
    }
    fn set_event_sound(&mut self, event_key: &str, sound: &str) {
        match event_key {
            "round_start_sound" => self.round_start_sound = sound.to_string(),
            "round_end_sound" => self.round_end_sound = sound.to_string(),
            "countdown_tick_sound" => self.countdown_tick_sound = sound.to_string(),
            "exercise_complete_sound" => self.exercise_complete_sound = sound.to_string(),
            "training_complete_sound" => self.training_complete_sound = sound.to_string(),
            _ => {}
        }
    }
}

