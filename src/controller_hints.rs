use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Label, Orientation};
use std::collections::HashMap;

fn build_hints() -> HashMap<String, HintValue> {
    let mut m = HashMap::new();

    let mut runner = HashMap::new();
    runner.insert(
        "exercise".to_string(),
        "[A] Done  [Y] Skip  [B] Back  [Start] Pause  [Guide] Fullscreen".to_string(),
    );
    runner.insert(
        "rest".to_string(),
        "[A] Done  [Y] Skip Rest  [Start] Pause  [Guide] Fullscreen".to_string(),
    );
    runner.insert(
        "timed".to_string(),
        "[Start] Pause/Resume  [B] Back  [Guide] Fullscreen".to_string(),
    );
    m.insert("runner_exercise".to_string(), HintValue::Sub(runner));

    m.insert(
        "runner_summary".to_string(),
        HintValue::Simple("[A] Back to Menu  [Guide] Fullscreen".to_string()),
    );
    m.insert(
        "timer".to_string(),
        HintValue::Simple(
            "[A] Start  [Start] Pause  [B] Skip  [X] Reset  [L1/R1] Tabs  [Guide] Fullscreen"
                .to_string(),
        ),
    );
    m.insert(
        "list".to_string(),
        HintValue::Simple("[A] Open  [X] Favorite  [B] Back  [L1/R1] Tabs  [Guide] Fullscreen".to_string()),
    );
    m.insert(
        "editor".to_string(),
        HintValue::Simple(
            "[A] Enter  [B] Back  [←/→] Adjust  [X] Toggle  [Start] Start Training  [L1/R1] Tabs  [Guide] Fullscreen"
                .to_string(),
        ),
    );
    m.insert(
        "ai_coach".to_string(),
        HintValue::Simple(
            "[A] Submit  [B] Back  [←/→] Toggle  [Select] Keyboard  [L1/R1] Tabs  [Guide] Fullscreen"
                .to_string(),
        ),
    );
    m.insert(
        "keyboard".to_string(),
        HintValue::Simple("[A] Type  [B] Backspace  [Y] Shift  [←/→] Move  [Start] Close".to_string()),
    );

    m
}

enum HintValue {
    Simple(String),
    Sub(HashMap<String, String>),
}

pub struct ControllerHintsOverlay {
    box_widget: GtkBox,
    label: Label,
    hints: HashMap<String, HintValue>,
    context: Option<String>,
    sub_key: String,
}

impl ControllerHintsOverlay {
    pub fn new() -> Self {
        let box_widget = GtkBox::new(Orientation::Horizontal, 18);
        let label = Label::new(None);
        label.add_css_class("controller-hints-label");
        box_widget.append(&label);
        box_widget.set_visible(false);
        box_widget.set_halign(Align::End);
        box_widget.set_valign(Align::End);
        box_widget.set_margin_bottom(8);

        Self {
            box_widget,
            label,
            hints: build_hints(),
            context: None,
            sub_key: String::new(),
        }
    }

    pub fn widget(&self) -> &GtkBox {
        &self.box_widget
    }

    pub fn set_context(&mut self, key: &str) {
        self.context = Some(key.to_string());
        self.update_text();
    }

    pub fn set_sub_key(&mut self, sub_key: &str) {
        self.sub_key = sub_key.to_string();
        self.update_text();
    }

    #[allow(dead_code)]
    pub fn show(&self) {
        self.box_widget.set_visible(true);
    }

    pub fn hide(&self) {
        self.box_widget.set_visible(false);
    }

    fn update_text(&mut self) {
        let text = if let Some(ctx) = &self.context {
            match self.hints.get(ctx) {
                Some(HintValue::Simple(t)) => t.clone(),
                Some(HintValue::Sub(map)) => map.get(&self.sub_key).cloned().unwrap_or_default(),
                None => String::new(),
            }
        } else {
            String::new()
        };

        self.label.set_label(&text);
        self.box_widget.set_visible(!text.is_empty());
    }
}

impl Default for ControllerHintsOverlay {
    fn default() -> Self {
        Self::new()
    }
}
