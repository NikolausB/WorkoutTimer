use crate::image_utils::{copy_user_image, load_all_exercises, resolve_image_path, ExerciseEntry};
use crate::user_exercises::add_user_exercise;
use gtk4::gdk;
use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Button, Image, Label, ListBox, Orientation, ScrolledWindow, SearchEntry};
use libadwaita::prelude::*;
use libadwaita::{ActionRow, Dialog, EntryRow};
use std::cell::RefCell;
use std::rc::Rc;

pub type OnSelected = Rc<dyn Fn(&str, Option<&str>)>;

pub struct ExercisePicker {
    dialog: Dialog,
    exercises: RefCell<Vec<ExerciseEntry>>,
    categories: RefCell<Vec<String>>,
    category_dropdown: gtk4::DropDown,
    on_selected: OnSelected,
    search_entry: SearchEntry,
    list_box: ListBox,
    focus_idx: std::cell::Cell<i32>,
    self_weak: RefCell<std::rc::Weak<ExercisePicker>>,
}

impl ExercisePicker {
    pub fn new<F: Fn(&str, Option<&str>) + 'static>(on_selected: F) -> Rc<Self> {
        let dialog = Dialog::new();
        dialog.set_title("Browse Exercises");
        dialog.set_content_width(500);
        dialog.set_content_height(600);
        dialog.set_follows_content_size(true);

        let box_widget = GtkBox::new(Orientation::Vertical, 12);
        box_widget.set_margin_top(12);
        box_widget.set_margin_bottom(12);
        box_widget.set_margin_start(12);
        box_widget.set_margin_end(12);

        let top_box = GtkBox::new(Orientation::Horizontal, 8);
        let search_entry = SearchEntry::new();
        search_entry.set_hexpand(true);
        top_box.append(&search_entry);

        let category_dropdown = gtk4::DropDown::new(None::<gtk4::StringList>, None::<gtk4::Expression>);
        category_dropdown.set_tooltip_text(Some("Filter by category"));
        top_box.append(&category_dropdown);

        let add_btn = Button::builder()
            .icon_name("list-add-symbolic")
            .css_classes(["suggested-action"])
            .build();
        top_box.append(&add_btn);
        box_widget.append(&top_box);

        let scrolled = ScrolledWindow::new();
        scrolled.set_vexpand(true);
        let list_box = ListBox::new();
        list_box.add_css_class("boxed-list");
        list_box.set_activate_on_single_click(true);
        scrolled.set_child(Some(&list_box));
        box_widget.append(&scrolled);

        dialog.set_child(Some(&box_widget));

        let picker = Rc::new(Self {
            dialog,
            exercises: RefCell::new(vec![]),
            categories: RefCell::new(vec![]),
            category_dropdown,
            on_selected: Rc::new(on_selected),
            search_entry,
            list_box,
            focus_idx: std::cell::Cell::new(-1),
            self_weak: RefCell::new(std::rc::Weak::new()),
        });
        *picker.self_weak.borrow_mut() = Rc::downgrade(&picker);

        // Load exercises
        let all = load_all_exercises();

        // Category filter model: "All Categories" + sorted unique categories
        {
            let mut cats: Vec<String> = all
                .iter()
                .filter_map(|e| e.category.clone())
                .collect();
            cats.sort();
            cats.dedup();
            let mut labels = vec!["All Categories".to_string()];
            labels.extend(cats.clone());
            let model = gtk4::StringList::new(
                &labels.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
            );
            picker.category_dropdown.set_model(Some(&model));
            picker.category_dropdown.set_selected(0);
            *picker.categories.borrow_mut() = cats;
        }

        *picker.exercises.borrow_mut() = all.clone();

        // Add rows
        for (i, ex) in all.iter().enumerate() {
            picker.add_exercise_row(ex, i);
        }

        // Search + category filter
        {
            let picker_weak = Rc::downgrade(&picker);
            picker.search_entry.connect_search_changed(move |_| {
                if let Some(p) = picker_weak.upgrade() {
                    p.apply_filters();
                }
            });
        }
        {
            let picker_weak = Rc::downgrade(&picker);
            picker
                .category_dropdown
                .connect_notify_local(Some("selected"), move |_, _| {
                    if let Some(p) = picker_weak.upgrade() {
                        p.apply_filters();
                    }
                });
        }

        // Row activation
        {
            let picker_weak = Rc::downgrade(&picker);
            picker.list_box.connect_row_activated(move |_lb, row| {
                let Some(p) = picker_weak.upgrade() else { return };
                let idx = row.index();
                let (name, image_key) = {
                    let exercises = p.exercises.borrow();
                    exercises
                        .get(idx as usize)
                        .map(|ex| (ex.name.clone(), ex.image_key.clone()))
                        .unwrap_or_default()
                };
                (p.on_selected)(&name, image_key.as_deref());
                p.dialog.close();
            });
        }

        // Add exercise button
        {
            let picker_weak = Rc::downgrade(&picker);
            add_btn.connect_clicked(move |_| {
                let Some(p) = picker_weak.upgrade() else { return };
                p.show_add_exercise_form();
            });
        }

        picker
    }

    #[allow(dead_code)]
    pub fn widget(&self) -> &Dialog {
        &self.dialog
    }

    /// Present the picker over the current window and register it for
    /// controller input (mirrors Python's `_register_controller_dialog`).
    /// The `closed` handler unregisters it again — otherwise the window
    /// would keep routing every gamepad action to the dead picker.
    pub fn present_over_window(self: &std::rc::Rc<Self>) {
        let Some(window) = crate::window::current_window() else {
            return;
        };
        window.register_exercise_picker_dialog(Some(self.clone()));
        {
            let window_weak = std::rc::Rc::downgrade(&window);
            self.dialog.connect_closed(move |_| {
                if let Some(w) = window_weak.upgrade() {
                    w.register_exercise_picker_dialog(None);
                }
            });
        }
        let gtk_win = window.gtk_window();
        self.dialog.present(Some(&gtk_win));
    }

    /// Re-apply search query + category filter to all rows.
    fn apply_filters(&self) {
        let query = self.search_entry.text().trim().to_lowercase();
        let sel = self.category_dropdown.selected() as usize;
        let categories = self.categories.borrow();
        let selected_cat: Option<&String> = if sel == 0 {
            None
        } else {
            categories.get(sel - 1)
        };

        let mut idx = 0;
        let exercises = self.exercises.borrow();
        while let Some(row) = self.list_box.row_at_index(idx) {
            let visible = match exercises.get(idx as usize) {
                None => true,
                Some(ex) => {
                    let matches_query = if query.is_empty() {
                        true
                    } else {
                        let title = ex.name.to_lowercase();
                        let subtitle = format!(
                            "{} | {}",
                            ex.primary_muscles.join(", "),
                            ex.equipment
                        )
                        .to_lowercase();
                        title.contains(&query) || subtitle.contains(&query)
                    };
                    let matches_cat = match selected_cat {
                        None => true,
                        Some(c) => ex.category.as_ref() == Some(c),
                    };
                    matches_query && matches_cat
                }
            };
            row.set_visible(visible);
            idx += 1;
        }
        self.focus_idx.set(-1);
    }

    fn add_exercise_row(self: &Rc<Self>, ex: &ExerciseEntry, list_index: usize) {
        let name = if ex.name.is_empty() {
            "Unknown".to_string()
        } else {
            ex.name.clone()
        };
        let muscles = ex.primary_muscles.join(", ");
        let equipment = ex.equipment.clone();

        let title = if ex.is_user {
            format!("{} (custom)", name)
        } else {
            name.clone()
        };

        let mut builder = ActionRow::builder().title(&title);
        let subtitle = if !muscles.is_empty() && !equipment.is_empty() {
            format!("{} | {}", muscles, equipment)
        } else if !muscles.is_empty() {
            muscles.clone()
        } else {
            equipment.clone()
        };
        if !subtitle.is_empty() {
            builder = builder.subtitle(&subtitle);
        }
        let row = builder.activatable(true).build();

        // Thumbnail
        if let Some(image_key) = &ex.image_key {
            if let Some(resolved) = resolve_image_path(&Some(image_key.clone())) {
                if let Ok(texture) = gdk::Texture::from_filename(&resolved) {
                    let img = Image::from_paintable(Some(&texture));
                    img.set_pixel_size(36);
                    img.set_margin_end(6);
                    row.add_prefix(&img);
                }
            }
        }

        // Custom exercises can be deleted (with undo).
        if ex.is_user {
            let user_index = self
                .exercises
                .borrow()
                .iter()
                .take(list_index)
                .filter(|e| e.is_user)
                .count();
            let del = Button::builder()
                .icon_name("user-trash-symbolic")
                .css_classes(["flat"])
                .tooltip_text("Delete custom exercise")
                .valign(gtk4::Align::Center)
                .build();
            {
                let page_weak = Rc::downgrade(self);
                del.connect_clicked(move |_| {
                    let Some(p) = page_weak.upgrade() else { return };
                    let Some(removed) = crate::user_exercises::delete_user_exercise(user_index)
                    else {
                        return;
                    };
                    p.reload_list();
                    crate::window::show_app_toast_action(
                        &format!("\"{}\" deleted", removed.name),
                        "Undo",
                        Rc::new(move || {
                            crate::user_exercises::insert_user_exercise(user_index, &removed);
                            crate::window::refresh_all_pages();
                        }),
                    );
                });
            }
            row.add_suffix(&del);
        }

        self.list_box.append(&row);
    }

    /// Rebuild the exercise list (after adding/removing custom exercises).
    fn reload_list(self: &Rc<Self>) {
        let mut child = self.list_box.first_child();
        while let Some(c) = child {
            child = c.next_sibling();
            self.list_box.remove(&c);
        }
        let all = load_all_exercises();
        {
            let mut cats: Vec<String> = Vec::new();
            for ex in &all {
                if let Some(c) = &ex.category {
                    if !cats.contains(c) {
                        cats.push(c.clone());
                    }
                }
            }
            cats.sort();
            let mut labels = vec!["All Categories".to_string()];
            labels.extend(cats.clone());
            let model = gtk4::StringList::new(
                &labels.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
            );
            self.category_dropdown.set_model(Some(&model));
            *self.categories.borrow_mut() = cats;
        }
        *self.exercises.borrow_mut() = all.clone();
        for (i, ex) in all.iter().enumerate() {
            self.add_exercise_row(ex, i);
        }
        self.apply_filters();
    }

    fn visible_rows(&self) -> Vec<gtk4::Widget> {
        let mut rows = Vec::new();
        let mut idx = 0;
        while let Some(row) = self.list_box.row_at_index(idx) {
            if row.is_visible() && row.is_child_visible() {
                rows.push(row.upcast());
            }
            idx += 1;
        }
        rows
    }

    fn picker_focus_cycle(&self, delta: i32) {
        let rows = self.visible_rows();
        if rows.is_empty() {
            return;
        }
        let idx = self.focus_idx.get();
        let old = if idx >= 0 && (idx as usize) < rows.len() {
            Some(rows[idx as usize].clone())
        } else {
            None
        };
        let next_idx = ((idx + delta).rem_euclid(rows.len() as i32)) as usize;
        self.focus_idx.set(next_idx as i32);
        if let Some(old) = old {
            if old.as_ptr() != rows[next_idx].as_ptr() {
                old.remove_css_class("controller-focus");
            }
        }
        rows[next_idx].add_css_class("controller-focus");
        rows[next_idx].grab_focus();
    }

    pub fn controller_dpad_up(&self) {
        self.picker_focus_cycle(-1);
    }

    pub fn controller_dpad_down(&self) {
        self.picker_focus_cycle(1);
    }

    pub fn controller_a(&self) {
        let rows = self.visible_rows();
        let idx = self.focus_idx.get();
        if idx >= 0 && (idx as usize) < rows.len() {
            if let Some(row) = rows[idx as usize].downcast_ref::<ActionRow>() {
                let row_idx = row.index();
                let (name, image_key) = {
                    let exercises = self.exercises.borrow();
                    exercises
                        .get(row_idx as usize)
                        .map(|ex| (ex.name.clone(), ex.image_key.clone()))
                        .unwrap_or_default()
                };
                (self.on_selected)(&name, image_key.as_deref());
                self.dialog.close();
            }
        }
    }

    pub fn controller_b(&self) {
        self.dialog.close();
    }

    pub(crate) fn show_add_exercise_form(self: &Rc<Self>) {
        let dialog = Dialog::new();
        dialog.set_title("Add Exercise");
        dialog.set_content_width(400);
        dialog.set_content_height(450);
        dialog.set_follows_content_size(true);

        let box_widget = GtkBox::new(Orientation::Vertical, 12);
        box_widget.set_margin_top(12);
        box_widget.set_margin_bottom(12);
        box_widget.set_margin_start(12);
        box_widget.set_margin_end(12);

        let group = libadwaita::PreferencesGroup::new();

        let name_row = EntryRow::builder().title("Exercise Name").build();
        group.add(&name_row);

        let muscles_row = EntryRow::builder().title("Primary Muscles").build();
        group.add(&muscles_row);

        let equip_row = EntryRow::builder().title("Equipment").build();
        group.add(&equip_row);

        // Clones for the gamepad focus tracker (the originals are moved
        // into the save handler below).
        let name_row_tracker = name_row.clone();
        let muscles_row_tracker = muscles_row.clone();
        let equip_row_tracker = equip_row.clone();

        box_widget.append(&group);

        let image_btn = Button::builder().label("Choose Image").build();
        let image_label = Label::new(Some("No image selected"));
        image_label.add_css_class("dim-label");

        let new_image_key: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));

        {
            let picker_weak = self.self_weak.borrow().clone();
            let new_image_key = Rc::clone(&new_image_key);
            let image_label_for_btn = image_label.clone();
            image_btn.connect_clicked(move |_| {
                let Some(p) = picker_weak.upgrade() else { return };
                let Some(root) = p.dialog.root().and_then(|r| r.dynamic_cast_ref::<gtk4::Window>().cloned()) else { return };
                crate::dialogs::open_dialog(
                    &root,
                    "Select Exercise Image",
                    Some(("image/*", "Images")),
                    {
                        let label = image_label_for_btn.clone();
                        let new_image_key = Rc::clone(&new_image_key);
                        move |path: Option<String>| {
                            if let Some(path) = path {
                                if let Some(key) = copy_user_image(&path) {
                                    *new_image_key.borrow_mut() = Some(key);
                                    label.set_label(
                                        &std::path::Path::new(&path)
                                            .file_name()
                                            .map(|s| s.to_string_lossy().to_string())
                                            .unwrap_or_default(),
                                    );
                                }
                            }
                        }
                    },
                );
            });
        }

        box_widget.append(&image_btn);
        box_widget.append(&image_label);

        let save_btn = Button::builder()
            .label("Save")
            .css_classes(["suggested-action"])
            .build();
        save_btn.set_margin_top(6);

        {
            let picker_weak = self.self_weak.borrow().clone();
            let new_image_key = Rc::clone(&new_image_key);
            let dialog_for_save = dialog.clone();
            save_btn.connect_clicked(move |_| {
                let Some(p) = picker_weak.upgrade() else { return };
                let name = name_row.text().trim().to_string();
                if name.is_empty() {
                    return;
                }
                let muscles_text = muscles_row.text().trim().to_string();
                let muscles: Vec<String> = if muscles_text.is_empty() {
                    Vec::new()
                } else {
                    muscles_text
                        .split(',')
                        .map(|m| m.trim().to_string())
                        .filter(|m| !m.is_empty())
                        .collect()
                };
                let equipment = if equip_row.text().trim().is_empty() {
                    "body only".to_string()
                } else {
                    equip_row.text().trim().to_string()
                };
                let entry = add_user_exercise(
                    &name,
                    muscles,
                    &equipment,
                    new_image_key.borrow().clone(),
                );
                let ex_entry = ExerciseEntry {
                    name: entry.name.clone(),
                    primary_muscles: entry.primary_muscles.clone(),
                    equipment: entry.equipment.clone().unwrap_or_default(),
                    category: None,
                    image_key: entry.image_path.clone(),
                    is_user: true,
                };
                p.exercises.borrow_mut().push(ex_entry.clone());
                let idx = p.exercises.borrow().len().saturating_sub(1);
                p.add_exercise_row(&ex_entry, idx);
                dialog_for_save.close();
            });
        }

        box_widget.append(&save_btn);

        dialog.set_child(Some(&box_widget));

        // Register the form so the gamepad can drive it (dpad cycles,
        // A activates, B closes). When it closes, the still-open picker
        // list takes input routing back.
        if let Some(window) = crate::window::current_window() {
            let mut tracker = crate::controller_focus::ControllerFocusTracker::new();
            tracker.set_widgets(vec![
                name_row_tracker.clone().upcast(),
                muscles_row_tracker.clone().upcast(),
                equip_row_tracker.clone().upcast(),
                image_btn.clone().upcast(),
                save_btn.clone().upcast(),
            ]);
            let tracker = Rc::new(RefCell::new(tracker));
            tracker.borrow_mut().focus_first();
            let controller = Rc::new(crate::round_timer::PresetsDialogController {
                dialog: dialog.clone(),
                tracker,
                hints: "form",
            });
            window.register_presets_dialog(Some(controller));
            let window_weak = Rc::downgrade(&window);
            let picker_weak = self.self_weak.borrow().clone();
            dialog.connect_closed(move |_| {
                if let Some(w) = window_weak.upgrade() {
                    w.register_presets_dialog(None);
                    if let Some(p) = picker_weak.upgrade() {
                        w.register_exercise_picker_dialog(Some(p));
                    }
                }
            });
        }

        let Some(root) = self.dialog.root().and_then(|r| r.dynamic_cast_ref::<gtk4::Window>().cloned()) else { return };
        dialog.present(Some(&root));
    }
}