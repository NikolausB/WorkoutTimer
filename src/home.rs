use crate::controller_focus::ControllerFocusTracker;
use crate::data_store::DataStore;
use crate::models::TrainingPlan;
use chrono::Local;
use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Button, Label, Orientation, ScrolledWindow};
use libadwaita::prelude::*;
use libadwaita::{ActionRow, Bin, Clamp, PreferencesGroup};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub struct HomePage {
    widget: Bin,
    store: Rc<DataStore>,
    on_open_plan: RefCell<Rc<dyn Fn(&TrainingPlan)>>,
    on_switch_to_plans: Rc<RefCell<Option<Rc<dyn Fn()>>>>,
    focus: Rc<RefCell<ControllerFocusTracker>>,
    stats_box: GtkBox,
    recent_card: PreferencesGroup,
    recommended_card: PreferencesGroup,
    recent_row: Rc<RefCell<Option<ActionRow>>>,
    recommended_row: Rc<RefCell<Option<ActionRow>>>,
    browse_btn: Button,
}

impl HomePage {
    pub fn new(
        store: Rc<DataStore>,
        initial_open_plan: Rc<dyn Fn(&TrainingPlan)>,
    ) -> Rc<Self> {
        let widget = Bin::new();

        let scrolled = ScrolledWindow::builder().vexpand(true).build();
        let clamp = Clamp::new();
        let box_widget = GtkBox::new(Orientation::Vertical, 24);
        box_widget.set_margin_top(36);
        box_widget.set_margin_bottom(36);
        box_widget.set_margin_start(24);
        box_widget.set_margin_end(24);

        let welcome = Label::new(Some("Welcome back!"));
        welcome.add_css_class("title-1");
        box_widget.append(&welcome);

        // Dashboard stats row
        let stats_box = GtkBox::new(Orientation::Horizontal, 12);
        stats_box.set_homogeneous(true);
        box_widget.append(&stats_box);

        let recent_card = PreferencesGroup::builder().title("Continue Training").build();
        box_widget.append(&recent_card);

        let recommended_card = PreferencesGroup::builder().title("Recommended").build();
        box_widget.append(&recommended_card);

        let on_switch = Rc::new(RefCell::new(None::<Rc<dyn Fn()>>));
        let on_switch_clone = Rc::clone(&on_switch);
        let browse_btn = Button::builder()
            .label("Browse Training Plans")
            .css_classes(["pill"])
            .halign(Align::Center)
            .build();
        browse_btn.connect_clicked(move |_| {
            if let Some(ref cb) = *on_switch_clone.borrow() {
                cb();
            }
        });
        box_widget.append(&browse_btn);

        clamp.set_child(Some(&box_widget));
        scrolled.set_child(Some(&clamp));
        widget.set_child(Some(&scrolled));

        Rc::new(Self {
            widget,
            store,
            on_open_plan: RefCell::new(initial_open_plan),
            on_switch_to_plans: on_switch,
            focus: Rc::new(RefCell::new(ControllerFocusTracker::new())),
            stats_box,
            recent_card,
            recommended_card,
            recent_row: Rc::new(RefCell::new(None)),
            recommended_row: Rc::new(RefCell::new(None)),
            browse_btn,
        })
    }

    pub fn widget(&self) -> &Bin {
        &self.widget
    }

    pub fn set_on_switch_to_plans(&self, cb: Rc<dyn Fn()>) {
        *self.on_switch_to_plans.borrow_mut() = Some(cb);
    }

    pub fn set_on_open_plan(&self, cb: Rc<dyn Fn(&TrainingPlan)>) {
        *self.on_open_plan.borrow_mut() = cb;
    }

    pub fn refresh(&self) {
        self.update_stats();
        self.update_recent();
        self.update_recommended();
        self.refresh_focus();
        self.focus.borrow_mut().focus_first();
    }

    fn update_stats(&self) {
        // Rebuild cards
        let mut child = self.stats_box.first_child();
        while let Some(c) = child {
            child = c.next_sibling();
            self.stats_box.remove(&c);
        }

        let sessions = self.store.load_sessions();
        let today = Local::now().date_naive();

        // Sessions within the last 7 days
        let week_count = sessions
            .iter()
            .filter(|s| (today - s.started_at.date_naive()).num_days() < 7)
            .count();

        // Total training time
        let total_secs: i64 = sessions.iter().map(|s| s.total_actual_seconds).sum();
        let total_h = total_secs / 3600;
        let total_m = (total_secs % 3600) / 60;
        let total_str = if total_h > 0 {
            format!("{}h {}m", total_h, total_m)
        } else {
            format!("{}m", total_m)
        };

        // Streak: consecutive days (ending today or yesterday) with >= 1 session
        let mut days: std::collections::HashSet<chrono::NaiveDate> =
            sessions.iter().map(|s| s.started_at.date_naive()).collect();
        let mut streak = 0;
        let mut cursor = if days.contains(&today) {
            today
        } else {
            today.pred_opt().unwrap_or(today)
        };
        while days.remove(&cursor) {
            streak += 1;
            cursor = cursor.pred_opt().unwrap_or(cursor);
        }

        let cards = [
            (week_count.to_string(), "Sessions · 7 days"),
            (total_str, "Total Time"),
            (streak.to_string(), "Day Streak"),
        ];
        for (value, caption) in cards {
            let card = GtkBox::new(gtk4::Orientation::Vertical, 2);
            card.add_css_class("card");
            card.add_css_class("stat-card");
            card.set_halign(gtk4::Align::Fill);
            let value_label = Label::new(Some(&value));
            value_label.add_css_class("stat-value");
            value_label.set_halign(gtk4::Align::Center);
            let caption_label = Label::new(Some(caption));
            caption_label.add_css_class("stat-caption");
            caption_label.add_css_class("dim-label");
            caption_label.set_halign(gtk4::Align::Center);
            card.append(&value_label);
            card.append(&caption_label);
            self.stats_box.append(&card);
        }
    }

    fn refresh_focus(&self) {
        let mut widgets: Vec<gtk4::Widget> = Vec::new();
        if let Some(ref row) = *self.recent_row.borrow() {
            widgets.push(row.clone().upcast::<gtk4::Widget>());
        }
        if let Some(ref row) = *self.recommended_row.borrow() {
            widgets.push(row.clone().upcast::<gtk4::Widget>());
        }
        widgets.push(self.browse_btn.clone().upcast());
        self.focus.borrow_mut().set_widgets(widgets);
    }

    fn update_recent(&self) {
        if let Some(row) = self.recent_row.borrow().clone() {
            self.recent_card.remove(&row);
        }
        *self.recent_row.borrow_mut() = None;

        let sessions = self.store.load_sessions();
        let store = Rc::clone(&self.store);
        let on_open = Rc::clone(&self.on_open_plan.borrow());
        let on_switch = Rc::clone(&self.on_switch_to_plans);

        if sessions.is_empty() {
            let row = ActionRow::builder()
                .title("No training yet")
                .subtitle("Start your first workout!")
                .activatable(true)
                .build();
            {
                let on_switch = Rc::clone(&on_switch);
                row.connect_activated(move |_| {
                    if let Some(ref cb) = *on_switch.borrow() {
                        cb();
                    }
                });
            }
            let go_btn = Button::builder()
                .label("Browse")
                .css_classes(["flat"])
                .build();
            {
                let on_switch = Rc::clone(&on_switch);
                go_btn.connect_clicked(move |_| {
                    if let Some(ref cb) = *on_switch.borrow() {
                        cb();
                    }
                });
            }
            row.add_suffix(&go_btn);
            *self.recent_row.borrow_mut() = Some(row.clone());
            self.recent_card.add(&row);
            return;
        }

        let last = &sessions[0];
        let plan = self
            .store
            .get_plan(&last.plan_id)
            .or_else(|| {
                self.store
                    .load_plans()
                    .into_iter()
                    .find(|p| p.name == last.plan_name)
            });

        let plan_name = plan.as_ref().map(|p| p.name.clone()).unwrap_or_else(|| last.plan_name.clone());
        let date_str = last.started_at.format("%Y-%m-%d %H:%M").to_string();
        let ex_count = last.exercises.len();
        let rounds_str = if let Some(ref p) = plan {
            if p.total_rounds > 1 {
                format!(" ({} rounds)", p.total_rounds)
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        let row = ActionRow::builder()
            .title(&plan_name)
            .subtitle(format!("Last: {} | {} exercises{}", date_str, ex_count, rounds_str))
            .activatable(true)
            .build();

        {
            let store = Rc::clone(&store);
            let on_open = Rc::clone(&on_open);
            row.connect_activated(move |_| {
                let sessions = store.load_sessions();
                if sessions.is_empty() {
                    return;
                }
                let last = &sessions[0];
                let plan = store
                    .get_plan(&last.plan_id)
                    .or_else(|| store.load_plans().into_iter().find(|p| p.name == last.plan_name));
                if let Some(p) = plan {
                    on_open(&p);
                }
            });
        }

        if let Some(ref p) = plan {
            let open_btn = Button::builder()
                .label("Open")
                .css_classes(["suggested-action", "flat"])
                .build();
            let p_clone = p.clone();
            let on_open = Rc::clone(&on_open);
            open_btn.connect_clicked(move |_| {
                on_open(&p_clone);
            });
            row.add_suffix(&open_btn);
        }

        *self.recent_row.borrow_mut() = Some(row.clone());
        self.recent_card.add(&row);
    }

    fn update_recommended(&self) {
        if let Some(row) = self.recommended_row.borrow().clone() {
            self.recommended_card.remove(&row);
        }
        *self.recommended_row.borrow_mut() = None;

        let sessions = self.store.load_sessions();
        let plans = self.store.load_plans();
        let on_open = Rc::clone(&self.on_open_plan.borrow());

        if plans.is_empty() {
            let row = ActionRow::builder().title("No plans available").build();
            *self.recommended_row.borrow_mut() = Some(row.clone());
            self.recommended_card.add(&row);
            return;
        }

        let (recommended, subtitle) = if !sessions.is_empty() {
            let mut counts: HashMap<String, usize> = HashMap::new();
            for s in &sessions {
                *counts.entry(s.plan_name.clone()).or_insert(0) += 1;
            }
            let most_common = counts
                .iter()
                .max_by_key(|(_, c)| *c)
                .map(|(n, _)| n.clone());
            let recommended = most_common
                .and_then(|name| plans.iter().find(|p| p.name == name).cloned())
                .unwrap_or_else(|| plans[0].clone());
            let count = counts.get(&recommended.name).copied().unwrap_or(0);
            let subtitle = format!(
                "Completed {} time{}",
                count,
                if count != 1 { "s" } else { "" }
            );
            (recommended, subtitle)
        } else {
            let recommended = plans
                .iter()
                .find(|p| p.name == "Full Body Beginner")
                .cloned()
                .unwrap_or_else(|| plans[0].clone());
            (recommended, "Great for beginners".to_string())
        };

        let mut subtitle = subtitle;
        if recommended.total_rounds > 1 {
            let rbr = if recommended.rest_between_rounds_seconds > 0 {
                format!(", {}s rest", recommended.rest_between_rounds_seconds)
            } else {
                String::new()
            };
            subtitle = format!("{} ({} rounds{})", subtitle, recommended.total_rounds, rbr);
        }

        let row = ActionRow::builder()
            .title(&recommended.name)
            .subtitle(&subtitle)
            .activatable(true)
            .build();

        let open_btn = Button::builder()
            .label("Open")
            .css_classes(["suggested-action", "flat"])
            .build();
        let rec_clone = recommended.clone();
        let on_open_clone = Rc::clone(&on_open);
        open_btn.connect_clicked(move |_| {
            on_open_clone(&rec_clone);
        });
        row.add_suffix(&open_btn);

        {
            let on_open = Rc::clone(&on_open);
            let rec = recommended.clone();
            row.connect_activated(move |_| {
                on_open(&rec);
            });
        }

        *self.recommended_row.borrow_mut() = Some(row.clone());
        self.recommended_card.add(&row);
    }

    pub fn controller_dpad_up(&self) {
        self.focus.borrow_mut().cycle(-1);
    }

    pub fn controller_dpad_down(&self) {
        self.focus.borrow_mut().cycle(1);
    }

    pub fn controller_a(&self) {
        if let Some(widget) = self.focus.borrow().current_widget() {
            if let Some(btn) = widget.downcast_ref::<Button>() {
                btn.emit_clicked();
            } else if let Some(row) = widget.downcast_ref::<ActionRow>() {
                libadwaita::prelude::ActionRowExt::activate(&*row);
            }
        }
    }

    pub fn controller_back(&self) {
        if let Some(ref cb) = *self.on_switch_to_plans.borrow() {
            cb();
        }
    }

    pub fn update_fonts(&self, _width: i32, _height: i32) {}
}

