use gtk4::prelude::*;
use libadwaita::prelude::*;

/// Open a save dialog and invoke `on_path` with the chosen path on the main
/// thread. Replaces the deprecated GtkFileChooserNative with GtkFileDialog.
pub fn save_dialog<F: FnOnce(Option<String>) + 'static>(
    parent: &gtk4::Window,
    title: &str,
    default_name: &str,
    pattern: &str,
    pattern_name: &str,
    on_path: F,
) {
    let dialog = gtk4::FileDialog::new();
    dialog.set_title(title);
    dialog.set_initial_name(Some(default_name));

    let filters = gtk4::gio::ListStore::new::<gtk4::FileFilter>();
    let filter = gtk4::FileFilter::new();
    filter.add_pattern(pattern);
    filter.set_name(Some(pattern_name));
    filters.append(&filter);
    dialog.set_filters(Some(&filters));

    dialog.save(
        Some(parent),
        None::<&gtk4::gio::Cancellable>,
        move |result| {
            let path = match result {
                Ok(file) => file.path().map(|p| p.to_string_lossy().to_string()),
                Err(_) => None,
            };
            on_path(path);
        },
    );
}

/// Open a file-open dialog.
pub fn open_dialog<F: FnOnce(Option<String>) + 'static>(
    parent: &gtk4::Window,
    title: &str,
    pattern: Option<(&str, &str)>,
    on_path: F,
) {
    let dialog = gtk4::FileDialog::new();
    dialog.set_title(title);

    if let Some((pattern, pattern_name)) = pattern {
        let filters = gtk4::gio::ListStore::new::<gtk4::FileFilter>();
        let filter = gtk4::FileFilter::new();
        filter.add_pattern(pattern);
        filter.set_name(Some(pattern_name));
        filters.append(&filter);
        dialog.set_filters(Some(&filters));
    }

    dialog.open(
        Some(parent),
        None::<&gtk4::gio::Cancellable>,
        move |result| {
            let path = match result {
                Ok(file) => file.path().map(|p| p.to_string_lossy().to_string()),
                Err(_) => None,
            };
            on_path(path);
        },
    );
}

pub fn show_error_dialog(heading: &str, body: &str, parent: &impl IsA<gtk4::Widget>) {
    let dialog = libadwaita::AlertDialog::builder()
        .heading(heading)
        .body(body)
        .build();
    dialog.add_response("ok", "OK");
    dialog.present(Some(parent));
}


/// Send a desktop notification through the application (uses the notification
/// portal under Flatpak). No-op if the app isn't registered.
pub fn send_notification(root: &gtk4::Window, title: &str, body: &str) {
    let Some(app) = root.application() else { return };
    let notification = gtk4::gio::Notification::new(title);
    notification.set_body(Some(body));
    notification.set_icon(&gtk4::gio::ThemedIcon::new("io.github.NikolausB.WorkoutTimer"));
    app.send_notification(Some("workout-timer-complete"), &notification);
}
