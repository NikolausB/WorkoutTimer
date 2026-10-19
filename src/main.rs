mod ai_coach;
mod analytics;
mod backup;
mod bar_chart;
mod controller_focus;
mod controller_hints;
mod controller_manager;
mod csv_io;
mod data_store;
mod dialogs;
mod exercise_picker;
mod help_page;
mod history;
mod home;
mod image_utils;
mod llm;
mod models;
mod preferences;
mod progress_ring;
mod round_timer;
mod settings;
mod sound;
mod timer;
mod training_plan;
mod ui_scaling;
mod user_exercises;
mod virtual_keyboard;
mod window;

use gtk4::prelude::*;
use libadwaita as adw;

fn main() {
    // Silence the common libadwaita dark-theme warning on systems that set the
    // legacy GtkSettings key.
    adw::init().expect("Failed to initialize libadwaita");

    let app = adw::Application::builder()
        .application_id(window::APP_ID)
        .build();

    app.connect_activate(|app| {
        let main_window = window::MainWindow::new(app);
        main_window.gtk_window().present();
    });

    // Debug: TRAINING_SHOT_DIR=/path renders every tab to PNG (used for store
    // screenshots; works without a working compositor screencopy).
    if let Ok(dir) = std::env::var("TRAINING_SHOT_DIR") {
        gtk4::glib::idle_add_local_once(move || {
            if let Some(mw) = window::current_window() {
                mw.screenshot_tabs(&dir);
            }
        });
    }

    app.run();
}


#[cfg(test)]
mod ui_tests {
    use gtk4::prelude::*;
    use std::rc::Rc;

    /// GTK can only be initialized once per process, so all widget smoke
    /// checks live in a single test: construct every page and exercise the
    /// main refresh paths.
    #[test]
    fn ui_smoke_all() {
        gtk4::init().expect("gtk init");
        let store = Rc::new(crate::data_store::DataStore::new());

        let home = crate::home::HomePage::new(store.clone(), Rc::new(|_| {}));
        home.refresh();

        let timer = crate::round_timer::RoundTimerPage::new(Rc::new(crate::sound::SoundPlayer::new()));
        timer.update_fonts(800, 600);

        let plans =
            crate::training_plan::TrainingPlanPage::new(store.clone(), Rc::new(crate::sound::SoundPlayer::new()));
        plans.refresh_plans();
        if let Some(plan) = store.load_plans().into_iter().next() {
            plans.start_training(&plan);
        }
        plans.update_fonts(800, 600);

        // Let the runner's timer tick so the ring/progress update path runs.
        let ctx = gtk4::glib::MainContext::default();
        for _ in 0..30 {
            while ctx.iteration(false) {}
            std::thread::sleep(std::time::Duration::from_millis(40));
        }

        let history = crate::history::HistoryPage::new(store);
        history.refresh();
        history.refresh();

        // Node-extraction chain used by the screenshot debug mode:
        // gtk_snapshot_to_node must return a node for content snapshots.
        {
            use gtk4::glib::translate::{from_glib_full, ToGlibPtr};
            let snap = gtk4::Snapshot::new();
            snap.append_color(
                &gtk4::gdk::RGBA::RED,
                &gtk4::graphene::Rect::new(0.0, 0.0, 10.0, 10.0),
            );
            snap.append_color(
                &gtk4::gdk::RGBA::BLUE,
                &gtk4::graphene::Rect::new(0.0, 0.0, 10.0, 10.0),
            );
            let node2 = unsafe {
                let p: *mut gtk4::ffi::GtkSnapshot = snap.to_glib_none().0;
                gtk4::ffi::gtk_snapshot_to_node(p)
            };
            assert!(!node2.is_null(), "render node extraction failed");
            let _rendered: gtk4::gsk::RenderNode = unsafe { from_glib_full(node2) };
        }

        // TimerCore: pause/resume must keep the remaining time accurate and
        // add_seconds must extend a running timer. (Lives here because
        // timeout_add_local can only run on the thread that owns the
        // default main context.)
        {
            let t = crate::timer::TimerCore::new();
            t.start(60);
            std::thread::sleep(std::time::Duration::from_millis(150));
            t.pause();
            let rem = t.remaining_seconds.get();
            assert!(rem < 60.0 && rem > 58.0, "remaining was {}", rem);
            assert!(!t.is_running.get());
            t.resume();
            assert!(t.is_running.get());
            std::thread::sleep(std::time::Duration::from_millis(150));
            t.pause();
            let rem2 = t.remaining_seconds.get();
            assert!(rem2 < rem && rem2 > 57.0, "remaining was {}", rem2);
            t.stop();

            let t2 = crate::timer::TimerCore::new();
            t2.start(30);
            t2.add_seconds(30);
            assert!((t2.remaining_seconds.get() - 60.0).abs() < 0.01);
            t2.stop();
        }

        // Signal string-argument round-trip: the controller pipeline depends
        // on connect_local receiving the emitted String payload intact.
        {
            let m = crate::controller_manager::ControllerManager::new();
            let received: std::rc::Rc<std::cell::RefCell<Option<String>>> =
                std::rc::Rc::new(std::cell::RefCell::new(None));
            let received_emit = received.clone();
            let type_seen: std::rc::Rc<std::cell::RefCell<String>> =
                std::rc::Rc::new(std::cell::RefCell::new(String::new()));
            let type_emit = type_seen.clone();
            m.connect_local("action-activated", false, move |args| {
                *type_emit.borrow_mut() = args
                    .get(0)
                    .map(|v| v.type_().name().to_string())
                    .unwrap_or_default();
                *received_emit.borrow_mut() = args.get(1).and_then(|v| v.get::<String>().ok());
                None
            });
            m.emit_by_name::<()>("action-activated", &[&String::from("r1")]);
            assert_eq!(received.borrow().as_deref(), Some("r1"),
                "signal arg lost; value type seen: {}", type_seen.borrow());
        }

        // Pango attribute sizing path: the timer countdown label must end up
        // with a scaled font size (in absolute pango units), not the theme
        // default (~14 px), or the countdown is unreadably small.
        {
            let timer = crate::round_timer::RoundTimerPage::new(Rc::new(crate::sound::SoundPlayer::new()));
            timer.update_fonts(800, 600);
            let countdown = timer.countdown_label();
            let attrs = countdown
                .attributes()
                .expect("timer countdown has no pango attributes");
            let it = attrs.iterator();
            let found = it.get(gtk4::pango::AttrType::FontDesc);
            let found_size = found
                .and_then(|a| a.downcast_ref::<gtk4::pango::AttrFontDesc>().map(|d| d.desc()))
                .map(|d| d.size() / gtk4::pango::SCALE);
            assert_eq!(found_size, Some(60), "timer label font size wrong: {:?}", found_size);
        }

        // Help page: the controller hint strings are the single source of
        // truth for both the hint bar and the help page.
        {
            use crate::controller_hints::{hints_for, HINT_CONTEXTS, KEYBOARD_SHORTCUTS};
            assert!(!KEYBOARD_SHORTCUTS.is_empty());
            for (context, _title) in HINT_CONTEXTS {
                assert!(
                    !hints_for(context, "").is_empty(),
                    "no hints for context {}",
                    context
                );
            }
            assert!(!hints_for("runner_exercise", "rest").is_empty());
            assert!(hints_for("does-not-exist", "").is_empty());
        }

        // AdwAlertDialog: closing programmatically must emit ::response with
        // the close response — the controller confirm path relies on it.
        {
            use libadwaita::prelude::*;

            let dlg = libadwaita::AlertDialog::builder().build();
            dlg.add_response("cancel", "Cancel");
            dlg.add_response("end", "End");
            dlg.set_close_response("cancel");
            let got: std::rc::Rc<std::cell::RefCell<Option<String>>> =
                std::rc::Rc::new(std::cell::RefCell::new(None));
            let got2 = got.clone();
            dlg.connect_response(None, move |_d, r| {
                *got2.borrow_mut() = Some(r.to_string());
            });
            let host = gtk4::Window::new();
            dlg.present(Some(&host));
            dlg.close();
            assert_eq!(
                got.borrow().as_deref(),
                Some("cancel"),
                "close() did not emit ::response with close_response"
            );
        }

        // Starter templates ship with the app and must be loadable.
        {
            let templates = crate::image_utils::load_default_plans();
            assert!(
                templates.len() >= 5,
                "expected the bundled starter plans, got {}",
                templates.len()
            );
            assert!(templates.iter().all(|p| !p.exercises.is_empty()));
        }

        // Progression: a copy with more reps is created from a logged session.
        {
            use crate::models::{Exercise, ExerciseLog, TrainingPlan, TrainingSession};
            let plan = TrainingPlan {
                name: "Push".to_string(),
                exercises: vec![Exercise {
                    name: "Pushups".to_string(),
                    reps: Some(10),
                    weight_kg: None,
                    duration_seconds: None,
                    rest_seconds: 30,
                    image_path: None,
                    sets: 1,
                }],
                ..Default::default()
            };
            let session = TrainingSession {
                id: uuid::Uuid::new_v4().to_string(),
                plan_id: plan.id.clone(),
                plan_name: plan.name.clone(),
                started_at: chrono::Local::now(),
                finished_at: Some(chrono::Local::now()),
                total_planned_seconds: 300,
                total_actual_seconds: 300,
                total_rounds: 1,
                rest_between_rounds_seconds: 30,
                exercises: vec![ExerciseLog {
                    exercise_name: "Pushups".to_string(),
                    planned_duration_seconds: None,
                    actual_duration_seconds: None,
                    planned_reps: Some(10),
                    actual_reps: Some(10),
                    planned_weight_kg: None,
                    actual_weight_kg: None,
                    rest_seconds: 30,
                    actual_rest_seconds: None,
                    completed: true,
                    round_number: 1,
                    started_at: None,
                    finished_at: None,
                    set_number: 1,
                }],
            };
            let progressed = crate::analytics::progressed_plan(&plan, &session);
            assert_eq!(progressed.exercises[0].reps, Some(11));
            assert_ne!(progressed.id, plan.id);

            // The stats must find the personal record.
            let stats = crate::analytics::exercise_stats(&[session]);
            assert_eq!(stats.len(), 1);
            assert_eq!(stats[0].best_reps, 10);
        }
    }

    /// Live end-to-end generation through the real page pipeline:
    /// `cargo test -- --ignored ai_coach` (needs Ollama running).
    #[test]
    #[ignore]
    fn ai_coach_live_generate() {
        gtk4::init().expect("gtk init");
        use gtk4::prelude::*;
        use std::time::Duration;

        let store = Rc::new(crate::data_store::DataStore::new());
        let coach = crate::ai_coach::AICoachPage::new(store, ());
        coach.model_row.set_text("glm-5.3-flash:cloud");
        coach
            .user_prompt
            .buffer()
            .set_text("Create a 10 minute pushup workout plan with 5 exercises");

        coach.on_generate();
        assert!(coach.generating.get(), "generation should be running");

        let ctx = gtk4::glib::MainContext::default();
        let mut last_label = String::new();
        for _ in 0..400 {
            while ctx.iteration(false) {}
            std::thread::sleep(Duration::from_millis(50));
            let label = coach.progress_label.label().to_string();
            if label != last_label {
                println!("label: {}", label);
                last_label = label.clone();
            }
            if !coach.generating.get() {
                break;
            }
        }
        let err_visible = coach.error_label.is_visible();
        println!(
            "error visible: {} text: {}",
            err_visible,
            coach.error_label.label()
        );
        assert!(
            !coach.generating.get(),
            "generation must end within 20 s (last label: {})",
            last_label
        );
        if err_visible {
            panic!("generation produced an error: {}", coach.error_label.label());
        }
    }
}
