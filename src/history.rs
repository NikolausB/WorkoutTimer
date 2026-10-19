use crate::bar_chart::BarChart;
use crate::controller_focus::ControllerFocusTracker;
use crate::csv_io::{export_history_csv, import_history_csv};
use crate::data_store::DataStore;
use crate::models::TrainingSession;
use crate::window::show_app_toast;
use chrono::Datelike;
use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, Label, ListBox, Orientation, ScrolledWindow,
};
use libadwaita::prelude::*;
use libadwaita::{
    ActionRow, Bin, ButtonRow, Clamp, NavigationPage, NavigationView, PreferencesGroup,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub struct HistoryPage {
    widget: Bin,
    store: Rc<DataStore>,
    nav: NavigationView,
    list_page: NavigationPage,
    list_stack: gtk4::Stack,
    session_list_box: ListBox,
    chart: BarChart,
    chart_title: Label,
    chart_mode: Cell<u8>, // 0 = 8 weeks, 1 = 12 months
    stats_box: GtkBox,
    sessions: RefCell<Vec<TrainingSession>>,
    session_rows: RefCell<Vec<ActionRow>>,
    on_switch_to_timer: RefCell<Option<Rc<dyn Fn()>>>,
    self_weak: RefCell<std::rc::Weak<HistoryPage>>,
    current_session_id: RefCell<Option<String>>,
    list_focus: RefCell<ControllerFocusTracker>,
    detail_focus: RefCell<ControllerFocusTracker>,
    detail_page: NavigationPage,
    detail_header: Label,
    detail_info: Label,
    detail_exercises_box: GtkBox,
    delete_btn: ButtonRow,
}

impl HistoryPage {
    pub fn new(store: Rc<DataStore>) -> Rc<Self> {
        let widget = Bin::new();
        let nav = NavigationView::new();

        // List page
        let list_box = GtkBox::new(Orientation::Vertical, 12);
        list_box.set_margin_top(24);
        list_box.set_margin_bottom(24);
        list_box.set_margin_start(24);
        list_box.set_margin_end(24);

        let io_box = GtkBox::new(Orientation::Horizontal, 6);
        io_box.set_halign(Align::End);
        let export_btn = Button::builder()
            .label("Export")
            .icon_name("document-save-symbolic")
            .css_classes(["flat"])
            .tooltip_text("Export history to CSV")
            .build();
        io_box.append(&export_btn);
        let import_btn = Button::builder()
            .label("Import")
            .icon_name("document-open-symbolic")
            .css_classes(["flat"])
            .tooltip_text("Import history from CSV")
            .build();
        io_box.append(&import_btn);
        list_box.append(&io_box);

        // Statistics: chart + totals
        let stats_box = GtkBox::new(Orientation::Horizontal, 12);
        stats_box.set_homogeneous(true);
        stats_box.set_margin_top(4);
        list_box.append(&stats_box);

        // Chart range toggle
        let chart_toggle_box = GtkBox::new(Orientation::Horizontal, 0);
        chart_toggle_box.set_halign(Align::End);
        chart_toggle_box.add_css_class("linked");
        let weeks_btn = gtk4::ToggleButton::builder()
            .label("8 Weeks")
            .active(true)
            .build();
        let months_btn = gtk4::ToggleButton::builder().label("12 Months").build();
        months_btn.set_group(Some(&weeks_btn));
        chart_toggle_box.append(&weeks_btn);
        chart_toggle_box.append(&months_btn);
        list_box.append(&chart_toggle_box);

        let chart = BarChart::new(150);
        let chart_box = GtkBox::new(Orientation::Vertical, 6);
        chart_box.add_css_class("card");
        chart_box.set_margin_top(6);
        chart_box.set_margin_bottom(4);
        let chart_title = Label::new(Some("Minutes per week (last 8 weeks)"));
        chart_title.add_css_class("dim-label");
        chart_title.set_halign(Align::Start);
        chart_box.append(&chart_title);
        chart_box.append(chart.widget());
        list_box.append(&chart_box);

        let list_stack = gtk4::Stack::new();
        list_stack.set_vhomogeneous(true);
        list_stack.set_vexpand(true);

        let scrolled = ScrolledWindow::new();
        let session_list_box = ListBox::new();
        session_list_box.add_css_class("boxed-list");
        session_list_box.set_activate_on_single_click(true);
        scrolled.set_child(Some(&session_list_box));
        list_stack.add_named(&scrolled, Some("list"));

        // Empty state with a shortcut to the round timer
        let empty_box = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
        empty_box.set_vexpand(true);
        empty_box.set_valign(gtk4::Align::Center);
        empty_box.set_halign(gtk4::Align::Center);
        let empty_label = Label::new(Some("No training sessions yet"));
        empty_label.add_css_class("title-3");
        empty_box.append(&empty_label);
        let empty_hint = Label::new(Some(
            "Finish a workout and it will show up here with your progress.",
        ));
        empty_hint.add_css_class("dim-label");
        empty_box.append(&empty_hint);
        let empty_timer_btn = gtk4::Button::builder()
            .label("Start a Round Timer")
            .css_classes(["suggested-action"])
            .build();
        empty_box.append(&empty_timer_btn);
        list_stack.add_named(&empty_box, Some("empty"));

        list_box.append(&list_stack);

        let list_page = NavigationPage::builder().title("History").build();
        list_page.set_title("History");
        list_page.set_child(Some(&list_box));
        nav.add(&list_page);

        // Detail page
        let detail_toolbar = libadwaita::ToolbarView::new();
        let detail_header_bar = libadwaita::HeaderBar::new();
        detail_header_bar.set_show_start_title_buttons(false);
        detail_header_bar.set_show_end_title_buttons(false);
        detail_toolbar.add_top_bar(&detail_header_bar);

        let detail_scrolled = ScrolledWindow::new();
        detail_scrolled.set_vexpand(true);
        let detail_clamp = Clamp::new();
        let detail_box = GtkBox::new(Orientation::Vertical, 12);
        detail_box.set_margin_top(12);
        detail_box.set_margin_bottom(12);
        detail_box.set_margin_start(12);
        detail_box.set_margin_end(12);

        let detail_header = Label::new(None);
        detail_header.add_css_class("title-2");
        detail_box.append(&detail_header);

        let detail_info = Label::new(None);
        detail_info.add_css_class("dim-label");
        detail_box.append(&detail_info);

        let detail_exercises_box = GtkBox::new(Orientation::Vertical, 6);
        detail_box.append(&detail_exercises_box);

        let actions_group = PreferencesGroup::new();
        let delete_btn = ButtonRow::builder()
            .title("Delete Session")
            .css_classes(["destructive-action"])
            .build();
        actions_group.add(&delete_btn);
        detail_box.append(&actions_group);

        detail_clamp.set_child(Some(&detail_box));
        detail_scrolled.set_child(Some(&detail_clamp));
        detail_toolbar.set_content(Some(&detail_scrolled));

        let detail_page = NavigationPage::builder().title("Session").build();
        detail_page.set_title("Session");
        detail_page.set_child(Some(&detail_toolbar));
        nav.add(&detail_page);

        widget.set_child(Some(&nav));

        let page = Rc::new(Self {
            widget,
            store: Rc::clone(&store),
            nav: nav.clone(),
            list_page: list_page.clone(),
            list_stack: list_stack.clone(),
            session_list_box: session_list_box.clone(),
            chart,
            chart_title,
            chart_mode: Cell::new(0),
            stats_box,
            sessions: RefCell::new(Vec::new()),
            session_rows: RefCell::new(Vec::new()),
            on_switch_to_timer: RefCell::new(None),
            self_weak: RefCell::new(std::rc::Weak::new()),
            current_session_id: RefCell::new(None),
            list_focus: RefCell::new(ControllerFocusTracker::new()),
            detail_focus: RefCell::new(ControllerFocusTracker::new()),
            detail_page: detail_page.clone(),
            detail_header: detail_header.clone(),
            detail_info: detail_info.clone(),
            detail_exercises_box: detail_exercises_box.clone(),
            delete_btn: delete_btn.clone(),
        });

        // Wire signals
        *page.self_weak.borrow_mut() = Rc::downgrade(&page);
        {
            let page_weak = Rc::downgrade(&page);
            empty_timer_btn.connect_clicked(move |_| {
                if let Some(p) = page_weak.upgrade() {
                    if let Some(ref cb) = *p.on_switch_to_timer.borrow() {
                        cb();
                    }
                }
            });
        }
        {
            let page_weak = Rc::downgrade(&page);
            let btn = months_btn.clone();
            btn.connect_toggled(move |b| {
                if !b.is_active() {
                    return;
                }
                if let Some(p) = page_weak.upgrade() {
                    p.chart_mode.set(1);
                    let sessions = p.sessions.borrow().clone();
                    p.update_stats_chart(&sessions);
                }
            });
        }
        {
            let page_weak = Rc::downgrade(&page);
            let btn = weeks_btn.clone();
            btn.connect_toggled(move |b| {
                if !b.is_active() {
                    return;
                }
                if let Some(p) = page_weak.upgrade() {
                    p.chart_mode.set(0);
                    let sessions = p.sessions.borrow().clone();
                    p.update_stats_chart(&sessions);
                }
            });
        }
        {
            let page_weak = Rc::downgrade(&page);
            session_list_box.connect_row_activated(move |_lb, row| {
                let Some(p) = page_weak.upgrade() else { return };
                let idx = p.session_rows.borrow().iter().position(|r| {
                    r.upcast_ref::<gtk4::Widget>().as_ptr()
                        == row.upcast_ref::<gtk4::Widget>().as_ptr()
                });
                let sessions = p.sessions.borrow();
                if let Some(idx) = idx {
                    if idx < sessions.len() {
                        let session = sessions[idx].clone();
                        drop(sessions);
                        p.show_detail(&session);
                    }
                }
            });
        }

        {
            let page_weak = Rc::downgrade(&page);
            delete_btn.connect_activated(move |_| {
                let Some(p) = page_weak.upgrade() else { return };
                let Some(session_id) = p.current_session_id.borrow().clone() else {
                    return;
                };
                let Some(session) = p.store.get_session(&session_id) else {
                    return;
                };
                p.store.delete_session(&session_id);
                *p.current_session_id.borrow_mut() = None;
                p.nav.pop_to_page(&p.list_page);
                p.refresh();

                // Undo: put the session back (same id) and refresh.
                let store = Rc::clone(&p.store);
                crate::window::show_app_toast_action(
                    "Session deleted",
                    "Undo",
                    Rc::new(move || {
                        store.save_session(&session);
                        crate::window::refresh_all_pages();
                    }),
                );
            });
        }

        {
            let page_weak = Rc::downgrade(&page);
            export_btn.connect_clicked(move |_| {
                let Some(p) = page_weak.upgrade() else { return };
                p.on_export_clicked();
            });
        }
        {
            let page_weak = Rc::downgrade(&page);
            import_btn.connect_clicked(move |_| {
                let Some(p) = page_weak.upgrade() else { return };
                p.on_import_clicked();
            });
        }

        page
    }

    pub fn widget(&self) -> &Bin {
        &self.widget
    }

    pub fn set_on_switch_to_timer(&self, cb: Rc<dyn Fn()>) {
        *self.on_switch_to_timer.borrow_mut() = Some(cb);
    }

    pub fn refresh(&self) {
        let sessions = self.store.load_sessions();
        self.sessions.replace(sessions.clone());
        self.update_stats_chart(&sessions);

        // Clear rows
        while let Some(row) = self.session_list_box.row_at_index(0) {
            self.session_list_box.remove(&row);
        }
        self.session_rows.replace(Vec::new());

        if sessions.is_empty() {
            self.list_stack.set_visible_child_name("empty");
            self.list_focus.borrow_mut().set_widgets(Vec::new());
            return;
        }

        let mut last_month = String::new();
        let mut session_rows: Vec<ActionRow> = Vec::new();
        for session in &sessions {
            let planned_min = session.total_planned_seconds / 60;
            let planned_sec = session.total_planned_seconds % 60;
            let actual_min = session.total_actual_seconds / 60;
            let actual_sec = session.total_actual_seconds % 60;
            let month_str = session
                .started_at
                .format("%B %Y")
                .to_string();
            if month_str != last_month {
                last_month = month_str.clone();
                let header_box = GtkBox::new(Orientation::Horizontal, 6);
                header_box.set_margin_top(12);
                header_box.set_margin_bottom(2);
                let header = Label::new(Some(&month_str));
                header.add_css_class("heading");
                header.add_css_class("caption");
                header.set_halign(Align::Start);
                header_box.append(&header);
                let header_row = gtk4::ListBoxRow::new();
                header_row.set_child(Some(&header_box));
                header_row.set_selectable(false);
                header_row.set_activatable(false);
                header_row.add_css_class("month-header");
                self.session_list_box.append(&header_row);
            }

            let title = &session.plan_name;
            let subtitle = format!(
                "{} - Actual: {:02}:{:02} / Planned: {:02}:{:02}",
                session.started_at.format("%d.%m.%Y %H:%M"),
                actual_min,
                actual_sec,
                planned_min,
                planned_sec
            );

            let list_row = ActionRow::builder()
                .title(title)
                .subtitle(&subtitle)
                .activatable(true)
                .build();
            self.session_list_box.append(&list_row);
            session_rows.push(list_row);
        }

        self.session_rows.replace(session_rows);

        self.list_stack.set_visible_child_name("list");
        self.refresh_list_focus();
        self.list_focus.borrow_mut().focus_first();
    }

    /// Build (label, minutes) bars for the current chart mode.
    fn chart_data(&self, sessions: &[TrainingSession]) -> (Vec<(String, f64)>, String) {
        let today = chrono::Local::now().date_naive();

        if self.chart_mode.get() == 1 {
            // Last 12 calendar months
            let mut bars: Vec<(String, f64)> = Vec::new();
            for months_ago in (0..12).rev() {
                let total = today.year() * 12 + (today.month() as i32 - 1) - months_ago;
                let y = total.div_euclid(12);
                let mo = (total.rem_euclid(12) as u32) + 1;
                let start = chrono::NaiveDate::from_ymd_opt(y, mo, 1).unwrap();
                let (ny, nm) = if mo == 12 { (y + 1, 1) } else { (y, mo + 1) };
                let end = chrono::NaiveDate::from_ymd_opt(ny, nm, 1).unwrap();
                let minutes: f64 = sessions
                    .iter()
                    .filter(|s| {
                        let d = s.started_at.date_naive();
                        d >= start && d < end
                    })
                    .map(|s| s.total_actual_seconds as f64 / 60.0)
                    .sum();
                bars.push((start.format("%b").to_string(), minutes));
            }
            return (bars, "Minutes per month (last 12 months)".to_string());
        }

        // Last 8 ISO weeks
        let current_week_start = iso_week_start(today);
        let mut bars: Vec<(String, f64)> = Vec::new();
        for weeks_ago in (0..8).rev() {
            let ws = current_week_start - chrono::Days::new(weeks_ago as u64 * 7);
            let we = ws + chrono::Days::new(7);
            let minutes: f64 = sessions
                .iter()
                .filter(|s| {
                    let d = s.started_at.date_naive();
                    d >= ws && d < we
                })
                .map(|s| s.total_actual_seconds as f64 / 60.0)
                .sum();
            bars.push((ws.format("%d.%m.").to_string(), minutes));
        }
        (bars, "Minutes per week (last 8 weeks)".to_string())
    }

    /// Chart + totals, honoring the 8-weeks / 12-months toggle.
    fn update_stats_chart(&self, sessions: &[TrainingSession]) {
        let (bars, title) = self.chart_data(sessions);
        self.chart_title.set_label(&title);
        self.chart.set_data(bars);

        // Totals row
        let mut child = self.stats_box.first_child();
        while let Some(c) = child {
            child = c.next_sibling();
            self.stats_box.remove(&c);
        }

        let total_secs: i64 = sessions.iter().map(|s| s.total_actual_seconds).sum();
        let total_h = total_secs / 3600;
        let total_m = (total_secs % 3600) / 60;
        let total_str = if total_h > 0 {
            format!("{}h {}m", total_h, total_m)
        } else {
            format!("{}m", total_m)
        };

        let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        for s in sessions {
            *counts.entry(s.plan_name.as_str()).or_insert(0) += 1;
        }
        let favorite = counts
            .iter()
            .max_by_key(|(_, c)| **c)
            .map(|(n, _)| n.to_string())
            .unwrap_or_else(|| "-".to_string());

        let cards = [
            (sessions.len().to_string(), "Sessions"),
            (total_str, "Total Time"),
            (favorite, "Most Trained"),
        ];
        for (value, caption) in cards {
            let card = GtkBox::new(Orientation::Vertical, 2);
            card.add_css_class("card");
            card.add_css_class("stat-card");
            let value_label = Label::new(Some(&value));
            value_label.add_css_class("stat-value");
            value_label.set_halign(Align::Center);
            value_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            let caption_label = Label::new(Some(caption));
            caption_label.add_css_class("stat-caption");
            caption_label.add_css_class("dim-label");
            caption_label.set_halign(Align::Center);
            card.append(&value_label);
            card.append(&caption_label);
            self.stats_box.append(&card);
        }
    }

    fn refresh_list_focus(&self) {
        let mut rows: Vec<gtk4::Widget> = Vec::new();
        let mut idx = 0;
        while let Some(row) = self.session_list_box.row_at_index(idx) {
            if !row.has_css_class("month-header") {
                rows.push(row.upcast());
            }
            idx += 1;
        }
        self.list_focus.borrow_mut().set_widgets(rows);
    }

    fn show_detail(&self, session: &TrainingSession) {
        *self.current_session_id.borrow_mut() = Some(session.id.clone());
        self.detail_page.set_title(&session.plan_name);
        self.detail_header.set_label(&session.plan_name);

        let date_str = session.started_at.format("%Y-%m-%d %H:%M").to_string();
        let finished_str = session
            .finished_at
            .map(|d| d.format("%H:%M").to_string())
            .unwrap_or_else(|| "?".to_string());
        let planned_min = session.total_planned_seconds / 60;
        let planned_sec = session.total_planned_seconds % 60;
        let actual_min = session.total_actual_seconds / 60;
        let actual_sec = session.total_actual_seconds % 60;
        self.detail_info.set_label(&format!(
            "{} - {} | Actual: {:02}:{:02} / Planned: {:02}:{:02}",
            date_str, finished_str, actual_min, actual_sec, planned_min, planned_sec
        ));

        // Clear exercises box
        let mut child = self.detail_exercises_box.first_child();
        while let Some(c) = child {
            child = c.next_sibling();
            self.detail_exercises_box.remove(&c);
        }

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

        self.detail_exercises_box.append(&group);
        self.nav.push(&self.detail_page);
        self.detail_focus.borrow_mut().set_widgets(vec![
            self.delete_btn.clone().upcast(),
        ]);
        self.detail_focus.borrow_mut().focus_first();
    }

    fn on_export_clicked(&self) {
        let Some(root) = self
            .widget
            .root()
            .and_then(|r| r.dynamic_cast_ref::<gtk4::Window>().cloned())
        else {
            return;
        };
        let store = Rc::clone(&self.store);
        let widget = self.widget.clone();
        crate::dialogs::save_dialog(
            &root,
            "Export Training History",
            "training-history.csv",
            "*.csv",
            "CSV files",
            move |path| {
                if let Some(path) = path {
                    let sessions = store.load_sessions();
                    if let Err(e) = export_history_csv(&sessions, &path) {
                        crate::dialogs::show_error_dialog(
                            "Export failed",
                            &e.to_string(),
                            &widget,
                        );
                    } else {
                        show_app_toast(&format!(
                            "Exported {} sessions",
                            sessions.len()
                        ));
                    }
                }
            },
        );
    }

    fn on_import_clicked(&self) {
        let Some(root) = self
            .widget
            .root()
            .and_then(|r| r.dynamic_cast_ref::<gtk4::Window>().cloned())
        else {
            return;
        };
        let store = Rc::clone(&self.store);
        let page_weak = self.self_weak.borrow().clone();
        crate::dialogs::open_dialog(
            &root,
            "Import Training History",
            Some(("*.csv", "CSV files")),
            move |path| {
                if let Some(path) = path {
                    let (imported, skipped) = import_history_csv(&path);
                    if !imported.is_empty() {
                        let existing_ids: Vec<String> =
                            store.load_sessions().iter().map(|s| s.id.clone()).collect();
                        let new_sessions: Vec<TrainingSession> = imported
                            .into_iter()
                            .filter(|s| !existing_ids.contains(&s.id))
                            .collect();
                        if !new_sessions.is_empty() {
                            let count = new_sessions.len();
                            let mut sessions = store.load_sessions();
                            sessions.extend(new_sessions);
                            store.save_sessions(&sessions);
                            let msg = if skipped > 0 {
                                format!("Imported {} sessions ({} rows skipped)", count, skipped)
                            } else {
                                format!("Imported {} sessions", count)
                            };
                            show_app_toast(&msg);
                        } else {
                            show_app_toast("Nothing new to import");
                        }
                    } else if skipped > 0 {
                        show_app_toast(&format!("Import failed: {} bad rows", skipped));
                    } else {
                        show_app_toast("Import failed: no valid sessions found");
                    }
                }
                if let Some(p) = page_weak.upgrade() {
                    p.refresh();
                }
            },
        );
    }

    pub fn controller_dpad_up(&self) {
        if self.visible_is_detail() {
            self.detail_focus.borrow_mut().cycle(-1);
        } else {
            self.list_focus.borrow_mut().cycle(-1);
        }
    }

    pub fn controller_dpad_down(&self) {
        if self.visible_is_detail() {
            self.detail_focus.borrow_mut().cycle(1);
        } else {
            self.list_focus.borrow_mut().cycle(1);
        }
    }

    fn visible_is_detail(&self) -> bool {
        self.nav
            .visible_page()
            .map(|p| p == self.detail_page)
            .unwrap_or(false)
    }

    pub fn controller_a(&self) {
        if self.visible_is_detail() {
            if let Some(widget) = self.detail_focus.borrow().current_widget() {
                if let Some(btn) = widget.downcast_ref::<ButtonRow>() {
                    btn.activate();
                }
            }
            return;
        }
        if let Some(widget) = self.list_focus.borrow().current_widget() {
            if let Some(row) = widget.downcast_ref::<ActionRow>() {
                let idx = self.session_rows.borrow().iter().position(|r| {
                    r.upcast_ref::<gtk4::Widget>().as_ptr() == row.upcast_ref::<gtk4::Widget>().as_ptr()
                });
                if let Some(idx) = idx {
                    let sessions = self.sessions.borrow();
                    if idx < sessions.len() {
                        let session = sessions[idx].clone();
                        drop(sessions);
                        self.show_detail(&session);
                    }
                }
            }
        }
    }

    pub fn controller_back(&self) {
        if self.visible_is_detail() {
            self.nav.pop_to_page(&self.list_page);
        }
    }

    pub fn update_fonts(&self, _width: i32, _height: i32) {}
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

/// Monday of the ISO week containing `d`.
fn iso_week_start(d: chrono::NaiveDate) -> chrono::NaiveDate {
    let offset = d.weekday().num_days_from_monday() as u64;
    d - chrono::Days::new(offset)
}

fn fmt_float(v: f64) -> String {
    format!("{}", v)
}

fn fmt_exercise_time(ex_log: &crate::models::ExerciseLog) -> String {
    
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
