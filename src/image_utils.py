import json
import os
import shutil
from gi.repository import GLib, Gdk, Gtk


_BUNDLED_EXERCISES_DIR = os.path.join(os.path.dirname(os.path.abspath(__file__)), "resources", "exercises")
_FLATPAK_EXERCISES_DIR = "/app/share/training/resources/exercises"
_USER_IMAGES_DIR = os.path.join(GLib.get_user_data_dir(), "training-flatpak", "images")

_bundled_index: list[dict] | None = None


def get_exercises_dir() -> str:
    if os.path.exists(_FLATPAK_EXERCISES_DIR):
        return _FLATPAK_EXERCISES_DIR
    return _BUNDLED_EXERCISES_DIR


def get_user_images_dir() -> str:
    os.makedirs(_USER_IMAGES_DIR, exist_ok=True)
    return _USER_IMAGES_DIR


def load_bundled_exercises() -> list[dict]:
    global _bundled_index
    if _bundled_index is not None:
        return _bundled_index
    path = os.path.join(get_exercises_dir(), "exercises.json")
    if not os.path.exists(path):
        _bundled_index = []
        return _bundled_index
    with open(path, "r") as f:
        _bundled_index = json.load(f)
    return _bundled_index


def resolve_image_path(image_path: str | None) -> str | None:
    if image_path is None:
        return None
    if image_path.startswith("bundled:"):
        key = image_path[len("bundled:"):]
        dirpath = os.path.join(get_exercises_dir(), "images", key)
        img_path = os.path.join(dirpath, "0.jpg")
        if os.path.exists(img_path):
            return img_path
        return None
    if image_path.startswith("user:"):
        filename = image_path[len("user:"):]
        user_path = os.path.join(get_user_images_dir(), filename)
        if os.path.exists(user_path):
            return user_path
        return None
    if os.path.isabs(image_path) and os.path.exists(image_path):
        return image_path
    return None


def copy_user_image(source_path: str) -> str:
    os.makedirs(_USER_IMAGES_DIR, exist_ok=True)
    filename = os.path.basename(source_path)
    dest = os.path.join(_USER_IMAGES_DIR, filename)
    base, ext = os.path.splitext(filename)
    counter = 1
    while os.path.exists(dest):
        dest = os.path.join(_USER_IMAGES_DIR, f"{base}_{counter}{ext}")
        counter += 1
    shutil.copy2(source_path, dest)
    return f"user:{os.path.basename(dest)}"


def load_image_widget(image_path: str | None, size: int = 200) -> Gtk.Picture | None:
    resolved = resolve_image_path(image_path)
    if resolved is None:
        return None
    try:
        texture = Gdk.Texture.new_from_filename(resolved)
        pic = Gtk.Picture.new_for_paintable(texture)
        pic.set_size_request(size, size)
        pic.set_content_fit(Gtk.ContentFit.CONTAIN)
        pic.set_halign(Gtk.Align.CENTER)
        return pic
    except Exception:
        return None


def load_thumbnail_widget(image_path: str | None, size: int = 48) -> Gtk.Picture | None:
    return load_image_widget(image_path, size)


def load_all_exercises() -> list[dict]:
    from user_exercises import load_user_exercises
    bundled = load_bundled_exercises()
    for ex in bundled:
        ex["source"] = "bundled"
        images = ex.get("images", [])
        if images:
            folder = images[0].split("/")[0]
            ex["image_key"] = f"bundled:{folder}"
        else:
            ex["image_key"] = None
    user = load_user_exercises()
    for ex in user:
        ex["image_key"] = ex.get("image_path")
    return bundled + user


def get_plans_dir() -> str:
    local = os.path.join(os.path.dirname(os.path.abspath(__file__)), "resources", "plans")
    flatpak = "/app/share/training/resources/plans"
    if os.path.exists(flatpak):
        return flatpak
    return local