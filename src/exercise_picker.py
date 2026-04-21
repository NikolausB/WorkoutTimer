import os
from gi.repository import Adw, Gtk, Gdk, GLib
from image_utils import load_all_exercises, resolve_image_path, copy_user_image
from user_exercises import add_user_exercise


class ExercisePicker(Adw.Dialog):
    def __init__(self, on_selected, **kwargs):
        super().__init__(**kwargs)
        self._on_selected = on_selected
        self._exercises = load_all_exercises()
        self._build_ui()

    def _build_ui(self):
        self.set_title("Browse Exercises")
        self.set_content_width(500)
        self.set_content_height(600)

        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
        box.set_margin_top(12)
        box.set_margin_bottom(12)
        box.set_margin_start(12)
        box.set_margin_end(12)

        top_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        self._search_entry = Gtk.SearchEntry()
        self._search_entry.set_hexpand(True)
        self._search_entry.connect("search-changed", self._on_search_changed)
        top_box.append(self._search_entry)

        add_btn = Gtk.Button(icon_name="list-add-symbolic", css_classes=["suggested-action"])
        add_btn.connect("clicked", self._on_add_clicked)
        top_box.append(add_btn)
        box.append(top_box)

        scrolled = Gtk.ScrolledWindow(vexpand=True)

        self._list_box = Gtk.ListBox(css_classes=["boxed-list"])
        self._list_box.set_activate_on_single_click(True)
        self._list_box.set_filter_func(self._filter_func)
        self._list_box.connect("row-activated", self._on_row_activated)
        scrolled.set_child(self._list_box)
        box.append(scrolled)

        for ex in self._exercises:
            self._add_exercise_row(ex)

        self.set_child(box)

    def _add_exercise_row(self, exercise: dict):
        name = exercise.get("name", "Unknown")
        image_key = exercise.get("image_key")
        muscles = ", ".join(exercise.get("primaryMuscles", []))
        equipment = exercise.get("equipment", "")
        is_user = exercise.get("source") == "user"

        row = Adw.ActionRow(title=name)
        if is_user:
            row.set_title(f"{name} (custom)")
        if muscles:
            row.set_subtitle(f"{muscles} | {equipment}" if equipment else muscles)
        elif equipment:
            row.set_subtitle(equipment)
        row.set_activatable(True)

        thumb = self._load_thumbnail(image_key)
        if thumb:
            row.add_prefix(thumb)

        row._exercise_data = exercise
        row._image_key = image_key
        self._list_box.append(row)

    def _load_thumbnail(self, image_key):
        if not image_key:
            return None
        resolved = resolve_image_path(image_key)
        if not resolved:
            return None
        try:
            texture = Gdk.Texture.new_from_filename(resolved)
            img = Gtk.Image.new_from_paintable(texture)
            img.set_pixel_size(36)
            img.set_margin_end(6)
            return img
        except Exception:
            return None

    def _filter_func(self, row):
        query = self._search_entry.get_text().strip().lower()
        if not query:
            return True
        text = row.get_title().lower()
        subtitle = row.get_subtitle().lower() if row.get_subtitle() else ""
        return query in text or query in subtitle

    def _on_search_changed(self, entry):
        self._list_box.invalidate_filter()

    def _on_row_activated(self, list_box, row):
        exercise_data = getattr(row, "_exercise_data", None)
        image_key = getattr(row, "_image_key", None)
        if exercise_data:
            name = exercise_data.get("name", "")
            self._on_selected(name, image_key)
        self.close()

    def _on_add_clicked(self, btn):
        self._show_add_exercise_form()

    def _show_add_exercise_form(self):
        dialog = Adw.Dialog()
        dialog.set_title("Add Exercise")
        dialog.set_content_width(400)
        dialog.set_content_height(450)
        dialog.set_follows_content_size(True)

        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
        box.set_margin_top(12)
        box.set_margin_bottom(12)
        box.set_margin_start(12)
        box.set_margin_end(12)

        group = Adw.PreferencesGroup()

        name_row = Adw.EntryRow(title="Exercise Name")
        group.add(name_row)

        muscles_row = Adw.EntryRow(title="Primary Muscles")
        muscles_row.set_subtitle("e.g. abdominals, quadriceps")
        group.add(muscles_row)

        equip_row = Adw.EntryRow(title="Equipment")
        equip_row.set_subtitle("e.g. body only, dumbbell, kettlebells")
        group.add(equip_row)

        box.append(group)

        image_btn = Gtk.Button(label="Choose Image")
        self._new_image_key = None
        image_label = Gtk.Label(label="No image selected", css_classes=["dim-label"])

        image_btn.connect("clicked", lambda _: self._on_pick_new_exercise_image(image_label))
        box.append(image_btn)
        box.append(image_label)

        save_btn = Gtk.Button(label="Save", css_classes=["suggested-action"])
        save_btn.set_margin_top(6)

        def on_save(_):
            name = name_row.get_text().strip()
            if not name:
                return
            muscles_text = muscles_row.get_text().strip()
            muscles = [m.strip() for m in muscles_text.split(",") if m.strip()] if muscles_text else []
            equipment = equip_row.get_text().strip() or "body only"
            entry = add_user_exercise(name, muscles, equipment, self._new_image_key)
            self._exercises.append(entry)
            row = self._add_exercise_row(entry)
            dialog.close()

        save_btn.connect("clicked", on_save)
        box.append(save_btn)

        dialog.set_child(box)
        dialog.present(self.get_root())

    def _on_pick_new_exercise_image(self, label):
        chooser = Gtk.FileChooserNative(
            title="Select Exercise Image",
            action=Gtk.FileChooserAction.OPEN,
        )
        image_filter = Gtk.FileFilter()
        image_filter.set_name("Images")
        image_filter.add_mime_type("image/*")
        chooser.add_filter(image_filter)
        chooser.connect("response", lambda _, response: self._on_new_image_selected(chooser, response, label))
        chooser.show()

    def _on_new_image_selected(self, chooser, response, label):
        if response == Gtk.ResponseType.ACCEPT:
            file = chooser.get_file()
            if file:
                path = file.get_path()
                if path:
                    key = copy_user_image(path)
                    self._new_image_key = key
                    label.set_label(os.path.basename(path))
        chooser.destroy()