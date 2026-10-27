use gtk4::glib;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

const TICK_INTERVAL_MS: u32 = 100;

/// Countdown timer anchored to the wall clock: the remaining time is derived
/// from an `Instant` deadline on every tick, so glib timeout jitter cannot
/// make the timer drift. Pause stores the remaining time; resume re-anchors.
pub struct TimerCore {
    source_id: Rc<RefCell<Option<glib::SourceId>>>,
    pub remaining_seconds: Rc<Cell<f64>>,
    pub total_seconds: Rc<Cell<i64>>,
    pub is_running: Rc<Cell<bool>>,
    deadline: Rc<Cell<Option<Instant>>>,
    pub on_tick: Rc<RefCell<Option<Box<dyn Fn(f64, i64)>>>>,
    pub on_finished: Rc<RefCell<Option<Box<dyn Fn()>>>>,
}

impl TimerCore {
    pub fn new() -> Self {
        Self {
            source_id: Rc::new(RefCell::new(None)),
            remaining_seconds: Rc::new(Cell::new(0.0)),
            total_seconds: Rc::new(Cell::new(0)),
            is_running: Rc::new(Cell::new(false)),
            deadline: Rc::new(Cell::new(None)),
            on_tick: Rc::new(RefCell::new(None)),
            on_finished: Rc::new(RefCell::new(None)),
        }
    }

    pub fn start(&self, seconds: i64) {
        self.stop();
        self.remaining_seconds.set(seconds as f64);
        self.total_seconds.set(seconds);
        self.deadline.set(Some(
            Instant::now() + Duration::from_secs(seconds.max(0) as u64),
        ));
        self.is_running.set(true);
        self.schedule_tick();
    }

    pub fn resume(&self) {
        let remaining = self.remaining_seconds.get();
        if remaining > 0.0 && !self.is_running.get() {
            self.deadline
                .set(Some(Instant::now() + Duration::from_secs_f64(remaining)));
            self.is_running.set(true);
            self.schedule_tick();
        }
    }

    pub fn pause(&self) {
        if self.is_running.get() {
            if let Some(dl) = self.deadline.get() {
                let rem = dl.saturating_duration_since(Instant::now()).as_secs_f64();
                self.remaining_seconds.set(rem.max(0.0));
            }
        }
        self.is_running.set(false);
        self.cancel_tick();
    }

    pub fn stop(&self) {
        self.is_running.set(false);
        self.deadline.set(None);
        self.cancel_tick();
        self.remaining_seconds.set(0.0);
        self.total_seconds.set(0);
    }

    /// Add (or subtract, with negative values) time to the current countdown.
    pub fn add_seconds(&self, seconds: i64) {
        let new_rem = (self.remaining_seconds.get() + seconds as f64).max(0.0);
        self.remaining_seconds.set(new_rem);
        if self.is_running.get() {
            if let Some(dl) = self.deadline.get() {
                self.deadline
                    .set(Some(dl + Duration::from_secs_f64(seconds as f64)));
            }
        }
    }

    fn schedule_tick(&self) {
        self.cancel_tick();
        let remaining = Rc::clone(&self.remaining_seconds);
        let total = Rc::clone(&self.total_seconds);
        let is_running = Rc::clone(&self.is_running);
        let deadline = Rc::clone(&self.deadline);
        let on_tick = Rc::clone(&self.on_tick);
        let on_finished = Rc::clone(&self.on_finished);
        let source_id = Rc::clone(&self.source_id);

        let id = glib::timeout_add_local(Duration::from_millis(TICK_INTERVAL_MS as u64), move || {
            if !is_running.get() {
                return glib::ControlFlow::Break;
            }

            let rem = match deadline.get() {
                Some(dl) => dl.saturating_duration_since(Instant::now()).as_secs_f64(),
                None => 0.0,
            };
            remaining.set(rem);
            if rem <= 0.0 {
                remaining.set(0.0);
                is_running.set(false);
                deadline.set(None);
                if let Some(ref cb) = *on_tick.borrow() {
                    cb(remaining.get(), total.get());
                }
                if let Some(ref cb) = *on_finished.borrow() {
                    cb();
                }
                *source_id.borrow_mut() = None;
                return glib::ControlFlow::Break;
            }

            if let Some(ref cb) = *on_tick.borrow() {
                cb(rem, total.get());
            }
            glib::ControlFlow::Continue
        });
        *self.source_id.borrow_mut() = Some(id);
    }

    fn cancel_tick(&self) {
        if let Some(id) = self.source_id.borrow_mut().take() {
            id.remove();
        }
    }
}

impl Default for TimerCore {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for TimerCore {
    fn drop(&mut self) {
        self.cancel_tick();
    }
}

// Timer unit tests live in main.rs::ui_tests — `timeout_add_local` can only
// acquire the default main context, which the GTK smoke test owns.
