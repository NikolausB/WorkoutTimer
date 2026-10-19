use crate::controller_focus::ControllerFocusTracker;
use crate::models::RoundConfig;
use crate::progress_ring::{ProgressRing, COLOR_DONE, COLOR_GO, COLOR_NEUTRAL, COLOR_REST};
use crate::settings::load_settings;
use crate::sound::SoundPlayer;
use crate::timer::TimerCore;
use crate::ui_scaling::{apply_scaling, LabelType};
use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Button, Label, Orientation};
use libadwaita::prelude::*;
use libadwaita::{Bin, PreferencesGroup, SpinRow};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

struct TimerPreset {
    label: &'static str,
    rounds: i64,
    round_seconds: i64,
    pause_seconds: i64,
}

const TIMER_PRESETS: &[TimerPreset] = &[
    TimerPreset { label: "Boxing 3 × 3min", rounds: 3, round_seconds: 180, pause_seconds: 60 },
    TimerPreset { label: "Tabata 8 × 20/10", rounds: 8, round_seconds: 20, pause_seconds: 10 },
    TimerPreset { label: "HIIT 10 × 30/30", rounds: 10, round_seconds: 30, pause_seconds: 30 },
    TimerPreset { label: "Endurance 5 × 5min", rounds: 5, round_seconds: 300, pause_seconds: 60 },
];

pub struct RoundTimerPage {
    widget: Bin,
    config: RefCell<RoundConfig>,
    timer: Rc<TimerCore>,
    current_round: Rc<Cell<i64>>,
    is_pause: Rc<Cell<bool>>,
    total_rounds: Rc<Cell<i64>>,
    focus: Rc<RefCell<ControllerFocusTracker>>,
    sound: Rc<SoundPlayer>,

    rounds_spin: SpinRow,
    round_time_spin: SpinRow,
    pause_time_spin: SpinRow,
    ring: ProgressRing,
    round_label: Label,
    phase_label: Label,
    countdown_label: Label,
    start_btn: Button,
    pause_btn: Button,
    reset_btn: Button,
    skip_btn: Button,
    preset_btns: Vec<Button>,
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

        let config_group = PreferencesGroup::builder().title("Configuration").build();

        let rounds_spin = Self::add_spin_row(&config_group, "Rounds", 1.0, 50.0, 10.0, 1.0);
        let round_time_spin =
            Self::add_spin_row(&config_group, "Round duration (seconds)", 10.0, 3600.0, 180.0, 10.0);
        let pause_time_spin =
            Self::add_spin_row(&config_group, "Pause duration (seconds)", 0.0, 600.0, 60.0, 5.0);

        main_box.append(&config_group);

        // Quick presets
        let preset_box = GtkBox::new(Orientation::Horizontal, 8);
        preset_box.set_halign(Align::Center);
        preset_box.set_margin_top(4);
        let mut preset_btns = Vec::new();
        for preset in TIMER_PRESETS {
            let btn = Button::builder()
                .label(preset.label)
                .css_classes(["pill"])
                .build();
            preset_box.append(&btn);
            preset_btns.push(btn);
        }
        main_box.append(&preset_box);

        let timer_group = PreferencesGroup::builder().title("Timer").build();
        timer_group.set_margin_top(12);

        let round_label = Label::new(Some("Round 0 / 0"));
        round_label.add_css_class("title-2");
        timer_group.add(&round_label);

        let phase_label = Label::new(Some(""));
        phase_label.set_css_classes(&["heading", "phase-reps"]);
        timer_group.add(&phase_label);

        main_box.append(&timer_group);

        // Circular ring with countdown in the middle
        let ring = ProgressRing::new(300);
        let ring_overlay = gtk4::Overlay::new();
        ring_overlay.set_child(Some(ring.widget()));
        let countdown_label = Label::new(Some("00:00"));
        countdown_label.set_halign(Align::Center);
        countdown_label.set_valign(Align::Center);
        ring_overlay.add_overlay(&countdown_label);
        ring_overlay.set_vexpand(true);
        ring_overlay.set_valign(Align::Center);
        main_box.append(&ring_overlay);

        let btn_box = GtkBox::new(Orientation::Horizontal, 12);
        btn_box.set_halign(Align::Center);
        btn_box.set_margin_bottom(12);

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
        widget.set_child(Some(&main_box));

        let timer = Rc::new(TimerCore::new());
        let current_round = Rc::new(Cell::new(0i64));
        let is_pause = Rc::new(Cell::new(false));
        let total_rounds = Rc::new(Cell::new(0i64));

        let page = Rc::new(Self {
            widget,
            config: RefCell::new(RoundConfig::default()),
            timer: Rc::clone(&timer),
            current_round: Rc::clone(&current_round),
            is_pause: Rc::clone(&is_pause),
            total_rounds: Rc::clone(&total_rounds),
            focus: Rc::new(RefCell::new(ControllerFocusTracker::new())),
            sound,
            rounds_spin: rounds_spin.clone(),
            round_time_spin: round_time_spin.clone(),
            pause_time_spin: pause_time_spin.clone(),
            ring,
            round_label: round_label.clone(),
            phase_label: phase_label.clone(),
            countdown_label: countdown_label.clone(),
            start_btn: start_btn.clone(),
            pause_btn: pause_btn.clone(),
            reset_btn: reset_btn.clone(),
            skip_btn: skip_btn.clone(),
            preset_btns,
        });

        // Preset chips fill the spin rows
        for (btn, preset) in page.preset_btns.iter().zip(TIMER_PRESETS) {
            let page_weak = Rc::downgrade(&page);
            let (rounds, round_s, pause_s) =
                (preset.rounds, preset.round_seconds, preset.pause_seconds);
            btn.connect_clicked(move |_| {
                if let Some(p) = page_weak.upgrade() {
                    if p.rounds_spin.is_sensitive() {
                        p.rounds_spin.set_value(rounds as f64);
                        p.round_time_spin.set_value(round_s as f64);
                        p.pause_time_spin.set_value(pause_s as f64);
                    }
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

        {
            let presets: Vec<gtk4::Widget> = page.preset_btns
                .iter()
                .map(|b| b.clone().upcast::<gtk4::Widget>())
                .collect();
            let mut initial: Vec<gtk4::Widget> = vec![
                rounds_spin.clone().upcast(),
                round_time_spin.clone().upcast(),
                pause_time_spin.clone().upcast(),
            ];
            initial.extend(presets);
            initial.extend(vec![
                start_btn.clone().upcast(),
                pause_btn.clone().upcast(),
                reset_btn.clone().upcast(),
                skip_btn.clone().upcast(),
            ]);
            page.focus.borrow_mut().set_widgets(initial);
        }

        page
    }

    pub fn widget(&self) -> &Bin {
        &self.widget
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

    fn on_start_clicked(&self) {
        if self.timer.is_running.get() {
            return;
        }

        if self.current_round.get() == 0 {
            self.config.borrow_mut().rounds = self.rounds_spin.value() as i64;
            self.config.borrow_mut().round_seconds = self.round_time_spin.value() as i64;
            self.config.borrow_mut().pause_seconds = self.pause_time_spin.value() as i64;
            self.total_rounds.set(self.config.borrow().rounds);
            self.current_round.set(1);
            self.is_pause.set(false);
            self.start_round();
        } else {
            self.timer.resume();
        }

        self.update_buttons_running(true);
    }

    fn start_round(&self) {
        self.round_label.set_label(&format!(
            "Round {} / {}",
            self.current_round.get(),
            self.total_rounds.get()
        ));
        self.set_phase_display("FIGHT", "phase-go", COLOR_GO);
        let settings = load_settings();
        self.sound
            .play_sound(settings.get_sound("round_start_sound").as_deref());
        self.timer.start(self.config.borrow().round_seconds);
    }

    fn start_pause(&self) {
        self.set_phase_display("PAUSE", "phase-rest", COLOR_REST);
        if self.config.borrow().pause_seconds > 0 {
            let settings = load_settings();
            self.sound
                .play_sound(settings.get_sound("round_end_sound").as_deref());
            self.timer.start(self.config.borrow().pause_seconds);
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
        if self.is_pause.get() {
            self.is_pause.set(false);
            self.advance_round();
        } else {
            if self.current_round.get() >= self.total_rounds.get() {
                self.on_training_complete();
            } else if self.config.borrow().pause_seconds > 0 {
                self.is_pause.set(true);
                self.start_pause();
            } else {
                self.advance_round();
            }
        }
    }

    fn on_training_complete(&self) {
        self.set_phase_display("COMPLETE!", "phase-done", COLOR_DONE);
        self.ring.set_progress(1.0);
        self.countdown_label.set_label("00:00");
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
        self.countdown_label.set_label("00:00");
        self.round_label.set_label("Round 0 / 0");
        self.set_phase_display("", "phase-reps", COLOR_NEUTRAL);
        self.ring.set_progress(1.0);
        self.ring.set_dimmed(false);
        self.update_buttons_running(false);
    }

    fn on_skip_clicked(&self) {
        self.timer.stop();
        let settings = load_settings();
        self.sound
            .play_sound(settings.get_sound("countdown_tick_sound").as_deref());
        if self.is_pause.get() {
            self.is_pause.set(false);
            self.advance_round();
        } else {
            if self.current_round.get() >= self.total_rounds.get() {
                self.on_training_complete();
            } else if self.config.borrow().pause_seconds > 0 {
                self.is_pause.set(true);
                self.start_pause();
            } else {
                self.advance_round();
            }
        }
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

        let mut widgets: Vec<gtk4::Widget> = Vec::new();
        for w in [
            &self.rounds_spin.clone().upcast::<gtk4::Widget>(),
            &self.round_time_spin.clone().upcast::<gtk4::Widget>(),
            &self.pause_time_spin.clone().upcast::<gtk4::Widget>(),
        ] {
            if w.is_sensitive() {
                widgets.push(w.clone());
            }
        }
        for btn in &self.preset_btns {
            if btn.is_sensitive() {
                widgets.push(btn.clone().upcast());
            }
        }
        for w in [
            &self.start_btn.clone().upcast::<gtk4::Widget>(),
            &self.pause_btn.clone().upcast::<gtk4::Widget>(),
            &self.reset_btn.clone().upcast::<gtk4::Widget>(),
            &self.skip_btn.clone().upcast::<gtk4::Widget>(),
        ] {
            if w.is_sensitive() {
                widgets.push(w.clone());
            }
        }
        self.focus.borrow_mut().set_widgets(widgets);
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn countdown_label(&self) -> gtk4::Label {
        self.countdown_label.clone()
    }

    pub fn update_fonts(&self, width: i32, height: i32) {
        apply_scaling(
            &[
                (LabelType::Timer, &self.countdown_label),
                (LabelType::Info, &self.round_label),
                (LabelType::Info, &self.phase_label),
            ],
            width,
            height,
        );
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
        self.on_start_clicked();
    }
}

fn format_time(seconds: f64) -> String {
    let total = (seconds.max(0.0)) as i64;
    format!("{:02}:{:02}", total / 60, total % 60)
}

