use gtk4::prelude::*;
use libadwaita::prelude::*;
use gtk4::Widget;

pub struct ControllerFocusTracker {
    widgets: Vec<Widget>,
    index: i32,
    css_class: String,
}

impl ControllerFocusTracker {
    pub fn new() -> Self {
        Self {
            widgets: Vec::new(),
            index: -1,
            css_class: "controller-focus".to_string(),
        }
    }

    #[allow(dead_code)]
    pub fn with_css_class(css_class: &str) -> Self {
        Self {
            widgets: Vec::new(),
            index: -1,
            css_class: css_class.to_string(),
        }
    }

    pub fn set_widgets(&mut self, widgets: Vec<Widget>) {
        let old = self.current_widget();
        self.clear_outline();
        self.widgets = widgets
            .into_iter()
            .filter(|w| w.is_sensitive())
            .collect();
        self.index = if let Some(ref old_w) = old {
            self.widgets
                .iter()
                .position(|w| w.as_ptr() == old_w.as_ptr())
                .map(|i| i as i32)
                .unwrap_or(-1)
        } else {
            -1
        };
    }

    pub fn current_widget(&self) -> Option<Widget> {
        if self.index >= 0 && (self.index as usize) < self.widgets.len() {
            Some(self.widgets[self.index as usize].clone())
        } else {
            None
        }
    }

    /// The tracked widgets in visual order (clone of the list).
    pub fn widgets(&self) -> Vec<Widget> {
        self.widgets.clone()
    }

    pub fn cycle(&mut self, delta: i32) {
        if self.widgets.is_empty() {
            return;
        }
        self.clear_outline();
        if self.index < 0 {
            self.index = if delta >= 0 {
                0
            } else {
                (self.widgets.len() - 1) as i32
            };
        } else {
            let len = self.widgets.len() as i32;
            self.index = (self.index + delta).rem_euclid(len);
        }
        self.apply_outline();
    }

    pub fn focus_first(&mut self) {
        if !self.widgets.is_empty() {
            self.clear_outline();
            self.index = 0;
            self.apply_outline();
        }
    }

    pub fn move_to_widget(&mut self, widget: &Widget) {
        if let Some(pos) = self
            .widgets
            .iter()
            .position(|w| w.as_ptr() == widget.as_ptr())
        {
            self.clear_outline();
            self.index = pos as i32;
            self.apply_outline();
        }
    }

    fn clear_outline(&self) {
        if let Some(w) = self.current_widget() {
            w.remove_css_class(&self.css_class);
        }
    }

    fn apply_outline(&self) {
        if let Some(w) = self.current_widget() {
            w.add_css_class(&self.css_class);
            w.grab_focus();
        }
    }
}

impl Default for ControllerFocusTracker {
    fn default() -> Self {
        Self::new()
    }
}

pub fn adjust_focused_widget(widget: &Widget, delta: i32) {
    use gtk4::prelude::*;

    if let Some(spin) = widget.downcast_ref::<libadwaita::SpinRow>() {
        let adj = spin.adjustment();
        let step = adj.step_increment();
        spin.set_value(spin.value() + step * delta as f64);
    } else if let Some(combo) = widget.downcast_ref::<libadwaita::ComboRow>() {
        if let Some(model) = combo.model() {
            let n = model.n_items();
            let current = combo.selected() as i32;
            let new_idx = current + delta;
            if new_idx >= 0 && (new_idx as u32) < n {
                combo.set_selected(new_idx as u32);
            }
        }
    } else if let Some(switch) = widget.downcast_ref::<libadwaita::SwitchRow>() {
        switch.set_active(!switch.is_active());
    }
}
