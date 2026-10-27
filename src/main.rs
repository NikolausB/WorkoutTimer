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
mod onboarding;
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

        // First run: the onboarding wizard sets up starter plans.
        // (skipped in screenshot/debug mode: the modal window would block
        // the main window from being mapped)
        if !crate::settings::load_settings().onboarding_done
            && std::env::var("TRAINING_SHOT_DIR").is_err()
        {
            main_window.open_onboarding();
        }
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

    /// GTK and libadwaita require every widget call to happen on one single
    /// thread, but the cargo test harness runs each test on its own thread.
    /// All GTK test bodies are therefore marshalled to this dedicated
    /// thread; panics are caught and re-raised on the calling test thread
    /// so failures still fail the individual test.
    struct GtkJob {
        func: Box<dyn FnOnce() + 'static>,
        reply: std::sync::mpsc::Sender<Result<(), String>>,
    }
    // The job fully owns its Rc's and other non-Send state: they are
    // created, used and dropped on the GTK thread only, so moving the
    // closure across the channel is sound.
    unsafe impl Send for GtkJob {}

    fn gtk_thread() -> &'static std::sync::mpsc::Sender<GtkJob> {
        static SENDER: std::sync::OnceLock<std::sync::mpsc::Sender<GtkJob>> =
            std::sync::OnceLock::new();
        SENDER.get_or_init(|| {
            let (tx, rx) = std::sync::mpsc::channel::<GtkJob>();
            std::thread::Builder::new()
                .name("gtk-test".to_string())
                .spawn(move || {
                    gtk4::init().expect("gtk init");
                    libadwaita::init().expect("adw init");
                    while let Ok(job) = rx.recv() {
                        let GtkJob { func, reply } = job;
                        let result = std::panic::catch_unwind(
                            std::panic::AssertUnwindSafe(move || func()),
                        )
                        .map_err(|payload| {
                            if let Some(s) = payload.downcast_ref::<&str>() {
                                (*s).to_string()
                            } else if let Some(s) = payload.downcast_ref::<String>() {
                                s.clone()
                            } else {
                                "gtk test thread panicked".to_string()
                            }
                        });
                        let _ = reply.send(result);
                    }
                })
                .expect("spawn gtk test thread");
            tx
        })
    }

    fn run_in_gtk<F: FnOnce() + 'static>(body: F) {
        let (reply_tx, reply_rx) = std::sync::mpsc::channel();
        gtk_thread()
            .send(GtkJob {
                func: Box::new(body),
                reply: reply_tx,
            })
            .expect("gtk test thread gone");
        match reply_rx.recv() {
            Ok(Ok(())) => {}
            Ok(Err(msg)) => panic!("{}", msg),
            Err(_) => panic!("gtk test thread died"),
        }
    }

    /// Isolated data dir: tests never touch the real user data. Only
    /// meaningful inside `run_in_gtk` bodies, which run one at a time.
    fn fresh_data_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "workout-timer-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).expect("create test data dir");
        std::env::set_var("TRAINING_DATA_DIR", &dir);
        dir
    }

    fn pump_main_loop(iterations: usize) {
        let ctx = gtk4::glib::MainContext::default();
        for _ in 0..iterations {
            while ctx.iteration(false) {}
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    fn test_plan() -> crate::models::TrainingPlan {
        use crate::models::{Exercise, TrainingPlan};
        TrainingPlan {
            name: "Controller Test Plan".to_string(),
            total_rounds: 1,
            rest_between_rounds_seconds: 30,
            exercises: vec![
                Exercise {
                    name: "Timed Hold".to_string(),
                    reps: None,
                    weight_kg: None,
                    duration_seconds: Some(3),
                    rest_seconds: 30,
                    image_path: None,
                    sets: 1,
                },
                Exercise {
                    name: "Pushups".to_string(),
                    reps: Some(10),
                    weight_kg: None,
                    duration_seconds: None,
                    rest_seconds: 30,
                    image_path: None,
                    sets: 1,
                },
            ],
            ..Default::default()
        }
    }

    /// GTK can only be initialized once per process, so all widget smoke
    /// checks live in a single test: construct every page and exercise the
    /// main refresh paths.
    #[test]
    fn ui_smoke_all() {
        run_in_gtk(|| {
        let _data_dir = fresh_data_dir();
        let store = Rc::new(crate::data_store::DataStore::new());

        let home = crate::home::HomePage::new(store.clone(), Rc::new(|_| {}));
        home.refresh();

        let timer = crate::round_timer::RoundTimerPage::new(Rc::new(crate::sound::SoundPlayer::new()));
        timer.update_fonts(800, 600);

        let plans =
            crate::training_plan::TrainingPlanPage::new(store.clone(), Rc::new(crate::sound::SoundPlayer::new()));
        store.save_plan(&test_plan());
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

        let history = crate::history::HistoryPage::new(Rc::clone(&store));
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
        // default (~14 px), or the countdown is unreadably small. The size
        // follows the ring, which adapts to the window height.
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
            let size = found_size.expect("no font size on the countdown label");
            assert!(
                (40..=90).contains(&size),
                "timer label font size out of range: {}",
                size
            );
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

        // Onboarding: the recommended plans must match the chosen goal.
        {
            let strength = crate::onboarding::recommend_plans(1, 0);
            let cardio = crate::onboarding::recommend_plans(2, 3);
            assert!(!strength.is_empty(), "no plans recommended for strength");
            assert!(!cardio.is_empty(), "no plans recommended for cardio");
            assert!(
                strength.iter().any(|p| p.name.contains("Strength")),
                "strength goal should recommend a strength plan"
            );

            // The wizard window must build without a parent window.
            let parent = libadwaita::Window::new();
            let _wizard =
                crate::onboarding::OnboardingWindow::new(&parent, Rc::clone(&store));
        }

        // History filter: plan + period.
        {
            use crate::history::{apply_filter, HistoryFilter};
            use crate::models::TrainingSession;
            let now = chrono::Local::now();
            let mk = |name: &str, days_ago: i64| TrainingSession {
                id: uuid::Uuid::new_v4().to_string(),
                plan_id: name.to_string(),
                plan_name: name.to_string(),
                started_at: now - chrono::Duration::days(days_ago),
                finished_at: Some(now),
                total_planned_seconds: 600,
                total_actual_seconds: 600,
                total_rounds: 1,
                rest_between_rounds_seconds: 60,
                exercises: vec![],
            };
            let sessions = vec![mk("Push", 1), mk("Push", 40), mk("Pull", 2)];

            let all = HistoryFilter::default();
            assert_eq!(apply_filter(&sessions, &all).len(), 3);

            let by_plan = HistoryFilter {
                plan: "Push".to_string(),
                days: 0,
            };
            assert_eq!(apply_filter(&sessions, &by_plan).len(), 2);

            let recent = HistoryFilter {
                plan: String::new(),
                days: 7,
            };
            assert_eq!(apply_filter(&sessions, &recent).len(), 2);
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
        })
    }

    /// Controller regression test: editor exercise sub-rows, the round timer
    /// focus list and the runner quit flow must not crash.
    #[test]
    fn controller_flows() {
        run_in_gtk(|| {
        let _data_dir = fresh_data_dir();
        use libadwaita::prelude::*;

        let store = Rc::new(crate::data_store::DataStore::new());
        let plan = test_plan();
        store.save_plan(&plan);

        let plans =
            crate::training_plan::TrainingPlanPage::new(store.clone(), Rc::new(crate::sound::SoundPlayer::new()));
        plans.refresh_plans();

        // Host the page in a real window so dialogs can present.
        let host = gtk4::Window::new();
        host.set_child(Some(plans.widget()));
        host.present();

        // --- Editor: open, expand the exercise sub-rows, adjust, collapse ---
        plans.show_editor(Some(&plan));
        assert_eq!(plans.visible_page(), "editor");
        pump_main_loop(3);
        // Walk the whole focus list so every widget is focused once.
        for _ in 0..40 {
            plans.controller_dpad_down();
        }
        plans.controller_x(); // collapse a focused expander
        for _ in 0..40 {
            plans.controller_dpad_up();
        }
        pump_main_loop(3);

        // --- Runner: start, then quit via B + confirm dialog ---
        plans.controller_b(); // editor -> back to list
        assert_eq!(plans.visible_page(), "list");
        plans.start_training(&plan);
        assert_eq!(plans.visible_page(), "runner");
        pump_main_loop(2);

        let captured: Rc<std::cell::RefCell<Option<(Rc<libadwaita::AlertDialog>, String)>>> =
            Rc::new(std::cell::RefCell::new(None));
        let captured_clone = captured.clone();
        plans.set_dialog_registrar(Rc::new(move |dlg| {
            *captured_clone.borrow_mut() = dlg;
        }));

        plans.controller_b(); // runner -> "End Training?" dialog
        pump_main_loop(2);
        let (dlg, confirm_id) = captured.borrow().clone().expect("quit dialog was not registered");
        assert_eq!(confirm_id, "end");

        // Confirm, exactly like the window's controller dialog handling.
        dlg.set_close_response(&confirm_id);
        dlg.close();
        pump_main_loop(5);

        assert_eq!(
            plans.visible_page(),
            "summary",
            "confirming End Training must land on the summary page"
        );
        // Back from summary to the list must work too.
        plans.controller_b();
        assert_eq!(plans.visible_page(), "list");
        pump_main_loop(3);

        // --- Runner: reps exercise, focus over every widget, quit at rest ---
        let reps_plan = crate::models::TrainingPlan {
            name: "Reps Plan".to_string(),
            exercises: vec![crate::models::Exercise {
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
        store.save_plan(&reps_plan);
        plans.refresh_plans();
        plans.start_training(&reps_plan);
        pump_main_loop(2);
        // Cycle through every runner widget, incl. the reps spin row.
        for _ in 0..10 {
            plans.controller_dpad_down();
            plans.controller_dpad_right();
            plans.controller_dpad_left();
        }
        plans.controller_a(); // mark the exercise done -> rest phase
        pump_main_loop(3);
        assert_eq!(plans.visible_page(), "runner");

        plans.controller_b(); // quit dialog
        pump_main_loop(2);
        let (dlg, confirm_id) = captured.borrow().clone().expect("second quit dialog");
        dlg.set_close_response(&confirm_id);
        dlg.close();
        pump_main_loop(5);
        assert_eq!(plans.visible_page(), "summary");
        plans.controller_b();

        host.close();
        })
    }

    /// The round timer's gamepad focus list must contain every config row
    /// at idle, A must activate the focused widget (open the presets
    /// dialog) instead of starting the timer, and the presets dialog
    /// itself must be navigable and close when a preset is applied.
    #[test]
    fn round_timer_controller_access() {
        run_in_gtk(|| {
        use std::cell::RefCell;
        let _data_dir = fresh_data_dir();

        let timer = crate::round_timer::RoundTimerPage::new(Rc::new(
            crate::sound::SoundPlayer::new(),
        ));

        // The presets dialog is presented on the page's root window and
        // only reports "closed" once it was presented.
        let win = gtk4::Window::new();
        win.set_child(Some(timer.widget()));
        win.present();
        pump_main_loop(2);

        // Capture the presets-dialog registration like MainWindow does.
        let registered: Rc<RefCell<Option<Rc<crate::round_timer::PresetsDialogController>>>> =
            Rc::new(RefCell::new(None));
        {
            let registered = Rc::clone(&registered);
            timer.set_presets_dialog_registrar(Rc::new(move |c| {
                *registered.borrow_mut() = c;
            }));
        }

        // Regression: the config rows (rounds / round time / pause /
        // prepare spins, warm-up + custom switches, activity entry) must
        // be reachable by gamepad at idle — they used to enter the focus
        // list only after the timer had run once.
        let widgets = timer.focus_widgets();
        let spin_count = widgets
            .iter()
            .filter(|w| w.is::<libadwaita::SpinRow>())
            .count();
        let switch_count = widgets
            .iter()
            .filter(|w| w.is::<libadwaita::SwitchRow>())
            .count();
        assert!(
            spin_count >= 4,
            "all four timer spin rows must be gamepad-reachable at idle, found {spin_count}"
        );
        assert!(
            switch_count >= 2,
            "warm-up and custom-rounds switches must be gamepad-reachable, found {switch_count}"
        );
        assert!(
            widgets
                .iter()
                .any(|w| w.is::<libadwaita::EntryRow>()),
            "the activity entry row must be gamepad-reachable"
        );

        // Cycle to the presets button and press A: it must open the
        // presets dialog, not start the timer.
        let presets_idx = widgets
            .iter()
            .position(|w| {
                w.downcast_ref::<gtk4::Button>()
                    .and_then(|b| b.label())
                    .map(|l| l.starts_with("Presets"))
                    .unwrap_or(false)
            })
            .expect("the presets button must be in the focus list");
        for _ in 0..=presets_idx {
            timer.controller_dpad_down();
        }
        timer.controller_a();
        pump_main_loop(2);

        let controller = registered
            .borrow()
            .clone()
            .expect("A on the presets button must open the presets dialog");

        // A must not have started the timer: while running, the config
        // spins are insensitive and drop out of the focus list.
        let spins_now = timer
            .focus_widgets()
            .iter()
            .filter(|w| w.is::<libadwaita::SpinRow>())
            .count();
        assert!(
            spins_now >= 4,
            "A must not start the timer (Start button does), config spins still reachable: {spins_now}"
        );

        // The dialog tracks its rows and buttons for dpad navigation.
        let dialog_widgets = controller.tracker.borrow().widgets();
        assert!(
            dialog_widgets.len() >= 3,
            "the presets dialog must track its rows and buttons"
        );
        assert!(
            dialog_widgets
                .iter()
                .any(|w| w.is::<libadwaita::ActionRow>()),
            "preset rows must be gamepad-focusable in the dialog"
        );

        // Activating a preset row applies it and closes the dialog, which
        // unregisters the controller.
        controller.tracker.borrow_mut().focus_first();
        controller.activate_focused();
        pump_main_loop(5);
        assert!(
            registered.borrow().is_none(),
            "applying a preset must close the dialog and unregister it"
        );

        win.close();
        })
    }

    /// Full window-level controller routing: open a plan, start the
    /// training, quit via B + confirm via A (the exact gamepad path).
    #[test]
    fn controller_window_routing() {
        run_in_gtk(|| {
        let data_dir = fresh_data_dir();

        // Seed the store before the window is built.
        {
            std::fs::create_dir_all(&data_dir).unwrap();
            let store = crate::data_store::DataStore::new();
            store.save_plan(&test_plan());
        }

        let app = libadwaita::Application::builder().build();
        let mw = crate::window::MainWindow::new(&app);
        mw.gtk_window().present();
        pump_main_loop(3);

        // Switch to the Training Plans tab via L1/R1 like a gamepad would.
        for _ in 0..8 {
            if mw.visible_tab() == "plans" {
                break;
            }
            mw.on_controller_action("r1");
            pump_main_loop(1);
        }
        assert_eq!(mw.visible_tab(), "plans");

        // Focus and open the first plan row with A.
        mw.on_controller_action("dpad_down");
        mw.on_controller_action("a");
        pump_main_loop(2);
        assert_eq!(
            mw.training_plan.visible_page(),
            "editor",
            "A on a plan row must open the editor"
        );

        // Start the training with the Start button.
        mw.on_controller_action("start");
        pump_main_loop(2);
        assert_eq!(mw.training_plan.visible_page(), "runner");

        // Quit: B opens the confirmation dialog...
        mw.on_controller_action("b");
        pump_main_loop(2);
        // ...A confirms it through the window dialog routing.
        mw.on_controller_action("a");
        pump_main_loop(5);
        assert_eq!(
            mw.training_plan.visible_page(),
            "summary",
            "confirming End Training must land on the summary page"
        );

        // B on the summary goes back to the plan list.
        mw.on_controller_action("b");
        pump_main_loop(3);
        assert_eq!(mw.training_plan.visible_page(), "list");
        })
    }

    /// Regression for the "gamepad freezes after Browse Exercises" bug:
    /// the picker dialog must unregister itself when it closes (instead
    /// of swallowing every controller action forever), and the
    /// add-exercise form must take over routing and hand it back.
    #[test]
    fn picker_dialog_controller_flow() {
        run_in_gtk(|| {
        use std::cell::RefCell;
        let _data_dir = fresh_data_dir();

        let app = libadwaita::Application::builder().build();
        let mw = crate::window::MainWindow::new(&app);
        mw.gtk_window().present();
        pump_main_loop(3);

        let selected: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
        let selected_cb = Rc::clone(&selected);
        let picker = crate::exercise_picker::ExercisePicker::new(move |name, _| {
            *selected_cb.borrow_mut() = Some(name.to_string());
        });
        picker.present_over_window();
        pump_main_loop(2);
        assert!(
            mw.has_open_dialog(),
            "presenting the picker must register it for controller input"
        );

        // The add-exercise form takes over routing while it is open...
        picker.show_add_exercise_form();
        pump_main_loop(2);
        assert!(
            mw.has_open_dialog(),
            "the add-exercise form must be registered while open"
        );

        // ...and B (through the window routing) closes it and hands
        // routing back to the still-open picker list.
        mw.on_controller_action("b");
        pump_main_loop(3);
        assert!(
            mw.has_open_dialog(),
            "after the form closes, the picker list must be re-registered"
        );

        // Selecting an exercise with dpad + A must close the picker AND
        // unregister it — the old code left the dead dialog registered,
        // freezing all gamepad input.
        mw.on_controller_action("dpad_down");
        mw.on_controller_action("a");
        pump_main_loop(3);
        assert!(
            !mw.has_open_dialog(),
            "closing the picker must unregister it (gamepad freeze regression)"
        );
        assert!(
            selected.borrow().is_some(),
            "A on a picker row must select the exercise"
        );
        })
    }

    /// Gamepad coverage for the presets dialog: star/delete buttons must
    /// be in the focus list (not mouse-only), and "Save current setup"
    /// must produce a gamepad-drivable save dialog that really saves.
    #[test]
    fn presets_dialog_gamepad_reach() {
        run_in_gtk(|| {
        use std::cell::RefCell;
        let _data_dir = fresh_data_dir();

        let timer = crate::round_timer::RoundTimerPage::new(Rc::new(
            crate::sound::SoundPlayer::new(),
        ));
        let win = gtk4::Window::new();
        win.set_child(Some(timer.widget()));
        win.present();
        pump_main_loop(2);

        let registered: Rc<RefCell<Option<Rc<crate::round_timer::PresetsDialogController>>>> =
            Rc::new(RefCell::new(None));
        {
            let registered = Rc::clone(&registered);
            timer.set_presets_dialog_registrar(Rc::new(move |c| {
                *registered.borrow_mut() = c;
            }));
        }

        // Open the presets dialog via the gamepad path (dpad to the
        // presets button, A to activate).
        let widgets = timer.focus_widgets();
        let presets_idx = widgets
            .iter()
            .position(|w| {
                w.downcast_ref::<gtk4::Button>()
                    .and_then(|b| b.label())
                    .map(|l| l.starts_with("Presets"))
                    .unwrap_or(false)
            })
            .expect("the presets button must be in the focus list");
        for _ in 0..=presets_idx {
            timer.controller_dpad_down();
        }
        timer.controller_a();
        pump_main_loop(2);

        let controller = registered
            .borrow()
            .clone()
            .expect("the presets dialog must be registered while open");

        // Every builtin preset row must bring its star button into the
        // focus list — starring used to be mouse-only.
        let dialog_widgets = controller.tracker.borrow().widgets();
        let rows = dialog_widgets
            .iter()
            .filter(|w| w.is::<libadwaita::ActionRow>())
            .count();
        let stars = dialog_widgets
            .iter()
            .filter(|w| {
                w.downcast_ref::<gtk4::Button>()
                    .and_then(|b| b.icon_name())
                    .map(|i| {
                        i.as_str() == "non-starred-symbolic"
                            || i.as_str() == "starred-symbolic"
                    })
                    .unwrap_or(false)
            })
            .count();
        assert!(rows >= 3, "builtin presets must be listed, found {rows}");
        assert_eq!(
            stars, rows,
            "each preset row must have its star button in the focus list"
        );

        // Star the first builtin preset with the gamepad: row is focused
        // first, one dpad_down lands on its star.
        controller.tracker.borrow_mut().focus_first();
        controller.tracker.borrow_mut().cycle(1);
        controller.activate_focused();
        pump_main_loop(3);
        assert!(
            registered.borrow().is_none(),
            "starring must close the dialog and unregister it"
        );
        assert!(
            !crate::settings::load_settings().favorite_preset.is_empty(),
            "A on the star button must favorite the preset"
        );

        // Reopen (the presets button still has focus on the page) and
        // drive the "Save current setup" flow entirely by gamepad.
        timer.controller_a();
        pump_main_loop(2);
        let controller = registered
            .borrow()
            .clone()
            .expect("the presets dialog must re-register on reopen");
        {
            let widgets = controller.tracker.borrow().widgets();
            let add_btn = widgets
                .iter()
                .find(|w| {
                    w.downcast_ref::<gtk4::Button>()
                        .and_then(|b| b.label())
                        .map(|l| l.contains("Save current setup"))
                        .unwrap_or(false)
                })
                .expect("the save-current-setup button must be focusable")
                .clone();
            controller.tracker.borrow_mut().move_to_widget(&add_btn);
            controller.activate_focused();
        }
        pump_main_loop(3);
        let save_controller = registered
            .borrow()
            .clone()
            .expect("the save-preset dialog must be registered while open");
        {
            let widgets = save_controller.tracker.borrow().widgets();
            let entry = widgets
                .iter()
                .find(|w| w.is::<libadwaita::EntryRow>())
                .expect("the name entry must be in the focus list")
                .clone();
            let save_btn = widgets
                .iter()
                .find(|w| {
                    w.downcast_ref::<gtk4::Button>()
                        .and_then(|b| b.label())
                        .map(|l| l == "Save")
                        .unwrap_or(false)
                })
                .expect("the Save button must be in the focus list")
                .clone();
            entry.set_property("text", "CI Preset");
            save_controller.tracker.borrow_mut().move_to_widget(&save_btn);
            save_controller.activate_focused();
        }
        pump_main_loop(3);
        assert!(
            registered.borrow().is_none(),
            "saving must close the save dialog and unregister it"
        );
        assert!(
            crate::settings::load_settings()
                .timer_presets
                .iter()
                .any(|p| p.name == "CI Preset"),
            "the gamepad-driven save flow must persist the preset"
        );

        // The saved custom preset must expose a delete button in the
        // focus list (deleting used to be mouse-only).
        timer.controller_a();
        pump_main_loop(2);
        let controller = registered
            .borrow()
            .clone()
            .expect("the presets dialog must re-register on reopen");
        {
            let widgets = controller.tracker.borrow().widgets();
            let delete = widgets
                .iter()
                .find(|w| {
                    w.downcast_ref::<gtk4::Button>()
                        .and_then(|b| b.icon_name())
                        .map(|i| i.as_str() == "user-trash-symbolic")
                        .unwrap_or(false)
                })
                .expect("the custom preset's delete button must be focusable")
                .clone();
            controller.tracker.borrow_mut().move_to_widget(&delete);
            controller.activate_focused();
        }
        pump_main_loop(3);
        assert!(
            !crate::settings::load_settings()
                .timer_presets
                .iter()
                .any(|p| p.name == "CI Preset"),
            "A on the delete button must remove the custom preset"
        );

        win.close();
        })
    }

    /// A in the runner activates the focused button: Pause pauses, and
    /// with the Skip button focused, A skips (regression for the
    /// Steam Deck bug where A always meant Done/Skip regardless of
    /// focus).
    #[test]
    fn runner_a_activates_focused_button() {
        run_in_gtk(|| {
        let data_dir = fresh_data_dir();
        {
            std::fs::create_dir_all(&data_dir).unwrap();
            let store = crate::data_store::DataStore::new();
            store.save_plan(&test_plan());
        }

        let app = libadwaita::Application::builder().build();
        let mw = crate::window::MainWindow::new(&app);
        mw.gtk_window().present();
        pump_main_loop(3);

        for _ in 0..8 {
            if mw.visible_tab() == "plans" {
                break;
            }
            mw.on_controller_action("r1");
            pump_main_loop(1);
        }
        mw.on_controller_action("dpad_down");
        mw.on_controller_action("a");
        pump_main_loop(2);
        mw.on_controller_action("start");
        pump_main_loop(2);
        assert_eq!(mw.training_plan.visible_page(), "runner");

        // Default focus is the Pause button: A must pause, not skip.
        assert_eq!(mw.training_plan.runner_pause_label(), "Pause");
        // While the timer runs, the +30s button must be in the gamepad
        // focus list (regression for the "+30s unreachable with the
        // controller" bug).
        assert!(
            mw.training_plan
                .runner_focus_widgets()
                .iter()
                .any(|w| w.downcast_ref::<gtk4::Button>()
                    .and_then(|b| b.label())
                    .map(|l| l == "+30s")
                    .unwrap_or(false)),
            "the +30s button must be reachable while the workout runs"
        );
        mw.on_controller_action("a");
        pump_main_loop(1);
        assert_eq!(
            mw.training_plan.runner_pause_label(),
            "Resume",
            "A with the Pause button focused must pause the workout"
        );
        mw.on_controller_action("a");
        pump_main_loop(1);
        assert_eq!(
            mw.training_plan.runner_pause_label(),
            "Pause",
            "A again must resume the workout"
        );

        // Two dpad_downs move focus to Skip (pause -> +30s -> skip;
        // +30s is sensitive while the timer runs and reachable on the
        // gamepad). A then skips: exercise, its rest, the next
        // exercise and its rest — after that the workout is over and
        // the summary shows. This is exactly the "A skips what I
        // selected" behavior.
        mw.on_controller_action("dpad_down");
        mw.on_controller_action("dpad_down");
        for _ in 0..4 {
            mw.on_controller_action("a");
            pump_main_loop(2);
        }
        assert_eq!(
            mw.training_plan.visible_page(),
            "summary",
            "A with the Skip button focused must skip each exercise and rest"
        );
        })
    }

    /// Multiple round-timer presets can be favorited at once, and every
    /// favorite pill joins the gamepad focus list (regression for the
    /// "only one favorite, unreachable with the controller" bugs).
    #[test]
    fn multiple_favorite_presets_pills() {
        run_in_gtk(|| {
        use std::cell::RefCell;
        let _data_dir = fresh_data_dir();

        let timer = crate::round_timer::RoundTimerPage::new(Rc::new(
            crate::sound::SoundPlayer::new(),
        ));
        let win = gtk4::Window::new();
        win.set_child(Some(timer.widget()));
        win.present();
        pump_main_loop(2);

        let registered: Rc<RefCell<Option<Rc<crate::round_timer::PresetsDialogController>>>> =
            Rc::new(RefCell::new(None));
        {
            let registered = Rc::clone(&registered);
            timer.set_presets_dialog_registrar(Rc::new(move |c| {
                *registered.borrow_mut() = c;
            }));
        }

        // Star two different builtin presets through the gamepad path.
        for round in 0..2 {
            if round == 0 {
                let widgets = timer.focus_widgets();
                let presets_idx = widgets
                    .iter()
                    .position(|w| {
                        w.downcast_ref::<gtk4::Button>()
                            .and_then(|b| b.label())
                            .map(|l| l.starts_with("Presets"))
                            .unwrap_or(false)
                    })
                    .expect("the presets button must be in the focus list");
                for _ in 0..=presets_idx {
                    timer.controller_dpad_down();
                }
            }
            // On the second round the presets button still has focus.
            timer.controller_a();
            pump_main_loop(2);
            let controller = registered
                .borrow()
                .clone()
                .expect("the presets dialog must be registered while open");

            let unstarred: Vec<gtk4::Widget> = {
                let widgets = controller.tracker.borrow().widgets();
                widgets
                    .iter()
                    .filter(|w| {
                        w.downcast_ref::<gtk4::Button>()
                            .and_then(|b| b.icon_name())
                            .map(|i| i.as_str() == "non-starred-symbolic")
                            .unwrap_or(false)
                    })
                    .cloned()
                    .collect()
            };
            let star = unstarred
                .get(round)
                .expect("there must be another unstarred builtin preset")
                .clone();
            controller.tracker.borrow_mut().move_to_widget(&star);
            controller.activate_focused();
            pump_main_loop(3);
            assert!(
                registered.borrow().is_none(),
                "starring must close the dialog and unregister it"
            );
        }

        let settings = crate::settings::load_settings();
        assert_eq!(
            settings.favorite_presets.len(),
            2,
            "two presets must be favoritable at the same time"
        );
        assert_eq!(
            settings.favorite_preset, settings.favorite_presets[0],
            "the legacy single-favorite field must stay in sync"
        );

        // Both pills must be reachable with the controller.
        let widgets = timer.focus_widgets();
        let pills = widgets
            .iter()
            .filter(|w| {
                w.downcast_ref::<gtk4::Button>()
                    .and_then(|b| b.label())
                    .map(|l| settings.favorite_presets.contains(&l.to_string()))
                    .unwrap_or(false)
            })
            .count();
        assert_eq!(
            pills, 2,
            "both favorite pills must be in the gamepad focus list"
        );

        win.close();
        })
    }

    /// Plans-list gamepad shortcuts: X favorites (and the list resorts
    /// favorites-first with focus kept), Y duplicates, R2 progresses a
    /// plan that has a completed session (regression for the
    /// "favorites/duplicate/progress unreachable with the controller"
    /// bugs).
    #[test]
    fn plans_list_gamepad_shortcuts() {
        run_in_gtk(|| {
        let data_dir = fresh_data_dir();
        let plan_id = {
            std::fs::create_dir_all(&data_dir).unwrap();
            let store = crate::data_store::DataStore::new();
            let plan = test_plan();
            let mut other = test_plan();
            other.name = "Aardvark Plan".to_string();
            store.save_plan(&plan);
            store.save_plan(&other);
            let session = crate::models::TrainingSession {
                plan_id: plan.id.clone(),
                exercises: vec![crate::models::ExerciseLog {
                    exercise_name: "Pushups".to_string(),
                    planned_reps: Some(10),
                    actual_reps: Some(10),
                    completed: true,
                    ..Default::default()
                }],
                ..Default::default()
            };
            store.save_session(&session);
            plan.id
        };

        let app = libadwaita::Application::builder().build();
        let mw = crate::window::MainWindow::new(&app);
        mw.gtk_window().present();
        pump_main_loop(3);

        for _ in 0..8 {
            if mw.visible_tab() == "plans" {
                break;
            }
            mw.on_controller_action("r1");
            pump_main_loop(1);
        }
        assert_eq!(mw.visible_tab(), "plans");

        let tp = mw.training_plan.clone();
        assert_eq!(tp.plans_order().len(), 2);
        assert_eq!(
            tp.plans_row_count(),
            2,
            "the freshly loaded list must not carry stale rows"
        );

        // Unfavorited list is alphabetical: Aardvark Plan first, the
        // test plan second.
        mw.on_controller_action("dpad_down");
        mw.on_controller_action("dpad_down");
        pump_main_loop(1);

        // X favorites the focused plan: it must jump to the top of the
        // list, and focus must stay on it.
        mw.on_controller_action("x");
        pump_main_loop(2);
        assert_eq!(
            tp.plans_row_count(),
            2,
            "refreshing after a favorite must not leave stale duplicate rows"
        );
        {
            let plans = crate::data_store::DataStore::new().load_plans();
            let fav = plans
                .iter()
                .find(|p| p.id == plan_id)
                .expect("the test plan must still exist");
            assert!(fav.is_favorite, "X must favorite the focused plan");
        }
        assert_eq!(
            tp.plans_order().first().map(|s| s.to_string()),
            Some(plan_id.clone()),
            "the favorited plan must sort to the top of the list"
        );
        assert_eq!(tp.plans_order().len(), 2);

        // X again must toggle the favorite off (the controller toggle
        // used to hit the wrong plan after a refresh left stale rows).
        mw.on_controller_action("x");
        pump_main_loop(2);
        assert_eq!(
            tp.plans_row_count(),
            2,
            "refreshing must not leave stale duplicate rows"
        );
        {
            let plans = crate::data_store::DataStore::new().load_plans();
            let fav = plans
                .iter()
                .find(|p| p.id == plan_id)
                .expect("the test plan must still exist");
            assert!(!fav.is_favorite, "X must unfavorite again");
        }
        // Re-favorite for the rest of the test.
        mw.on_controller_action("x");
        pump_main_loop(2);
        assert_eq!(tp.plans_row_count(), 2);

        // Y duplicates the focused plan.
        mw.on_controller_action("y");
        pump_main_loop(2);
        {
            let plans = crate::data_store::DataStore::new().load_plans();
            assert_eq!(plans.len(), 3, "Y must duplicate the focused plan");
            assert!(
                plans
                    .iter()
                    .any(|p| p.name == "Controller Test Plan (copy)"),
                "the duplicate must be saved"
            );
        }
        assert_eq!(
            tp.plans_row_count(),
            3,
            "refreshing after a duplicate must not leave stale duplicate rows"
        );

        // Focus moved to the copy (an unfavorite); move back up to the
        // favorite and progress it with R2.
        mw.on_controller_action("dpad_up");
        mw.on_controller_action("dpad_up");
        pump_main_loop(1);
        mw.on_controller_action("r2");
        pump_main_loop(2);
        {
            let plans = crate::data_store::DataStore::new().load_plans();
            assert_eq!(plans.len(), 4, "R2 must create a progression plan");
            assert_eq!(
                tp.plans_row_count(),
                4,
                "refreshing after a progression must not leave stale duplicate rows"
            );
            let prog = plans
                .iter()
                .find(|p| p.name == "Controller Test Plan (progression)")
                .expect("R2 must create a \"(progression)\" plan");
            let pushups = prog
                .exercises
                .iter()
                .find(|e| e.name == "Pushups")
                .expect("the progression must keep the exercises");
            assert_eq!(
                pushups.reps,
                Some(11),
                "progression must bump the completed exercise by one rep"
            );
        }

        mw.gtk_window().close();
        })
    }

    /// The favorite switch in the plan editor must persist immediately
    /// when toggled with the gamepad, and pressing Save Plan must keep
    /// the favorite state (regression for the "favorite toggle in the
    /// plan details does not work" bug).
    #[test]
    fn editor_favorite_toggle_gamepad() {
        run_in_gtk(|| {
        let data_dir = fresh_data_dir();
        let plan_id = {
            std::fs::create_dir_all(&data_dir).unwrap();
            let store = crate::data_store::DataStore::new();
            let plan = test_plan();
            let mut other = test_plan();
            other.name = "Aardvark Plan".to_string();
            store.save_plan(&plan);
            store.save_plan(&other);
            plan.id
        };

        let app = libadwaita::Application::builder().build();
        let mw = crate::window::MainWindow::new(&app);
        mw.gtk_window().present();
        pump_main_loop(3);

        for _ in 0..8 {
            if mw.visible_tab() == "plans" {
                break;
            }
            mw.on_controller_action("r1");
            pump_main_loop(1);
        }
        assert_eq!(mw.visible_tab(), "plans");

        // Rows: New Plan, Aardvark Plan, Controller Test Plan — two
        // dpad_downs focus the test plan; A opens it.
        mw.on_controller_action("dpad_down");
        mw.on_controller_action("dpad_down");
        mw.on_controller_action("a");
        pump_main_loop(2);
        assert_eq!(mw.training_plan.visible_page(), "editor");

        // Focus starts on the name entry; one dpad_down lands on the
        // favorite switch. A must toggle it (like on the round timer
        // page; it used to only grab focus and do nothing).
        mw.on_controller_action("dpad_down");
        mw.on_controller_action("a");
        pump_main_loop(2);
        {
            let plans = crate::data_store::DataStore::new().load_plans();
            let plan = plans.iter().find(|p| p.id == plan_id).unwrap();
            assert!(
                plan.is_favorite,
                "A on the favorite switch must persist the favorite"
            );
        }

        // A again toggles the switch back off — and on again, so the
        // rest of the test runs with the favorite set.
        mw.on_controller_action("a");
        pump_main_loop(2);
        {
            let plans = crate::data_store::DataStore::new().load_plans();
            let plan = plans.iter().find(|p| p.id == plan_id).unwrap();
            assert!(!plan.is_favorite, "A must toggle the favorite off again");
        }
        mw.on_controller_action("dpad_right");
        pump_main_loop(2);

        // Walk down to "Save Plan" and press A: the favorite must
        // survive the explicit save.
        // Focus order from the switch: rounds, rest, add, browse,
        // 2 exercise rows, Start Training, Save Plan -> 8 downs.
        for _ in 0..8 {
            mw.on_controller_action("dpad_down");
        }
        mw.on_controller_action("a");
        pump_main_loop(2);
        assert_eq!(
            mw.training_plan.visible_page(),
            "editor",
            "Save Plan must not start a workout"
        );
        {
            let plans = crate::data_store::DataStore::new().load_plans();
            assert_eq!(plans.len(), 2, "Save must not create a duplicate plan");
            let plan = plans.iter().find(|p| p.id == plan_id).unwrap();
            assert!(
                plan.is_favorite,
                "Save Plan must keep the favorite state"
            );
        }

        // Simulate the NavigationView back button (mouse path): it pops
        // the nav without going through show_list — the list must still
        // reload and show the favorited plan at the top.
        assert!(mw.training_plan.nav_pop(), "pop() must accept the editor page");
        pump_main_loop(10);
        assert_eq!(mw.training_plan.visible_page(), "list");
        assert_eq!(
            mw.training_plan.plans_order().first().map(|s| s.to_string()),
            Some(plan_id.clone()),
            "returning to the list via the back button must reload it"
        );
        assert_eq!(mw.training_plan.plans_row_count(), 2);

        mw.gtk_window().close();
        })
    }

    /// L3 opens the Help window and R3 opens Preferences, from any page
    /// (regression for the "help/settings unreachable with the
    /// controller" gap).
    #[test]
    fn l3_help_r3_settings_shortcuts() {
        run_in_gtk(|| {
        let _data_dir = fresh_data_dir();
        let app = libadwaita::Application::builder().build();
        let mw = crate::window::MainWindow::new(&app);
        mw.gtk_window().present();
        pump_main_loop(3);

        assert!(!mw.help_window_open());
        mw.on_controller_action("l3");
        pump_main_loop(2);
        assert!(mw.help_window_open(), "L3 must open the help window");
        // B closes the help window (window-level handling).
        mw.on_controller_action("b");
        pump_main_loop(2);
        assert!(!mw.help_window_open(), "B must close the help window");

        assert!(!mw.preferences_dialog_open());
        mw.on_controller_action("r3");
        pump_main_loop(2);
        assert!(
            mw.preferences_dialog_open(),
            "R3 must open the preferences dialog"
        );

        mw.gtk_window().close();
        })
    }

    /// Every context the help page lists must have hint text, and the
    /// new dialog contexts (picker, form, alert) must be listed.
    #[test]
    fn hint_contexts_complete() {
        for (ctx, _) in crate::controller_hints::HINT_CONTEXTS {
            assert!(
                !crate::controller_hints::hints_for(ctx, "").is_empty(),
                "missing hint text for context {ctx}"
            );
        }
        for ctx in ["picker", "form", "alert", "presets"] {
            assert!(
                crate::controller_hints::HINT_CONTEXTS
                    .iter()
                    .any(|(c, _)| *c == ctx),
                "context {ctx} must be listed in HINT_CONTEXTS"
            );
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
