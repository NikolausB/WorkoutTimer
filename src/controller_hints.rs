use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Label, Orientation};

/// Controller hint text for a context (and optional sub-context).
///
/// This is the single source of truth: the hint bar at the bottom of the
/// window and the help page both render these strings, so they can never
/// drift apart.
pub fn hints_for(context: &str, sub_key: &str) -> String {
    match context {
        "runner_exercise" => match sub_key {
            "rest" => "[A] Activate  [Y] Skip Rest  [Start] Pause  [Guide] Fullscreen".to_string(),
            "timed" => "[Start] Pause/Resume  [A] Activate  [B] End  [Guide] Fullscreen".to_string(),
            _ => "[A] Activate  [Y] Skip  [B] End  [Start] Pause  [Guide] Fullscreen".to_string(),
        },
        "runner_summary" => "[A] Back to Menu  [Guide] Fullscreen".to_string(),
        "timer" => {
            "[A] Activate  [Start] Start/Pause  [B] Skip  [X] Reset  [L1/R1] Tabs  [Guide] Fullscreen"
                .to_string()
        }
        "presets" => {
            "[A] Apply  [B] Back  [Start] Start/Pause  [L1/R1] Tabs  [Guide] Fullscreen".to_string()
        }
        "list" => "[A] Open  [X] Favorite  [B] Back  [L1/R1] Tabs  [Guide] Fullscreen".to_string(),
        "picker" => "[A] Select  [B] Back  [Guide] Fullscreen".to_string(),
        "form" => "[D-Pad] Navigate  [A] Activate  [B] Cancel  [Guide] Fullscreen".to_string(),
        "alert" => "[A] Confirm  [B] Cancel  [Guide] Fullscreen".to_string(),
        "editor" => {
            "[A] Enter  [B] Back  [←/→] Adjust  [X] Toggle  [Start] Start Training  [L1/R1] Tabs  [Guide] Fullscreen"
                .to_string()
        }
        "ai_coach" => {
            "[A] Submit  [B] Back  [←/→] Toggle  [Select] Keyboard  [L1/R1] Tabs  [Guide] Fullscreen"
                .to_string()
        }
        "keyboard" => {
            "[A] Type  [B] Backspace  [Y] Shift  [←/→] Move  [Start] Close".to_string()
        }
        _ => String::new(),
    }
}

/// All controller contexts, in the order the help page lists them.
pub const HINT_CONTEXTS: &[(&str, &str)] = &[
    ("timer", "Round timer"),
    ("presets", "Timer presets dialog"),
    ("list", "Training plans"),
    ("editor", "Plan editor"),
    ("runner_exercise", "Workout runner"),
    ("runner_summary", "Workout summary"),
    ("picker", "Exercise picker"),
    ("form", "Dialogs with text fields"),
    ("alert", "Confirmation dialogs"),
    ("ai_coach", "AI coach"),
    ("keyboard", "On-screen keyboard"),
];

/// Keyboard shortcuts shown in the help page.
pub const KEYBOARD_SHORTCUTS: &[(&str, &str)] = &[
    ("Space", "Start / pause the timer or workout"),
    ("S", "Skip the current round or exercise"),
    ("R", "Reset the round timer"),
    ("?", "Open this help page"),
    ("Ctrl + ,", "Open preferences"),
    ("Esc", "Close dialogs and overlays"),
];

pub struct ControllerHintsOverlay {
    box_widget: GtkBox,
    label: Label,
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
        let text = match &self.context {
            Some(ctx) => hints_for(ctx, &self.sub_key),
            None => String::new(),
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
