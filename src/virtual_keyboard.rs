use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Button, Grid, Label, Orientation};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq)]
enum KeyType {
    Char,
    Shift,
    Backspace,
    Enter,
    Space,
    Close,
}

struct KeyDef {
    label: String,
    key_type: KeyType,
}

fn layout() -> Vec<Vec<KeyDef>> {
    vec![
        vec![
            KeyDef { label: "1".into(), key_type: KeyType::Char },
            KeyDef { label: "2".into(), key_type: KeyType::Char },
            KeyDef { label: "3".into(), key_type: KeyType::Char },
            KeyDef { label: "4".into(), key_type: KeyType::Char },
            KeyDef { label: "5".into(), key_type: KeyType::Char },
            KeyDef { label: "6".into(), key_type: KeyType::Char },
            KeyDef { label: "7".into(), key_type: KeyType::Char },
            KeyDef { label: "8".into(), key_type: KeyType::Char },
            KeyDef { label: "9".into(), key_type: KeyType::Char },
            KeyDef { label: "0".into(), key_type: KeyType::Char },
        ],
        vec![
            KeyDef { label: "q".into(), key_type: KeyType::Char },
            KeyDef { label: "w".into(), key_type: KeyType::Char },
            KeyDef { label: "e".into(), key_type: KeyType::Char },
            KeyDef { label: "r".into(), key_type: KeyType::Char },
            KeyDef { label: "t".into(), key_type: KeyType::Char },
            KeyDef { label: "y".into(), key_type: KeyType::Char },
            KeyDef { label: "u".into(), key_type: KeyType::Char },
            KeyDef { label: "i".into(), key_type: KeyType::Char },
            KeyDef { label: "o".into(), key_type: KeyType::Char },
            KeyDef { label: "p".into(), key_type: KeyType::Char },
        ],
        vec![
            KeyDef { label: "a".into(), key_type: KeyType::Char },
            KeyDef { label: "s".into(), key_type: KeyType::Char },
            KeyDef { label: "d".into(), key_type: KeyType::Char },
            KeyDef { label: "f".into(), key_type: KeyType::Char },
            KeyDef { label: "g".into(), key_type: KeyType::Char },
            KeyDef { label: "h".into(), key_type: KeyType::Char },
            KeyDef { label: "j".into(), key_type: KeyType::Char },
            KeyDef { label: "k".into(), key_type: KeyType::Char },
            KeyDef { label: "l".into(), key_type: KeyType::Char },
            KeyDef { label: "⌫".into(), key_type: KeyType::Backspace },
        ],
        vec![
            KeyDef { label: "Shift".into(), key_type: KeyType::Shift },
            KeyDef { label: "z".into(), key_type: KeyType::Char },
            KeyDef { label: "x".into(), key_type: KeyType::Char },
            KeyDef { label: "c".into(), key_type: KeyType::Char },
            KeyDef { label: "v".into(), key_type: KeyType::Char },
            KeyDef { label: "b".into(), key_type: KeyType::Char },
            KeyDef { label: "n".into(), key_type: KeyType::Char },
            KeyDef { label: "m".into(), key_type: KeyType::Char },
            KeyDef { label: ",".into(), key_type: KeyType::Char },
            KeyDef { label: "↵".into(), key_type: KeyType::Enter },
        ],
        vec![
            KeyDef { label: "Space".into(), key_type: KeyType::Space },
            KeyDef { label: "".into(), key_type: KeyType::Char },
            KeyDef { label: "".into(), key_type: KeyType::Char },
            KeyDef { label: "".into(), key_type: KeyType::Char },
            KeyDef { label: "".into(), key_type: KeyType::Char },
            KeyDef { label: "".into(), key_type: KeyType::Char },
            KeyDef { label: "".into(), key_type: KeyType::Char },
            KeyDef { label: "Close".into(), key_type: KeyType::Close },
            KeyDef { label: "".into(), key_type: KeyType::Char },
            KeyDef { label: "".into(), key_type: KeyType::Char },
        ],
    ]
}

pub struct VirtualKeyboard {
    widget: GtkBox,
    buttons: Rc<RefCell<HashMap<(usize, usize), Button>>>,
    #[allow(dead_code)]
    key_types: Rc<RefCell<HashMap<(usize, usize), KeyType>>>,
    cursor_x: Rc<Cell<usize>>,
    cursor_y: Rc<Cell<usize>>,
    upper: Rc<Cell<bool>>,
    shift_locked: Rc<Cell<bool>>,
    on_text_typed: Rc<RefCell<Box<dyn Fn(&str)>>>,
}

impl VirtualKeyboard {
    #[allow(dead_code)]
    pub fn new<F: Fn(&str) + 'static>(on_text_typed: F) -> Self {
        Self::with_callback(Box::new(on_text_typed))
    }

    pub fn with_callback(callback: Box<dyn Fn(&str)>) -> Self {
        let widget = GtkBox::new(Orientation::Vertical, 8);

        let title = Label::new(Some("Virtual Keyboard"));
        title.add_css_class("title-2");
        title.set_margin_bottom(4);
        widget.append(&title);

        let grid = Grid::new();
        grid.set_row_spacing(6);
        grid.set_column_spacing(6);
        grid.set_halign(Align::Center);
        widget.append(&grid);

        let buttons = Rc::new(RefCell::new(HashMap::new()));
        let key_types = Rc::new(RefCell::new(HashMap::new()));
        let cursor_x = Rc::new(Cell::new(0usize));
        let cursor_y = Rc::new(Cell::new(0usize));
        let upper = Rc::new(Cell::new(false));
        let shift_locked = Rc::new(Cell::new(false));
        let on_text_typed: Rc<RefCell<Box<dyn Fn(&str)>>> = Rc::new(RefCell::new(callback));

        let layout = layout();

        for (row_idx, row_keys) in layout.iter().enumerate() {
            for (col_idx, key_def) in row_keys.iter().enumerate() {
                if key_def.key_type == KeyType::Char && key_def.label.is_empty() {
                    continue;
                }

                let mut css_classes = vec!["keyboard-key".to_string()];
                if key_def.key_type != KeyType::Char {
                    css_classes.push("special".to_string());
                }
                if key_def.key_type == KeyType::Space {
                    css_classes.push("space-key".to_string());
                }

                let btn = Button::builder()
                    .label(&key_def.label)
                    .css_classes(css_classes.iter().map(|s| s.as_str()).collect::<Vec<&str>>().as_slice())
                    .hexpand(true)
                    .build();
                btn.set_focusable(true);
                btn.set_can_focus(true);

                let width: usize = match key_def.key_type {
                    KeyType::Space => 7,
                    KeyType::Close => 3,
                    _ => 1,
                };

                grid.attach(&btn, col_idx as i32, row_idx as i32, width as i32, 1);

                for w in 0..width {
                    buttons.borrow_mut().insert((row_idx, col_idx + w), btn.clone());
                }
                key_types
                    .borrow_mut()
                    .insert((row_idx, col_idx), key_def.key_type);

                {
                    let on_text_typed = Rc::clone(&on_text_typed);
                    let upper = Rc::clone(&upper);
                    let shift_locked = Rc::clone(&shift_locked);
                    let buttons = Rc::clone(&buttons);
                    let key_types = Rc::clone(&key_types);
                    let widget_clone = widget.clone();

                    btn.connect_clicked(move |b| {
                        let label = b.label().unwrap_or_default().to_string();

                        let mut typed_text: Option<String> = None;

                        if let Some((row, col)) = find_key_pos(&buttons, b) {
                            let key_type = key_types
                                .borrow()
                                .get(&(row, col))
                                .copied()
                                .unwrap_or(KeyType::Char);

                            match key_type {
                                KeyType::Shift => {
                                    shift_locked.set(!shift_locked.get());
                                    upper.set(shift_locked.get());
                                    update_shift_labels(&buttons, &upper, &shift_locked);
                                    return;
                                }
                                KeyType::Backspace => {
                                    typed_text = Some("\u{8}".to_string());
                                }
                                KeyType::Enter => {
                                    typed_text = Some("\n".to_string());
                                }
                                KeyType::Close => {
                                    widget_clone.set_visible(false);
                                    return;
                                }
                                KeyType::Space => {
                                    typed_text = Some(" ".to_string());
                                }
                                KeyType::Char => {
                                    let text = if upper.get() {
                                        label.to_uppercase()
                                    } else {
                                        label.clone()
                                    };
                                    if !shift_locked.get() {
                                        upper.set(false);
                                        update_shift_labels(&buttons, &upper, &shift_locked);
                                    }
                                    typed_text = Some(text);
                                }
                            }
                        }

                        if let Some(text) = typed_text {
                            let cb_val = on_text_typed.borrow(); {
                                cb_val(&text);
                            }
                        }
                    });
                }
            }
        }

        let kb = Self {
            widget,
            buttons,
            key_types,
            cursor_x,
            cursor_y,
            upper,
            shift_locked,
            on_text_typed,
        };

        kb.set_initial_cursor();

        kb
    }

    pub fn widget(&self) -> &GtkBox {
        &self.widget
    }

    #[allow(dead_code)]
    pub fn set_callback(&self, cb: Box<dyn Fn(&str)>) {
        *self.on_text_typed.borrow_mut() = cb;
    }

    pub fn on_dpad(&self, direction: &str) {
        let rows = 5;
        let cols = 10;
        match direction {
            "dpad_up" => self.cursor_y.set(self.cursor_y.get().saturating_sub(1)),
            "dpad_down" => self.cursor_y.set((self.cursor_y.get() + 1).min(rows - 1)),
            "dpad_left" => self.cursor_x.set(self.cursor_x.get().saturating_sub(1)),
            "dpad_right" => self.cursor_x.set((self.cursor_x.get() + 1).min(cols - 1)),
            _ => {}
        }
        self.update_cursor();
    }

    pub fn on_confirm(&self) {
        if let Some(btn) = self.find_button(self.cursor_x.get(), self.cursor_y.get()) {
            btn.activate();
        }
    }

    pub fn on_backspace(&self) {
        let cb_val = self.on_text_typed.borrow(); {
            cb_val("\u{8}");
        }
    }

    pub fn on_toggle_shift(&self) {
        self.shift_locked.set(!self.shift_locked.get());
        self.upper.set(self.shift_locked.get());
        update_shift_labels(&self.buttons, &self.upper, &self.shift_locked);
    }

    #[allow(dead_code)]
    pub fn on_submit(&self) {
        let cb_val = self.on_text_typed.borrow(); {
            cb_val("\n");
        }
    }

    fn set_initial_cursor(&self) {
        for col in 0..10 {
            if self.find_button(col, 0).is_some() {
                self.cursor_x.set(col);
                self.cursor_y.set(0);
                break;
            }
        }
        self.update_cursor();
    }

    fn find_button(&self, x: usize, y: usize) -> Option<Button> {
        self.buttons.borrow().get(&(y, x)).cloned()
    }

    fn update_cursor(&self) {
        for btn in self.buttons.borrow().values() {
            btn.remove_css_class("selected");
        }
        if let Some(btn) = self.find_button(self.cursor_x.get(), self.cursor_y.get()) {
            btn.add_css_class("selected");
            btn.grab_focus();
        }
    }
}

fn find_key_pos(
    buttons: &Rc<RefCell<HashMap<(usize, usize), Button>>>,
    target: &Button,
) -> Option<(usize, usize)> {
    for (pos, btn) in buttons.borrow().iter() {
        if btn.as_ptr() == target.as_ptr() {
            return Some(*pos);
        }
    }
    None
}

fn update_shift_labels(
    buttons: &Rc<RefCell<HashMap<(usize, usize), Button>>>,
    upper: &Rc<Cell<bool>>,
    shift_locked: &Rc<Cell<bool>>,
) {
    let layout = layout();
    for (row_idx, row_keys) in layout.iter().enumerate() {
        for (col_idx, key_def) in row_keys.iter().enumerate() {
            if key_def.key_type != KeyType::Char || key_def.label.is_empty() {
                continue;
            }
            if let Some(btn) = buttons.borrow().get(&(row_idx, col_idx)) {
                let display = if upper.get() {
                    key_def.label.to_uppercase()
                } else {
                    key_def.label.clone()
                };
                btn.set_label(&display);
            }
        }
    }

    if let Some(shift_btn) = buttons.borrow().get(&(3, 0)) {
        let label = if upper.get() || shift_locked.get() {
            "SHIFT"
        } else {
            "Shift"
        };
        shift_btn.set_label(label);
    }
}
