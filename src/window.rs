use crate::controller_hints::ControllerHintsOverlay;
use crate::controller_manager::ControllerManager;
use crate::data_store::DataStore;
use crate::home::HomePage;
use crate::preferences::{apply_color_scheme, PreferencesDialog};
use crate::round_timer::RoundTimerPage;
use crate::settings::{load_settings, save_settings};
use crate::sound::SoundPlayer;
use gtk4::glib;
use gtk4::glib::prelude::*;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use libadwaita::{HeaderBar, ToolbarView, ViewStack, Window};
use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::time::Duration;

pub const APP_ID: &str = "io.github.NikolausB.WorkoutTimer";

thread_local! {
    // Strong anchor: the MainWindow (and with it every page) must live for the
    // whole application lifetime. Page signal handlers only hold weak
    // references, so if this anchor were weak everything would silently
    // no-op once `connect_activate` returns.
    static GLOBAL_WINDOW: RefCell<Option<Rc<MainWindow>>> = const { RefCell::new(None) };
}

/// Register the main window so helper pages can reach it (used for
/// controller-dialog registration, mirroring Python's `_register_controller_dialog`).
/// Also keeps the window (and all pages) alive for the application lifetime.
pub fn set_global_window(mw: &Rc<MainWindow>) {
    GLOBAL_WINDOW.with(|g| *g.borrow_mut() = Some(Rc::clone(mw)));
}

pub fn current_window() -> Option<Rc<MainWindow>> {
    GLOBAL_WINDOW.with(|g| g.borrow().clone())
}

/// Show a toast on the main window from anywhere (no-op if closed).
pub fn show_app_toast(title: &str) {
    if let Some(mw) = current_window() {
        mw.show_toast(title);
    }
}

/// Show a toast with an action button (used for "Undo") from anywhere.
pub fn show_app_toast_action(title: &str, action: &str, cb: Rc<dyn Fn()>) {
    if let Some(mw) = current_window() {
        mw.show_toast_action(title, action, cb);
    }
}

/// Re-read the data files and refresh every page (used after undo/restore).
pub fn refresh_all_pages() {
    if let Some(mw) = current_window() {
        mw.refresh_all_pages();
    }
}
enum OpenDialog {
    Preferences(Rc<PreferencesDialog>),
    ExercisePicker(Rc<crate::exercise_picker::ExercisePicker>),
    /// libadwaita confirmation AlertDialogs (end training, delete plan).
    /// The String is the response id that should count as "confirm".
    AlertConfirm(Rc<libadwaita::AlertDialog>, String),
}

pub struct MainWindow {
    window: Window,
    controller: RefCell<Option<ControllerManager>>,
    open_dialog: RefCell<Option<OpenDialog>>,
    resize_timeout_id: RefCell<Option<glib::SourceId>>,

    round_timer: Rc<RoundTimerPage>,
    training_plan: Rc<crate::training_plan::TrainingPlanPage>,
    history: Rc<crate::history::HistoryPage>,
    home: Rc<HomePage>,
    ai_coach: Rc<crate::ai_coach::AICoachPage>,

    stack: ViewStack,
    home_tab: adw::ViewStackPage,
    timer_tab: adw::ViewStackPage,
    plans_tab: adw::ViewStackPage,
    ai_tab: adw::ViewStackPage,
    history_tab: adw::ViewStackPage,
    hints_overlay: Rc<RefCell<ControllerHintsOverlay>>,
    toast_overlay: libadwaita::ToastOverlay,
    help_window: RefCell<Option<Rc<crate::help_page::HelpWindow>>>,
    self_weak: RefCell<Weak<MainWindow>>,
}

impl MainWindow {
    pub fn new(app: &adw::Application) -> Rc<Self> {
        let window = Window::builder()
            .application(app)
            .title("Training")
            .default_width(600)
            .default_height(700)
            .build();

        let store = Rc::new(DataStore::new());
        store.seed_default_plans();

        let sound = Rc::new(SoundPlayer::new());

        let round_timer = RoundTimerPage::new(Rc::clone(&sound));
        let training_plan = crate::training_plan::TrainingPlanPage::new(
            Rc::clone(&store),
            Rc::clone(&sound),
        );
        let history = crate::history::HistoryPage::new(Rc::clone(&store));
        let ai_coach = crate::ai_coach::AICoachPage::new(Rc::clone(&store), ());
        let home = HomePage::new(
            Rc::clone(&store),
            Rc::new(move |_plan| {}),
        );

        let stack = adw::ViewStack::new();
        stack.set_vhomogeneous(true);

        let home_tab = stack.add_titled(home.widget(), Some("home"), "Home");
        home_tab.set_icon_name(Some("go-home-symbolic"));
        let timer_tab = stack.add_titled(round_timer.widget(), Some("timer"), "Round Timer");
        timer_tab.set_icon_name(Some("preferences-system-time-symbolic"));
        let plans_tab = stack.add_titled(training_plan.widget(), Some("plans"), "Training Plans");
        plans_tab.set_icon_name(Some("view-list-symbolic"));
        let ai_tab = stack.add_titled(ai_coach.widget(), Some("ai"), "AI Coach");
        ai_tab.set_icon_name(Some("computer-symbolic"));
        let history_tab = stack.add_titled(history.widget(), Some("history"), "History");
        history_tab.set_icon_name(Some("document-open-recent-symbolic"));

        let switcher = adw::ViewSwitcher::builder()
            .stack(&stack)
            .policy(adw::ViewSwitcherPolicy::Wide)
            .build();

        let prefs_btn = gtk4::Button::builder()
            .icon_name("preferences-system-symbolic")
            .css_classes(["flat"])
            .tooltip_text("Preferences")
            .build();

        let help_btn = gtk4::Button::builder()
            .icon_name("help-about-symbolic")
            .css_classes(["flat"])
            .tooltip_text("Help & Shortcuts")
            .build();

        let header = HeaderBar::new();
        header.set_title_widget(Some(&switcher));
        header.pack_end(&prefs_btn);
        header.pack_end(&help_btn);

        let toolbar = ToolbarView::new();
        toolbar.add_top_bar(&header);

        let hints_overlay = Rc::new(RefCell::new(ControllerHintsOverlay::new()));
        let overlay = gtk4::Overlay::new();
        overlay.set_child(Some(&stack));
        overlay.add_overlay(hints_overlay.borrow().widget());
        toolbar.set_content(Some(&overlay));

        // Toast overlay wraps everything so notifications float above all pages
        let toast_overlay = libadwaita::ToastOverlay::new();
        toast_overlay.set_child(Some(&toolbar));

        window.set_content(Some(&toast_overlay));

        let mw = Rc::new(Self {
            window,
            controller: RefCell::new(None),
            open_dialog: RefCell::new(None),
            resize_timeout_id: RefCell::new(None),
            round_timer,
            training_plan,
            history,
            home,
            ai_coach,
            stack,
            home_tab,
            timer_tab,
            plans_tab,
            ai_tab,
            history_tab,
            hints_overlay,
            toast_overlay,
            help_window: RefCell::new(None),
            self_weak: RefCell::new(Weak::new()),
        });
        *mw.self_weak.borrow_mut() = Rc::downgrade(&mw);
        set_global_window(&mw);

        // ---- Wire signals ----

        // Preferences button
        {
            let mw_weak = Rc::downgrade(&mw);
            prefs_btn.connect_clicked(move |_| {
                if let Some(m) = mw_weak.upgrade() {
                    m.on_preferences_clicked();
                }
            });
        }

        // Help button
        {
            let mw_weak = Rc::downgrade(&mw);
            help_btn.connect_clicked(move |_| {
                if let Some(m) = mw_weak.upgrade() {
                    m.open_help();
                }
            });
        }

        // Home page: open plan goes to plans tab; browse button switches tab
        {
            let mw_weak_2 = Rc::downgrade(&mw);
            mw.home.set_on_open_plan(Rc::new(move |plan: &crate::models::TrainingPlan| {
                let _ = plan;
                if let Some(m) = mw_weak_2.upgrade() {
                    m.preset_page(plan);
                }
            }));
            let mw_weak = Rc::downgrade(&mw);
            mw.home.set_on_switch_to_plans(Rc::new(move || {
                if let Some(m) = mw_weak.upgrade() {
                    m.stack.set_visible_child_name("plans");
                }
            }));
            let mw_weak = Rc::downgrade(&mw);
            mw.home.set_on_switch_to_timer(Rc::new(move || {
                if let Some(m) = mw_weak.upgrade() {
                    m.stack.set_visible_child_name("timer");
                }
            }));
            let mw_weak = Rc::downgrade(&mw);
            mw.home.set_on_open_help(Rc::new(move || {
                if let Some(m) = mw_weak.upgrade() {
                    m.open_help();
                }
            }));
            let mw_weak = Rc::downgrade(&mw);
            mw.history.set_on_switch_to_timer(Rc::new(move || {
                if let Some(m) = mw_weak.upgrade() {
                    m.stack.set_visible_child_name("timer");
                }
            }));
        }

        // Stack visibility change
        {
            let mw_weak = Rc::downgrade(&mw);
            mw.stack
                .connect_visible_child_notify(move |_stack| {
                    if let Some(m) = mw_weak.upgrade() {
                        m.on_tab_switched();
                    }
                });
        }

        // Keyboard shortcuts: Space = start/pause, S = skip, R = reset
        // (only on the timer and the plan runner, and never while typing or
        // while a widget that handles Space itself has focus).
        {
            let mw_weak = Rc::downgrade(&mw);
            let key_ctrl = gtk4::EventControllerKey::new();
            key_ctrl.connect_key_pressed(move |_ctrl, key, _keycode, _state| {
                use gtk4::glib::Propagation;
                let Some(m) = mw_weak.upgrade() else {
                    return Propagation::Proceed;
                };
                if m.open_dialog.borrow().is_some() {
                    return Propagation::Proceed;
                }
                if let Some(focused) = gtk4::prelude::GtkWindowExt::focus(&m.window) {
                    let w = focused.upcast_ref::<gtk4::Widget>();
                    if w.is::<gtk4::Editable>()
                        || w.is::<gtk4::TextView>()
                        || w.is::<gtk4::SearchEntry>()
                        || w.is::<gtk4::Button>()
                        || w.is::<gtk4::Switch>()
                        || w.is::<gtk4::DropDown>()
                        || w.is::<libadwaita::ComboRow>()
                    {
                        return Propagation::Proceed;
                    }
                }
                let on_timer = m.visible_tab() == "timer";
                let on_runner = m.visible_tab() == "plans"
                    && m.training_plan.visible_page() == "runner";
                match key {
                    gtk4::gdk::Key::question => {
                        m.open_help();
                        Propagation::Stop
                    }
                    gtk4::gdk::Key::comma if _state.contains(gtk4::gdk::ModifierType::CONTROL_MASK) => {
                        m.on_preferences_clicked();
                        Propagation::Stop
                    }
                    gtk4::gdk::Key::space if on_timer || on_runner => {
                        if on_timer {
                            m.round_timer.controller_start();
                        } else {
                            m.training_plan.controller_start();
                        }
                        Propagation::Stop
                    }
                    gtk4::gdk::Key::s if on_timer || on_runner => {
                        if on_timer {
                            m.round_timer.controller_back();
                        } else {
                            m.training_plan.controller_y();
                        }
                        Propagation::Stop
                    }
                    gtk4::gdk::Key::r if on_timer => {
                        m.round_timer.controller_x();
                        Propagation::Stop
                    }
                    _ => Propagation::Proceed,
                }
            });
            mw.window.add_controller(key_ctrl);
        }

        // AI coach plan saved: refresh plans and switch tab
        {
            let mw_weak = Rc::downgrade(&mw);
            mw.ai_coach
                .set_on_plan_saved(Rc::new(move || {
                    if let Some(m) = mw_weak.upgrade() {
                        m.training_plan.refresh_plans();
                        m.stack.set_visible_child_name("plans");
                    }
                }));
        }

        // Sub-pages register transient confirmation dialogs with the window
        // so controller input can be routed to them.
        {
            let mw_weak = Rc::downgrade(&mw);
            mw.training_plan
                .set_dialog_registrar(Rc::new(move |dlg| {
                    if let Some(m) = mw_weak.upgrade() {
                        m.register_alert_dialog(dlg);
                    }
                }));
            let mw_weak = Rc::downgrade(&mw);
            mw.training_plan
                .set_font_refresh_callback(Rc::new(move || {
                    if let Some(m) = mw_weak.upgrade() {
                        m.do_font_update();
                    }
                }));
        }

        mw.rebuild_tabs();
        mw.home.refresh();

        // Restore window size and last visible tab
        {
            let settings = load_settings();
            if settings.window_width > 0 && settings.window_height > 0 {
                mw.window
                    .set_default_size(settings.window_width, settings.window_height);
            }
            let last = settings.last_tab;
            if !last.is_empty() {
                let pages: [(&adw::ViewStackPage, &str); 5] = [
                    (&mw.home_tab, "home"),
                    (&mw.timer_tab, "timer"),
                    (&mw.plans_tab, "plans"),
                    (&mw.ai_tab, "ai"),
                    (&mw.history_tab, "history"),
                ];
                for (page, name) in pages {
                    if name == last && page.is_visible() {
                        mw.stack.set_visible_child_name(name);
                        break;
                    }
                }
            }
        }

        // Persist window size and tab on close
        {
            let mw_weak = Rc::downgrade(&mw);
            mw.window.connect_close_request(move |win| {
                if let Some(m) = mw_weak.upgrade() {
                    let mut s = load_settings();
                    s.window_width = win.width();
                    s.window_height = win.height();
                    s.last_tab = m.visible_tab().to_string();
                    save_settings(&s);
                }
                gtk4::glib::Propagation::Proceed
            });
        }

        {
            let settings = load_settings();
            apply_color_scheme(&settings.color_scheme);
        }

        {
            let mw_weak = Rc::downgrade(&mw);
            mw.window.connect_realize(move |_| {
                if let Some(m) = mw_weak.upgrade() {
                    m.on_realize();
                }
            });
        }

        {
            let mw_weak = Rc::downgrade(&mw);
            mw.window
                .connect_notify_local(Some("is-fullscreen"), move |_, _| {
                    if let Some(m) = mw_weak.upgrade() {
                        m.on_window_state_changed();
                    }
                });
        }
        {
            let mw_weak = Rc::downgrade(&mw);
            mw.window
                .connect_notify_local(Some("is-maximized"), move |_, _| {
                    if let Some(m) = mw_weak.upgrade() {
                        m.on_window_state_changed();
                    }
                });
        }
        {
            let mw_weak = Rc::downgrade(&mw);
            mw.window.connect_notify_local(Some("default-width"), move |_, _| {
                if let Some(m) = mw_weak.upgrade() {
                    m.on_window_state_changed();
                }
            });
        }
        {
            let mw_weak = Rc::downgrade(&mw);
            mw.window
                .connect_notify_local(Some("default-height"), move |_, _| {
                    if let Some(m) = mw_weak.upgrade() {
                        m.on_window_state_changed();
                    }
                });
        }

        mw
    }

    pub fn gtk_window(&self) -> gtk4::Window {
        self.window.clone().upcast()
    }

    pub fn show_toast(&self, title: &str) {
        let toast = libadwaita::Toast::builder().title(title).timeout(3).build();
        self.toast_overlay.add_toast(toast);
    }

    /// Toast with an action button (e.g. "Undo"). The callback runs when the
    /// user activates the button.
    pub fn show_toast_action(&self, title: &str, action: &str, cb: Rc<dyn Fn()>) {
        let toast = libadwaita::Toast::builder()
            .title(title)
            .button_label(action)
            .timeout(6)
            .build();
        toast.connect_button_clicked(move |_| cb());
        self.toast_overlay.add_toast(toast);
    }

    /// Debug helper (TRAINING_SHOT_DIR): render every tab to a PNG in `dir`.
    /// Uses the widget paintable snapshot, so it works without a compositor.
    pub fn screenshot_tabs(&self, dir: &str) {
        eprintln!("[shots] screenshot_tabs start dir={}", dir);
        let _ = std::fs::create_dir_all(dir);
        let ctx = gtk4::glib::MainContext::default();

        // Wait for the window to be mapped and sized
        for _ in 0..40 {
            for _ in 0..5 {
                let _ = ctx.iteration(false);
            }
            if self.stack.width() > 100 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        eprintln!("[shots] stack size {}x{}", self.stack.width(), self.stack.height());

        let Some(content) = self.window.content() else {
            eprintln!("[shots] no window content");
            return;
        };

        // One renderer for all tabs (for_surface already realizes it)
        let Some(window_surface) = self.window.surface() else {
            eprintln!("[shots] window has no surface");
            return;
        };
        let Some(renderer) = gtk4::gsk::Renderer::for_surface(&window_surface) else {
            eprintln!("[shots] no renderer");
            return;
        };

        for tab in ["home", "timer", "plans", "history"] {
            self.stack.set_visible_child_name(tab);
            // Pump the main loop so the new tab is laid out and drawn
            for _ in 0..10 {
                for _ in 0..5 {
                    let _ = ctx.iteration(false);
                }
                std::thread::sleep(std::time::Duration::from_millis(60));
            }
            let w = content.width().max(1) as f64;
            let h = content.height().max(1) as f64;
            let snap = gtk4::Snapshot::new();
            let paintable = gtk4::WidgetPaintable::new(Some(&content));
            paintable.snapshot(&snap, w, h);
            // Unwrap the widget paintable's render node:
            // gtk_snapshot_free_to_node returns None for the wrapper chain,
            // but the non-consuming ffi call on the re-snapshotted paintable
            // yields the node directly.
            let node = snap.to_paintable(None).and_then(|p| {
                use gtk4::glib::translate::ToGlibPtr;
                let snap2 = gtk4::Snapshot::new();
                p.snapshot(&snap2, w, h);
                let node2 = unsafe {
                    let ptr2: *mut gtk4::ffi::GtkSnapshot = snap2.to_glib_none().0;
                    gtk4::ffi::gtk_snapshot_to_node(ptr2)
                };
                if node2.is_null() {
                    None
                } else {
                    use gtk4::glib::translate::from_glib_full;
                    let node: gtk4::gsk::RenderNode = unsafe { from_glib_full(node2) };
                    Some(node)
                }
            });
            let Some(node) = node else {
                eprintln!("[shots] {} snapshot produced no node", tab);
                continue;
            };
            let viewport = gtk4::graphene::Rect::new(0.0, 0.0, w as f32, h as f32);
            let tex = gtk4::gsk::prelude::GskRendererExt::render_texture(
                &renderer,
                &node,
                Some(&viewport),
            );
            let path = format!("{}/{}.png", dir, tab);
            match tex.save_to_png(&path) {
                Ok(()) => println!("screenshot saved: {}", path),
                Err(e) => eprintln!("screenshot {} failed: {}", path, e),
            }
        }

        // A realized renderer must be unrealized before disposal
        gtk4::gsk::prelude::GskRendererExt::unrealize(&renderer);
    }

    pub fn register_exercise_picker_dialog(
        &self,
        picker: Rc<crate::exercise_picker::ExercisePicker>,
    ) {
        *self.open_dialog.borrow_mut() = Some(OpenDialog::ExercisePicker(picker));
    }

    pub fn preset_page(&self, plan: &crate::models::TrainingPlan) {
        self.training_plan.open_plan(plan);
        self.stack.set_visible_child_name("plans");
    }

    fn on_realize(self: &Rc<Self>) {
        self.apply_app_css();
        let controller = ControllerManager::new();
        {
            let mw_weak = self.self_weak.borrow().clone();
            controller.connect_local("action-activated", false, move |args| {
                // args[0] is the emitting object (the ControllerManager
                // instance); the String signal parameter is args[1].
                let action = args
                    .get(1)
                    .and_then(|v| v.get::<String>().ok())
                    .unwrap_or_default();
                if let Some(m) = mw_weak.upgrade() {
                    m.on_controller_action(&action);
                }
                None
            });
        }
        {
            let mw_weak = self.self_weak.borrow().clone();
            controller.connect_local("device-connected", false, move |_args| {
                if let Some(m) = mw_weak.upgrade() {
                    m.on_controller_connected();
                }
                None
            });
        }
        {
            let mw_weak = self.self_weak.borrow().clone();
            controller.connect_local("device-disconnected", false, move |_args| {
                if let Some(m) = mw_weak.upgrade() {
                    m.hints_overlay.borrow().hide();
                }
                None
            });
        }
        *self.controller.borrow_mut() = Some(controller);

        if self.is_deck_mode() {
            self.window.add_css_class("deck-mode");
        }

        let mw_weak = Rc::downgrade(self);
        glib::idle_add_local_once(move || {
            if let Some(m) = mw_weak.upgrade() {
                m.do_font_update();
            }
        });
    }

    fn is_deck_mode(&self) -> bool {
        let Some(ref controller) = *self.controller.borrow() else {
            return false;
        };
        let settings = load_settings();
        match settings.deck_mode.as_str() {
            "on" => true,
            "off" => false,
            _ => controller.deck_mode(),
        }
    }

    fn apply_app_css(&self) {
        let provider = gtk4::CssProvider::new();
        let flatpak_css = std::path::PathBuf::from("/app/share/training/app_style.css");
        let css_path = if flatpak_css.exists() {
            flatpak_css
        } else {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("src")
                .join("app_style.css")
        };
        provider.load_from_path(css_path.to_str().unwrap_or("app_style.css"));

        if let Some(display) = gtk4::gdk::Display::default() {
            gtk4::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk4::STYLE_PROVIDER_PRIORITY_USER,
            );
        }
    }

    /// Open the "Help & Shortcuts" window (only one at a time).
    pub fn open_help(self: &Rc<Self>) {
        let mut slot = self.help_window.borrow_mut();
        if let Some(existing) = slot.as_ref() {
            existing.present();
            return;
        }
        let win = Rc::new(crate::help_page::HelpWindow::new(&self.window));
        *slot = Some(Rc::clone(&win));
        win.present();
    }

    fn on_controller_connected(&self) {
        self.update_hints_visibility();
    }

    /// Pages register confirmation AlertDialogs here so controller input is
    /// routed to them; `None` releases the registration when it closes.
    pub fn register_alert_dialog(&self, dlg: Option<(Rc<libadwaita::AlertDialog>, String)>) {
        *self.open_dialog.borrow_mut() = dlg.map(|(d, id)| OpenDialog::AlertConfirm(d, id));
    }

    pub fn update_hints_visibility(&self) {
        let settings = load_settings();
        let connected = self
            .controller
            .borrow()
            .as_ref()
            .map(|c| c.connected())
            .unwrap_or(false);

        if !settings.gamepad_enabled || !connected || !settings.gamepad_hints {
            self.hints_overlay.borrow().hide();
            return;
        }

        let child = match self.stack.visible_child() {
            Some(child) => child,
            None => return,
        };

        let mut overlay = self.hints_overlay.borrow_mut();

        if child.as_ptr() == self.round_timer.widget().upcast_ref::<gtk4::Widget>().as_ptr() {
            overlay.set_context("timer");
        } else if child.as_ptr() == self.training_plan.widget().upcast_ref::<gtk4::Widget>().as_ptr() {
            let ctx = self.training_plan.get_controller_context();
            let sub = self.training_plan.get_controller_sub_key();
            overlay.set_sub_key(&sub);
            overlay.set_context(&ctx);
        } else if child.as_ptr() == self.home.widget().upcast_ref::<gtk4::Widget>().as_ptr() {
            overlay.set_context("list");
        } else if child.as_ptr() == self.history.widget().upcast_ref::<gtk4::Widget>().as_ptr() {
            overlay.set_context("list");
        } else if child.as_ptr() == self.ai_coach.widget().upcast_ref::<gtk4::Widget>().as_ptr() {
            let ctx = self.ai_coach.get_controller_context();
            overlay.set_context(&ctx);
        } else {
            overlay.hide();
        }
    }

    fn on_controller_action(&self, action: &str) {
        let settings = load_settings();
        if std::env::var("CONTROLLER_DEBUG").map(|v| !v.is_empty()).unwrap_or(false) {
            println!("[window] controller action '{}' (gamepad_enabled={})", action, settings.gamepad_enabled);
        }
        if !settings.gamepad_enabled {
            return;
        }

        // The help window is modal-ish: B closes it, everything else is ignored.
        if let Some(help) = self.help_window.borrow().as_ref() {
            if help.is_open() {
                if action == "b" || action == "back" {
                    help.close();
                }
                return;
            }
        }

        match action {
            "l1" => {
                self.switch_tab(-1);
                return;
            }
            "r1" => {
                self.switch_tab(1);
                return;
            }
            "guide" => {
                let window = self.window.clone();
                glib::idle_add_local_once(move || {
                    if window.is_fullscreen() {
                        window.unfullscreen();
                    } else {
                        window.fullscreen();
                    }
                });
                return;
            }
            _ => {}
        }

        // Dialogs get priority
        if let Some(dialog) = self.open_dialog.borrow().as_ref() {
            match dialog {
                OpenDialog::Preferences(p) => match action {
                    "dpad_up" => p.controller_dpad_up(),
                    "dpad_down" => p.controller_dpad_down(),
                    "dpad_left" => p.controller_dpad_left(),
                    "dpad_right" => p.controller_dpad_right(),
                    "a" => p.controller_a(),
                    "b" => p.controller_b(),
                    "start" => p.controller_start(),
                    _ => {}
                },
                OpenDialog::ExercisePicker(p) => match action {
                    "dpad_up" => p.controller_dpad_up(),
                    "dpad_down" => p.controller_dpad_down(),
                    "a" => p.controller_a(),
                    "b" => p.controller_b(),
                    _ => {}
                },
                OpenDialog::AlertConfirm(dlg, confirm_id) => match action {
                    "a" => {
                        // Confirm: AdwAlertDialog emits ::response with the
                        // close response when closed programmatically.
                        dlg.set_close_response(confirm_id.as_str());
                        dlg.close();
                    }
                    "b" | "back" => {
                        // Cancel (the default close response).
                        dlg.close();
                    }
                    _ => {}
                },
            }
            return;
        }

        let child = match self.stack.visible_child() {
            Some(c) => c,
            None => return,
        };

        if child.as_ptr() == self.round_timer.widget().upcast_ref::<gtk4::Widget>().as_ptr() {
            match action {
                "dpad_up" => self.round_timer.controller_dpad_up(),
                "dpad_down" => self.round_timer.controller_dpad_down(),
                "dpad_left" => self.round_timer.controller_dpad_left(),
                "dpad_right" => self.round_timer.controller_dpad_right(),
                "a" => self.round_timer.controller_a(),
                "start" => self.round_timer.controller_start(),
                "x" => self.round_timer.controller_x(),
                "b" => self.round_timer.controller_back(),
                _ => {}
            }
        } else if child.as_ptr() == self.training_plan.widget().upcast_ref::<gtk4::Widget>().as_ptr() {
            match action {
                "dpad_up" => self.training_plan.controller_dpad_up(),
                "dpad_down" => self.training_plan.controller_dpad_down(),
                "dpad_left" => self.training_plan.controller_dpad_left(),
                "dpad_right" => self.training_plan.controller_dpad_right(),
                "a" => self.training_plan.controller_a(),
                "b" | "back" => self.training_plan.controller_back(),
                "start" => self.training_plan.controller_start(),
                "y" => self.training_plan.controller_y(),
                "x" => self.training_plan.controller_x(),
                _ => {}
            }
        } else if child.as_ptr() == self.history.widget().upcast_ref::<gtk4::Widget>().as_ptr() {
            match action {
                "dpad_up" => self.history.controller_dpad_up(),
                "dpad_down" => self.history.controller_dpad_down(),
                "a" => self.history.controller_a(),
                "b" => self.history.controller_back(),
                _ => {}
            }
        } else if child.as_ptr() == self.home.widget().upcast_ref::<gtk4::Widget>().as_ptr() {
            match action {
                "dpad_up" => self.home.controller_dpad_up(),
                "dpad_down" => self.home.controller_dpad_down(),
                "a" => self.home.controller_a(),
                "b" => self.home.controller_back(),
                _ => {}
            }
        } else if child.as_ptr() == self.ai_coach.widget().upcast_ref::<gtk4::Widget>().as_ptr() {
            match action {
                "dpad_up" => self.ai_coach.controller_dpad_up(),
                "dpad_down" => self.ai_coach.controller_dpad_down(),
                "dpad_left" => self.ai_coach.controller_dpad_left(),
                "dpad_right" => self.ai_coach.controller_dpad_right(),
                "a" => self.ai_coach.controller_a(),
                "b" => self.ai_coach.controller_b(),
                "select" => self.ai_coach.controller_select(),
                "start" => self.ai_coach.controller_start(),
                "y" => self.ai_coach.controller_y(),
                "back" => self.ai_coach.controller_back(),
                _ => {}
            }
        }
    }

    fn switch_tab(&self, delta: i32) {
        let pages: [(adw::ViewStackPage, &str); 5] = [
            (self.home_tab.clone(), "home"),
            (self.timer_tab.clone(), "timer"),
            (self.plans_tab.clone(), "plans"),
            (self.ai_tab.clone(), "ai"),
            (self.history_tab.clone(), "history"),
        ];
        let visible_names: Vec<&str> = pages
            .iter()
            .filter(|(p, _)| p.is_visible())
            .map(|(_, n)| *n)
            .collect();

        let current_child = match self.stack.visible_child() {
            Some(c) => c,
            None => return,
        };

        // Find the index of the currently visible page
        let tab_order: [(&adw::ViewStackPage, &str, gtk4::Widget); 5] = [
            (&self.home_tab, "home", self.home.widget().clone().upcast()),
            (&self.timer_tab, "timer", self.round_timer.widget().clone().upcast()),
            (&self.plans_tab, "plans", self.training_plan.widget().clone().upcast()),
            (&self.ai_tab, "ai", self.ai_coach.widget().clone().upcast()),
            (&self.history_tab, "history", self.history.widget().clone().upcast()),
        ];

        let mut current_name: Option<&str> = tab_order
            .iter()
            .find(|(_, _, child)| child.as_ptr() == current_child.as_ptr())
            .map(|(_, name, _)| *name);
        let _ = &mut current_name;
        let Some(current_name) = current_name.take() else { return };

        let Some(idx) = visible_names.iter().position(|n| *n == current_name) else {
            return;
        };
        let next_idx = (idx as i32 + delta).rem_euclid(visible_names.len() as i32) as usize;
        self.stack.set_visible_child_name(visible_names[next_idx]);
    }

    fn visible_tab(&self) -> &'static str {
        let Some(child) = self.stack.visible_child() else {
            return "";
        };
        let w = child.upcast_ref::<gtk4::Widget>();
        if w.as_ptr() == self.round_timer.widget().upcast_ref::<gtk4::Widget>().as_ptr() {
            "timer"
        } else if w.as_ptr() == self.training_plan.widget().upcast_ref::<gtk4::Widget>().as_ptr() {
            "plans"
        } else if w.as_ptr() == self.home.widget().upcast_ref::<gtk4::Widget>().as_ptr() {
            "home"
        } else if w.as_ptr() == self.ai_coach.widget().upcast_ref::<gtk4::Widget>().as_ptr() {
            "ai"
        } else if w.as_ptr() == self.history.widget().upcast_ref::<gtk4::Widget>().as_ptr() {
            "history"
        } else {
            ""
        }
    }

    fn on_tab_switched(&self) {
        let current = match self.stack.visible_child() {
            Some(c) => c,
            None => return,
        };
        if current.as_ptr() == self.history.widget().upcast_ref::<gtk4::Widget>().as_ptr() {
            self.history.refresh();
        } else if current.as_ptr() == self.home.widget().upcast_ref::<gtk4::Widget>().as_ptr() {
            self.home.refresh();
        } else if current.as_ptr() == self.training_plan.widget().upcast_ref::<gtk4::Widget>().as_ptr() {
            self.training_plan.on_page_visible();
        } else if current.as_ptr() == self.ai_coach.widget().upcast_ref::<gtk4::Widget>().as_ptr() {
            self.ai_coach.on_page_visible();
        }

        self.on_window_state_changed_impl();
        self.update_hints_visibility();
        // Scaled fonts (timer countdown etc.) are only valid for the visible
        // tab; re-apply whenever another tab becomes visible.
        self.do_font_update();
    }

    fn on_window_state_changed_impl(&self) {
        if let Some(id) = self.resize_timeout_id.borrow_mut().take() {
            id.remove();
        }
    }

    pub fn on_window_state_changed(self: &Rc<Self>) {
        if let Some(id) = self.resize_timeout_id.borrow_mut().take() {
            id.remove();
        }
        let mw_weak = Rc::downgrade(self);
        let id = glib::timeout_add_local(Duration::from_millis(200), move || {
            if let Some(m) = mw_weak.upgrade() {
                m.do_font_update();
                *m.resize_timeout_id.borrow_mut() = None;
            }
            glib::ControlFlow::Break
        });
        *self.resize_timeout_id.borrow_mut() = Some(id);
    }

    fn font_dims(&self) -> (i32, i32) {
        let mut w = self.stack.width();
        let mut h = self.stack.height();
        if w <= 1 || h <= 1 {
            w = self.window.width();
            h = self.window.height();
        }
        h = (h - 90).max(h / 2);
        (w, h)
    }

    fn do_font_update(&self) {
        let (w, h) = self.font_dims();
        if w <= 0 || h <= 0 {
            return;
        }

        let current = match self.stack.visible_child() {
            Some(c) => c,
            None => return,
        };
        if current.as_ptr() == self.round_timer.widget().upcast_ref::<gtk4::Widget>().as_ptr() {
            self.round_timer.update_fonts(w, h);
        } else if current.as_ptr() == self.training_plan.widget().upcast_ref::<gtk4::Widget>().as_ptr() {
            self.training_plan.update_fonts(w, h);
        } else if current.as_ptr() == self.history.widget().upcast_ref::<gtk4::Widget>().as_ptr() {
            self.history.update_fonts(w, h);
        } else if current.as_ptr() == self.home.widget().upcast_ref::<gtk4::Widget>().as_ptr() {
            self.home.update_fonts(w, h);
        } else if current.as_ptr() == self.ai_coach.widget().upcast_ref::<gtk4::Widget>().as_ptr() {
            self.ai_coach.update_fonts(w, h);
        }
    }

    fn rebuild_tabs(&self) {
        let settings = load_settings();
        let show_home = settings.show_home_page;
        let show_timer = settings.show_timer_page;
        let show_workout = settings.show_workout_page;
        let show_ai = settings.show_ai_page;

        let visibility = [
            ("home", show_home),
            ("timer", show_timer),
            ("plans", show_workout),
            ("ai", show_ai),
            ("history", true),
        ];

        self.home_tab.set_visible(show_home);
        self.timer_tab.set_visible(show_timer);
        self.plans_tab.set_visible(show_workout);
        self.ai_tab.set_visible(show_ai);

        if let Some(name) = self.stack.visible_child_name() {
            let name = name.as_str();
            if visibility
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, v)| *v)
                .unwrap_or(false)
            {
                return;
            }
        }

        if show_home {
            self.stack.set_visible_child_name("home");
        } else if show_timer {
            self.stack.set_visible_child_name("timer");
        } else if show_workout {
            self.stack.set_visible_child_name("plans");
        } else if show_ai {
            self.stack.set_visible_child_name("ai");
        } else {
            self.stack.set_visible_child_name("history");
        }
    }

    pub fn on_dialog_closed(&self) {
        *self.open_dialog.borrow_mut() = None;
        self.rebuild_tabs();
        self.update_hints_visibility();
    }

    /// Re-read all data-backed pages (used after a backup restore).
    pub fn refresh_all_pages(&self) {
        self.training_plan.refresh_plans();
        self.home.refresh();
        self.history.refresh();
    }

    fn on_preferences_clicked(self: &Rc<Self>) {
        let gtk_win = self.gtk_window();
        let dialog = PreferencesDialog::new(&gtk_win);
        dialog.present(&gtk_win);
        *self.open_dialog.borrow_mut() = Some(OpenDialog::Preferences(dialog.clone()));
        // Clear the open dialog when closed
        let mw_weak = Rc::downgrade(self);
        dialog.widget().connect_closed(move |_| {
            if let Some(m) = mw_weak.upgrade() {
                m.on_dialog_closed();
            }
        });
    }
}