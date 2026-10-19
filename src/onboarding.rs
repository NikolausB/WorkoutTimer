use crate::data_store::DataStore;
use crate::models::TrainingPlan;
use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Button, CheckButton, Label, Orientation, Orientation::Vertical, SpinButton};
use libadwaita::prelude::*;
use std::cell::Cell;
use std::rc::Rc;

/// First-run wizard: goal, equipment and frequency, then it drops matching
/// starter plans into the user's plan list.
pub struct OnboardingWindow {
    window: libadwaita::Window,
}

const GOALS: &[(&str, &str)] = &[
    ("General fitness", "A bit of everything, stay active"),
    ("Build muscle", "Strength training with progressive loads"),
    ("Lose weight", "HIIT and cardio to burn calories"),
    ("Endurance", "Longer efforts and stamina"),
    ("Mobility & core", "Flexibility, core work and stability"),
];

/// Template keywords per goal (used to rank the bundled starter plans).
fn goal_keywords(goal: usize) -> &'static [&'static str] {
    match goal {
        1 => &["Strength", "Kettlebell", "Full Body", "Power"],
        2 => &["HIIT", "Cardio", "Full Body"],
        3 => &["Endurance", "HIIT", "Full Body", "Kettlebell"],
        4 => &["Core", "Full Body"],
        _ => &["Full Body", "Beginner", "Core", "HIIT"],
    }
}

impl OnboardingWindow {
    pub fn new(parent: &libadwaita::Window, store: Rc<DataStore>) -> Self {
        let window = libadwaita::Window::builder()
            .title("Welcome")
            .transient_for(parent)
            .modal(true)
            .default_width(520)
            .default_height(560)
            .build();

        let stack = gtk4::Stack::new();
        let goal = Rc::new(Cell::new(0usize));
        let equipment = Rc::new(Cell::new(0u32));
        let days = Rc::new(Cell::new(3i64));
        let summary_label = Label::new(None);
        summary_label.add_css_class("dim-label");
        summary_label.set_wrap(true);

        // ---- Step 1: goal ----
        let goal_page = GtkBox::new(Vertical, 8);
        let goal_title = Label::new(Some("What is your main goal?"));
        goal_title.add_css_class("title-2");
        goal_page.append(&goal_title);
        let goal_group = libadwaita::PreferencesGroup::new();
        let mut goal_buttons: Vec<CheckButton> = Vec::new();
        for (i, (title, subtitle)) in GOALS.iter().enumerate() {
            let btn = CheckButton::builder()
                .label(*title)
                .active(i == 0)
                .build();
            let subtitle_label = Label::new(Some(*subtitle));
            subtitle_label.add_css_class("dim-label");
            subtitle_label.set_halign(Align::Start);
            subtitle_label.set_margin_start(28);
            goal_group.add(&btn);
            goal_group.add(&subtitle_label);
            let g = goal.clone();
            btn.connect_toggled(move |b| {
                if b.is_active() {
                    g.set(i);
                }
            });
            goal_buttons.push(btn);
        }
        goal_page.append(&goal_group);
        stack.add_named(&goal_page, Some("goal"));

        // ---- Step 2: equipment ----
        let equip_page = GtkBox::new(Vertical, 8);
        let equip_title = Label::new(Some("What equipment do you have?"));
        equip_title.add_css_class("title-2");
        equip_page.append(&equip_title);
        let equip_group = libadwaita::PreferencesGroup::new();
        for (i, name) in ["Full gym", "Dumbbells", "Resistance bands", "Bodyweight only"]
            .iter()
            .enumerate()
        {
            let btn = CheckButton::builder()
                .label(*name)
                .active(i == 3)
                .build();
            let e = equipment.clone();
            btn.connect_toggled(move |b| {
                if b.is_active() {
                    e.set(i as u32);
                }
            });
            equip_group.add(&btn);
        }
        equip_page.append(&equip_group);
        stack.add_named(&equip_page, Some("equipment"));

        // ---- Step 3: frequency + start ----
        let freq_page = GtkBox::new(Vertical, 8);
        let freq_title = Label::new(Some("How often do you train?"));
        freq_title.add_css_class("title-2");
        freq_page.append(&freq_title);
        let freq_group = libadwaita::PreferencesGroup::new();
        let days_spin = SpinButton::with_range(1.0, 7.0, 1.0);
        days_spin.set_value(3.0);
        let days_row = libadwaita::ActionRow::builder()
            .title("Days per week")
            .subtitle("You can change the plan later")
            .build();
        days_row.add_suffix(&days_spin);
        freq_group.add(&days_row);
        freq_page.append(&freq_group);

        let start_title = Label::new(Some("Your starter plans"));
        start_title.add_css_class("title-3");
        freq_page.append(&start_title);
        freq_page.append(&summary_label);
        stack.add_named(&freq_page, Some("frequency"));

        // ---- Navigation ----
        let next_btn = Button::builder().label("Next").css_classes(["suggested-action"]).build();
        let back_btn = Button::builder().label("Back").css_classes(["flat"]).build();
        let finish_btn = Button::builder()
            .label("Start Training")
            .css_classes(["suggested-action"])
            .build();
        let nav_box = GtkBox::new(Orientation::Horizontal, 12);
        nav_box.set_halign(Align::End);
        nav_box.set_margin_top(12);
        nav_box.append(&back_btn);
        nav_box.append(&next_btn);
        nav_box.append(&finish_btn);

        let content = GtkBox::new(Vertical, 12);
        content.set_margin_top(24);
        content.set_margin_bottom(24);
        content.set_margin_start(24);
        content.set_margin_end(24);
        content.append(&stack);
        content.append(&nav_box);

        let toolbar = libadwaita::ToolbarView::new();
        toolbar.set_content(Some(&content));
        window.set_content(Some(&toolbar));

        let summary_for_next = summary_label.clone();
        let stack_for_next = stack.clone();
        let goal_for_next = goal.clone();
        let equipment_for_next = equipment.clone();
        next_btn.connect_clicked(move |_| {
            let current = stack_for_next
                .visible_child_name()
                .map(|n| n.to_string())
                .unwrap_or_else(|| "goal".to_string());
            match current.as_str() {
                "goal" => stack_for_next.set_visible_child_name("equipment"),
                "equipment" => {
                    let plans = recommend_plans(goal_for_next.get(), equipment_for_next.get());
                    let names: Vec<String> = plans
                        .iter()
                        .map(|p| format!("\u{b7} {}", p.name))
                        .collect();
                    summary_for_next.set_label(&format!(
                        "We will add these plans to your list:\n{}",
                        names.join("\n")
                    ));
                    stack_for_next.set_visible_child_name("frequency");
                }
                _ => {}
            }
        });

        let stack_for_back = stack.clone();
        back_btn.connect_clicked(move |_| {
            let current = stack_for_back
                .visible_child_name()
                .map(|n| n.to_string())
                .unwrap_or_else(|| "goal".to_string());
            match current.as_str() {
                "equipment" => stack_for_back.set_visible_child_name("goal"),
                "frequency" => stack_for_back.set_visible_child_name("equipment"),
                _ => {}
            }
        });

        let goal_for_finish = goal.clone();
        let equipment_for_finish = equipment.clone();
        let days_for_finish = days.clone();
        let store_for_finish = Rc::clone(&store);
        let window_for_close = window.clone();
        finish_btn.connect_clicked(move |_| {
            let plans = recommend_plans(goal_for_finish.get(), equipment_for_finish.get());
            let count = plans.len().min(days_for_finish.get().max(1) as usize);
            for template in plans.iter().take(count) {
                let mut plan: TrainingPlan = template.clone();
                plan.id = uuid::Uuid::new_v4().to_string();
                plan.created = chrono::Local::now();
                plan.is_favorite = false;
                store_for_finish.save_plan(&plan);
            }
            let mut settings = crate::settings::load_settings();
            settings.onboarding_done = true;
            crate::settings::save_settings(&settings);
            window_for_close.close();
            crate::window::refresh_all_pages();
            crate::window::show_app_toast("Welcome! Your starter plans are ready.");
        });

        // Sync the days spin into the state.
        {
            let days = days.clone();
            days_spin.connect_value_changed(move |s| {
                days.set(s.value() as i64);
            });
        }

        Self { window }
    }

    pub fn present(&self) {
        self.window.present();
    }
}

/// Rank the bundled starter plans for the chosen goal and pick the best ones.
pub fn recommend_plans(goal: usize, equipment: u32) -> Vec<TrainingPlan> {
    let templates = crate::image_utils::load_default_plans();
    let keywords = goal_keywords(goal);
    let bodyweight = equipment == 3;

    let mut scored: Vec<(i32, TrainingPlan)> = templates
        .into_iter()
        .map(|plan| {
            let name = plan.name.to_lowercase();
            let mut score = 0;
            for kw in keywords {
                if name.contains(&kw.to_lowercase()) {
                    score += 10;
                }
            }
            // Bodyweight-only users get a bonus for beginner-friendly plans.
            if bodyweight && name.contains("beginner") {
                score += 5;
            }
            (score, plan)
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0));
    scored.into_iter().take(3).map(|(_, p)| p).collect()
}
