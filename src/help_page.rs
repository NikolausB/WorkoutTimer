use crate::controller_hints::{hints_for, HINT_CONTEXTS, KEYBOARD_SHORTCUTS};
use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Orientation};
use libadwaita::prelude::*;
use libadwaita::{ActionRow, Clamp, PreferencesGroup};

/// Content of the help window: keyboard shortcuts and the gamepad layout for
/// every screen. The controller strings come from `controller_hints::hints_for`
/// so the hint bar and this page always match.
fn build_help_content() -> Clamp {
    let box_widget = GtkBox::new(Orientation::Vertical, 18);
    box_widget.set_margin_top(24);
    box_widget.set_margin_bottom(24);
    box_widget.set_margin_start(24);
    box_widget.set_margin_end(24);

    let intro = gtk4::Label::new(Some(
        "Everything you can do with the keyboard and a gamepad.",
    ));
    intro.add_css_class("dim-label");
    box_widget.append(&intro);

    // Keyboard
    let keys_group = PreferencesGroup::builder().title("Keyboard").build();
    for (key, description) in KEYBOARD_SHORTCUTS {
        let row = ActionRow::builder()
            .title(*key)
            .subtitle(*description)
            .build();
        keys_group.add(&row);
    }
    box_widget.append(&keys_group);

    // Gamepad
    let pad_group = PreferencesGroup::builder()
        .title("Gamepad")
        .description("Buttons change slightly per screen")
        .build();
    for (context, title) in HINT_CONTEXTS {
        let row = ActionRow::builder()
            .title(*title)
            .subtitle(hints_for(context, ""))
            .subtitle_lines(0)
            .build();
        pad_group.add(&row);
    }
    box_widget.append(&pad_group);

    let clamp = Clamp::new();
    clamp.set_child(Some(&box_widget));
    clamp
}

/// Standalone "Help & Shortcuts" window.
pub struct HelpWindow {
    window: libadwaita::Window,
}

impl HelpWindow {
    pub fn new(parent: &libadwaita::Window) -> Self {
        let window = libadwaita::Window::builder()
            .title("Help & Shortcuts")
            .transient_for(parent)
            .modal(false)
            .default_width(560)
            .default_height(640)
            .build();

        let header = libadwaita::HeaderBar::new();
        let close_btn = gtk4::Button::builder()
            .label("Close")
            .css_classes(["suggested-action"])
            .build();
        header.pack_end(&close_btn);

        let toolbar = libadwaita::ToolbarView::new();
        toolbar.add_top_bar(&header);
        toolbar.set_content(Some(&build_help_content()));

        window.set_content(Some(&toolbar));

        {
            let window_clone = window.clone();
            close_btn.connect_clicked(move |_| {
                window_clone.close();
            });
        }

        Self { window }
    }

    pub fn present(&self) {
        self.window.present();
    }

    pub fn close(&self) {
        self.window.close();
    }

    pub fn is_open(&self) -> bool {
        self.window.is_visible()
    }
}
