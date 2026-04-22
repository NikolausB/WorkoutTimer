from gi.repository import Pango


def _compute_font_size(width, height, base_ratio, min_size=16, max_size=120):
    available = min(width, height)
    size = int(available * base_ratio)
    return max(min_size, min(max_size, size))


def apply_scaling(labels, width, height):
    timer_size = _compute_font_size(width, height, 0.12, 36, 128)
    exercise_size = _compute_font_size(width, height, 0.06, 24, 72)
    info_size = _compute_font_size(width, height, 0.035, 18, 40)

    for label_type, label in labels:
        if label_type == "timer":
            _apply_font(label, timer_size, 800)
        elif label_type == "exercise":
            _apply_font(label, exercise_size, 700)
        elif label_type == "info":
            _apply_font(label, info_size, 400)


def _apply_font(label, size, weight=400):
    try:
        desc = Pango.FontDescription()
        desc.set_size(size * Pango.SCALE)
        desc.set_weight(weight)
        attrs = Pango.AttrList()
        attrs.insert(Pango.attr_font_desc_new(desc))
        label.set_attributes(attrs)
    except Exception:
        pass