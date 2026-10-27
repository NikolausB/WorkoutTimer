use gtk4::pango;
use gtk4::Label;

fn compute_font_size(width: i32, height: i32, base_ratio: f64, min_size: i32, max_size: i32) -> i32 {
    let available = width.min(height).max((width as f64 * 0.35) as i32);
    let size = (available as f64 * base_ratio) as i32;
    size.max(min_size).min(max_size)
}

fn apply_font(label: &Label, size: i32, weight: pango::Weight) {
    let mut desc = pango::FontDescription::new();
    desc.set_size(size * pango::SCALE);
    desc.set_weight(weight);
    let attrs = pango::AttrList::new();
    attrs.insert(pango::AttrFontDesc::new(&desc));
    label.set_attributes(Some(&attrs));
}

pub enum LabelType {
    /// Timer-sized text (kept for callers that scale by window height).
    #[allow(dead_code)]
    Timer,
    Exercise,
    Info,
}

/// Set an explicit font size (in px) on a label.
pub fn set_label_size(label: &Label, size: i32) {
    apply_font(label, size, pango::Weight::Heavy);
}

pub fn apply_scaling(labels: &[(LabelType, &Label)], width: i32, height: i32) {
    let timer_size = compute_font_size(width, height, 0.10, 36, 96);
    let exercise_size = compute_font_size(width, height, 0.045, 24, 56);
    let info_size = compute_font_size(width, height, 0.035, 18, 40);

    for (label_type, label) in labels {
        match label_type {
            LabelType::Timer => apply_font(label, timer_size, pango::Weight::Heavy),
            LabelType::Exercise => apply_font(label, exercise_size, pango::Weight::Bold),
            LabelType::Info => apply_font(label, info_size, pango::Weight::Normal),
        }
    }
}
