use gtk4::cairo;
use gtk4::prelude::*;
use gtk4::DrawingArea;
use std::cell::RefCell;
use std::f64::consts::PI;
use std::rc::Rc;

const ACCENT: (f64, f64, f64) = (0.21, 0.52, 0.89); // #3584e4
const CURRENT: (f64, f64, f64) = (0.20, 0.82, 0.48); // #33d17a

struct ChartState {
    bars: RefCell<Vec<(String, f64)>>, // label, value (minutes)
}

/// Simple vertical bar chart drawn with cairo.
pub struct BarChart {
    widget: DrawingArea,
    state: Rc<ChartState>,
}

impl BarChart {
    pub fn new(height: i32) -> Self {
        let widget = DrawingArea::new();
        widget.set_content_height(height);
        widget.set_hexpand(true);
        widget.set_vexpand(false);

        let state = Rc::new(ChartState {
            bars: RefCell::new(Vec::new()),
        });

        {
            let state = Rc::clone(&state);
            widget.set_draw_func(move |_da, cr, width, height| {
                draw_chart(&state, cr, width, height);
            });
        }

        Self { widget, state }
    }

    pub fn widget(&self) -> &DrawingArea {
        &self.widget
    }

    pub fn set_data(&self, bars: Vec<(String, f64)>) {
        *self.state.bars.borrow_mut() = bars;
        self.widget.queue_draw();
    }
}

fn draw_chart(state: &ChartState, cr: &cairo::Context, width: i32, height: i32) {
    let bars = state.bars.borrow();
    if bars.is_empty() || width < 40 || height < 40 {
        return;
    }

    let w = width as f64;
    let h = height as f64;
    let top_pad = 22.0;
    let bottom_pad = 20.0;
    let side_pad = 8.0;
    let plot_h = h - top_pad - bottom_pad;
    let n = bars.len() as f64;
    let slot_w = (w - side_pad * 2.0) / n;
    let bar_w = (slot_w * 0.65).min(48.0);

    // Baseline
    cr.set_source_rgba(0.5, 0.5, 0.5, 0.35);
    cr.set_line_width(1.0);
    cr.move_to(side_pad, h - bottom_pad);
    cr.line_to(w - side_pad, h - bottom_pad);
    let _ = cr.stroke();

    let max_v = bars.iter().map(|(_, v)| *v).fold(0.0_f64, f64::max).max(1.0);

    // Cairo text setup
    cr.set_font_size(11.0);
    let _ = cr.select_font_face(
        "Sans",
        cairo::FontSlant::Normal,
        cairo::FontWeight::Bold,
    );

    for (i, (label, value)) in bars.iter().enumerate() {
        let is_last = i == bars.len() - 1;
        let (r, g, b) = if is_last { CURRENT } else { ACCENT };
        let bar_h = plot_h * (value / max_v);
        let x = side_pad + slot_w * i as f64 + (slot_w - bar_w) / 2.0;
        let y = h - bottom_pad - bar_h;

        cr.set_source_rgba(r, g, b, 0.85);
        let radius = (bar_w / 4.0).min(6.0).min(bar_h / 2.0);
        rounded_rect(cr, x, y, bar_w, bar_h, radius);
        let _ = cr.fill();

        // Value on top of the bar
        let val_txt = format!("{}", value.round());
        cr.set_source_rgba(0.5, 0.5, 0.5, 1.0);
        let vw = cr.text_extents(&val_txt).map(|e| e.width()).unwrap_or(0.0);
        let _ = cr.move_to(
            x + (bar_w - vw) / 2.0,
            y - 6.0,
        );
        let _ = cr.show_text(&val_txt);

        // Label under the bar
        cr.set_source_rgba(0.5, 0.5, 0.5, 1.0);
        let lw = cr.text_extents(label).map(|e| e.width()).unwrap_or(0.0);
        let _ = cr.move_to(
            x + (bar_w - lw) / 2.0,
            h - bottom_pad + 14.0,
        );
        let _ = cr.show_text(label);
    }
}

fn rounded_rect(cr: &cairo::Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    if r <= 0.0 || h <= 1.0 {
        cr.rectangle(x, y, w, h);
        return;
    }
    cr.new_sub_path();
    cr.arc(x + w - r, y + r, r, -PI / 2.0, 0.0);
    cr.arc(x + w - r, y + h - r, r, 0.0, PI / 2.0);
    cr.arc(x + r, y + h - r, r, PI / 2.0, PI);
    cr.arc(x + r, y + r, r, PI, PI * 1.5);
    cr.close_path();
}