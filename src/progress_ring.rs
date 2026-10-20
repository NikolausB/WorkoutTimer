use gtk4::cairo;
use gtk4::prelude::*;
use gtk4::DrawingArea;
use std::cell::Cell;
use std::f64::consts::TAU;
use std::rc::Rc;

// Palette aligned with the CSS phase colors in app_style.css.
pub const COLOR_GO: (f64, f64, f64) = (0.20, 0.82, 0.48); // #33d17a green
pub const COLOR_REST: (f64, f64, f64) = (0.21, 0.52, 0.89); // #3584e4 blue
pub const COLOR_BREAK: (f64, f64, f64) = (0.90, 0.38, 0.0); // #e66100 orange
pub const COLOR_DONE: (f64, f64, f64) = (0.96, 0.76, 0.07); // #f5c211 gold
pub const COLOR_NEUTRAL: (f64, f64, f64) = (0.58, 0.58, 0.58);

struct RingState {
    progress: Cell<f64>,
    color: Cell<(f64, f64, f64)>,
    dimmed: Cell<bool>,
}

/// Circular progress ring drawn with cairo.
pub struct ProgressRing {
    widget: DrawingArea,
    state: Rc<RingState>,
}

impl ProgressRing {
    pub fn new(size: i32) -> Self {
        let widget = DrawingArea::new();
        widget.set_content_width(size);
        widget.set_content_height(size);
        widget.set_halign(gtk4::Align::Center);
        widget.set_valign(gtk4::Align::Center);

        let state = Rc::new(RingState {
            progress: Cell::new(1.0),
            color: Cell::new(COLOR_NEUTRAL),
            dimmed: Cell::new(false),
        });

        {
            let state = Rc::clone(&state);
            widget.set_draw_func(move |_da, cr, width, height| {
                draw_ring(&state, cr, width, height);
            });
        }

        Self { widget, state }
    }

    pub fn widget(&self) -> &DrawingArea {
        &self.widget
    }

    /// Resize the ring (used to fit the page on short windows).
    pub fn set_size(&self, size: i32) {
        self.widget.set_content_width(size);
        self.widget.set_content_height(size);
        self.widget.queue_draw();
    }

    /// Set progress in 0.0..=1.0.
    pub fn set_progress(&self, p: f64) {
        self.state.progress.set(p.clamp(0.0, 1.0));
        self.widget.queue_draw();
    }

    pub fn set_color(&self, color: (f64, f64, f64)) {
        self.state.color.set(color);
        self.widget.queue_draw();
    }

    /// Dim the ring while the timer is paused.
    pub fn set_dimmed(&self, dimmed: bool) {
        self.state.dimmed.set(dimmed);
        self.widget.queue_draw();
    }
}

fn draw_ring(state: &RingState, cr: &cairo::Context, width: i32, height: i32) {
    let w = width as f64;
    let h = height as f64;
    let line_w = (w.min(h) * 0.075).max(6.0);
    let radius = (w.min(h) / 2.0) - line_w / 2.0 - 2.0;
    if radius <= 0.0 {
        return;
    }
    let cx = w / 2.0;
    let cy = h / 2.0;

    // Track
    cr.set_line_width(line_w);
    cr.set_source_rgba(0.5, 0.5, 0.5, 0.25);
    cr.arc(cx, cy, radius, 0.0, TAU);
    let _ = cr.stroke();

    // Progress arc, starting at 12 o'clock
    let progress = state.progress.get();
    if progress > 0.0 {
        let (r, g, b) = state.color.get();
        let alpha = if state.dimmed.get() { 0.35 } else { 1.0 };
        cr.set_source_rgba(r, g, b, alpha);
        cr.arc(cx, cy, radius, -std::f64::consts::FRAC_PI_2, -std::f64::consts::FRAC_PI_2 + TAU * progress);
        cr.set_line_cap(cairo::LineCap::Round);
        let _ = cr.stroke();
    }
}