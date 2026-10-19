use gtk4::glib;
use gtk4::glib::prelude::*;
use gtk4::glib::subclass::prelude::*;
use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

const EV_KEY: u16 = 0x01;
const EV_ABS: u16 = 0x03;

const ABS_X: u16 = 0x00;
const ABS_Y: u16 = 0x01;
const ABS_HAT0X: u16 = 0x10;
const ABS_HAT0Y: u16 = 0x11;

const BTN_A: u16 = 0x130;
const BTN_B: u16 = 0x131;
const BTN_X: u16 = 0x133;
const BTN_Y: u16 = 0x134;
const BTN_TL: u16 = 0x136;
const BTN_TR: u16 = 0x137;
const BTN_TL2: u16 = 0x138;
const BTN_TR2: u16 = 0x139;
const BTN_SELECT: u16 = 0x13A;
const BTN_START: u16 = 0x13B;
const BTN_MODE: u16 = 0x13C;
const BTN_THUMBL: u16 = 0x13D;
const BTN_THUMBR: u16 = 0x13E;
const BTN_DPAD_UP: u16 = 0x220;
const BTN_DPAD_DOWN: u16 = 0x221;
const BTN_DPAD_LEFT: u16 = 0x222;
const BTN_DPAD_RIGHT: u16 = 0x223;

const STICK_THRESHOLD: f64 = 0.5;

fn debug_enabled() -> bool {
    std::env::var("CONTROLLER_DEBUG").map(|v| !v.is_empty()).unwrap_or(false)
}

fn button_map() -> Vec<(u16, &'static str)> {
    vec![
        (BTN_A, "a"),
        (BTN_B, "b"),
        (BTN_X, "x"),
        (BTN_Y, "y"),
        (BTN_TL, "l1"),
        (BTN_TR, "r1"),
        (BTN_TL2, "l2"),
        (BTN_TR2, "r2"),
        (BTN_SELECT, "select"),
        (BTN_START, "start"),
        (BTN_MODE, "guide"),
        (BTN_THUMBL, "l3"),
        (BTN_THUMBR, "r3"),
        (BTN_DPAD_UP, "dpad_up"),
        (BTN_DPAD_DOWN, "dpad_down"),
        (BTN_DPAD_LEFT, "dpad_left"),
        (BTN_DPAD_RIGHT, "dpad_right"),
    ]
}

fn js_button_map() -> Vec<(u8, &'static str)> {
    vec![
        (0, "a"),
        (1, "b"),
        (2, "x"),
        (3, "y"),
        (4, "l1"),
        (5, "r1"),
        (6, "select"),
        (7, "start"),
        (8, "guide"),
        (9, "l3"),
        (10, "r3"),
    ]
}

#[repr(C)]
struct InputEvent {
    tv_sec: i64,
    tv_usec: i64,
    ev_type: u16,
    code: u16,
    value: i32,
}

#[repr(C)]
struct JsEvent {
    time: u32,
    value: i16,
    ev_type: u8,
    number: u8,
}

fn list_event_devices() -> Vec<(String, String)> {
    let mut devices = Vec::new();
    let Ok(entries) = std::fs::read_dir("/dev/input") else {
        return devices;
    };
    let mut entries: Vec<_> = entries.filter_map(|e| e.ok()).collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let name = entry.file_name();
        let name_str = name.to_string_lossy().to_string();
        if !name_str.starts_with("event") {
            continue;
        }
        let path = format!("/dev/input/{}", name_str);
        let dev_name = get_device_name(&path).unwrap_or_default();
        devices.push((path, dev_name));
    }
    devices
}

fn get_device_name(path: &str) -> Option<String> {
    use std::fs::File;
    use std::os::fd::FromRawFd;

    let fd = unsafe { libc::open(format!("{}\0", path).as_ptr() as *const i8, libc::O_RDONLY) };
    if fd < 0 {
        return None;
    }
    let file = unsafe { File::from_raw_fd(fd) };

    let mut buf = [0u8; 256];
    let ret = unsafe { libc::ioctl(fd, 0x81004506, buf.as_mut_ptr()) };
    if ret < 0 {
        return None;
    }
    let name = String::from_utf8_lossy(&buf)
        .trim_end_matches('\0')
        .to_string();
    drop(file);
    Some(name)
}

fn get_js_device_name(path: &str) -> String {
    use std::fs::File;
    use std::os::fd::FromRawFd;

    let fd = unsafe { libc::open(format!("{}\0", path).as_ptr() as *const i8, libc::O_RDONLY) };
    if fd < 0 {
        return path.to_string();
    }
    let file = unsafe { File::from_raw_fd(fd) };
    let mut buf = [0u8; 256];
    let ret = unsafe { libc::ioctl(fd, 0x80136A13, buf.as_mut_ptr()) };
    if ret < 0 {
        return path.to_string();
    }
    let name = String::from_utf8_lossy(&buf)
        .trim_end_matches('\0')
        .to_string();
    drop(file);
    if name.is_empty() {
        path.to_string()
    } else {
        name
    }
}

fn looks_like_gamepad(name: &str) -> bool {
    let lower = name.to_lowercase();
    let strong = [
        "gamepad",
        "joystick",
        "xbox",
        "playstation",
        "dualshock",
        "dualsense",
        "8bitdo",
        "usb gamepad",
        "usb joystick",
        "microsoft",
        "360",
        // Valve devices: "Steam Deck" (hid-steam, no Steam Input running) and
        // Steam Controllers show these names in evdev.
        "steam deck",
        "steam controller",
        "neptune",
        "jupiter",
        // Other common pads with non-obvious evdev names.
        "wireless controller",
        "pro controller",
        "nintendo",
        "switch",
        "mayflash",
        "afterglow",
        "n64",
        "genesis",
    ];
    if !strong.iter().any(|k| lower.contains(k)) {
        return false;
    }
    let exclusions = [
        "consumer control",
        "keyboard",
        "mouse",
        "radio",
        "lid switch",
        "touchpad",
        "headphone",
        "hdmi",
        "speaker",
        "headset",
        "audio",
        "microphone",
        "radio control",
    ];
    !exclusions.iter().any(|e| lower.contains(e))
}

// EVIOCGBIT(0, 8): which event types the device supports.
const EVIOCGBIT0: u64 = 0x80084520;
// EVIOCGBIT(EV_KEY, 96): full button bitmap (KEY_MAX 0x2ff → 96 bytes).
const EVIOCGBIT_KEY: u64 = 0x80604521;

fn ev_bit(buf: &[u8], bit: usize) -> bool {
    buf.get(bit / 8).map(|b| (b >> (bit % 8)) & 1 == 1).unwrap_or(false)
}

/// Capability-based gamepad detection: an evdev node is a gamepad when its
/// key bitmap contains any button from the BTN_JOYSTICK/BTN_GAMEPAD ranges
/// (0x130-0x13E, 0x220-0x223). Keyboards, mice and touchpads never expose
/// those codes, so this catches every pad regardless of its device name —
/// including the Deck's built-in "Steam Deck" node and Steam Input's virtual
/// "Microsoft X-Box 360 pad".
fn is_gamepad_capability(path: &str) -> bool {
    use std::fs::File;
    use std::os::fd::FromRawFd;

    let fd = unsafe { libc::open(format!("{}\0", path).as_ptr() as *const i8, libc::O_RDONLY) };
    if fd < 0 {
        return false;
    }
    let _file = unsafe { File::from_raw_fd(fd) };

    let mut types = [0u8; 8];
    let ret = unsafe {
        libc::ioctl(fd, EVIOCGBIT0, types.as_mut_ptr() as *mut libc::c_void)
    };
    if ret < 0 || !ev_bit(&types, EV_KEY as usize) {
        return false;
    }

    let mut keys = [0u8; 96];
    let ret = unsafe {
        libc::ioctl(fd, EVIOCGBIT_KEY as libc::c_ulong, keys.as_mut_ptr() as *mut libc::c_void)
    };
    if ret < 0 {
        return false;
    }
    const GAMEPAD_BUTTONS: &[usize] = &[
        0x130, 0x131, 0x132, 0x133, 0x134, 0x135, // A C X Y Z
        0x136, 0x137, 0x138, 0x139, // TL TR TL2 TR2
        0x13a, 0x13b, 0x13c, 0x13d, 0x13e, // select start mode thumbs
        0x220, 0x221, 0x222, 0x223, // dpad
    ];
    GAMEPAD_BUTTONS.iter().any(|b| ev_bit(&keys, *b))
}

fn check_deck_mode() -> bool {
    if std::env::var("GAMESCOPE_WAYLAND_DISPLAY").is_ok() {
        return true;
    }
    if std::env::var("XDG_CURRENT_DESKTOP")
        .map(|v| v.to_lowercase().contains("deck"))
        .unwrap_or(false)
    {
        return true;
    }
    false
}

mod imp {
    use super::*;
    use std::sync::atomic::AtomicBool;

    pub struct ControllerManager {
        pub deck_mode: AtomicBool,
        /// Paths with a live reader thread; the source of truth for
        /// `connected()` (no increment/decrement races between the scanner
        /// and reader threads).
        pub active_paths: Mutex<HashSet<String>>,
    }

    impl Default for ControllerManager {
        fn default() -> Self {
            Self {
                deck_mode: AtomicBool::new(false),
                active_paths: Mutex::new(HashSet::new()),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ControllerManager {
        const NAME: &'static str = "TrainingControllerManager";
        type Type = super::ControllerManager;
    }

    impl ObjectImpl for ControllerManager {
        fn signals() -> &'static [glib::subclass::Signal] {
            use std::sync::OnceLock;
            static SIGNALS: OnceLock<Vec<glib::subclass::Signal>> = OnceLock::new();
            SIGNALS.get_or_init(|| {
                vec![
                    glib::subclass::Signal::builder("action-activated")
                        .param_types([String::static_type()])
                        .build(),
                    glib::subclass::Signal::builder("device-connected").build(),
                    glib::subclass::Signal::builder("device-disconnected").build(),
                ]
            })
        }
    }
}

glib::wrapper! {
    pub struct ControllerManager(ObjectSubclass<imp::ControllerManager>);
}

impl ControllerManager {
    pub fn new() -> Self {
        let obj: Self = glib::Object::new();

        println!("[ControllerManager] Scanning for controllers...");
        obj.scan_evdev();
        obj.scan_js();

        let count = obj.imp().active_paths.lock().unwrap().len();
        if count == 0 {
            println!("[ControllerManager] WARNING: No controllers detected.");
            println!("                Bluetooth Xbox pads often need the 'xpadneo' driver.");
            println!("                Try connecting via USB cable for an instant test.");
        } else {
            println!("[ControllerManager] Total attached: {}", count);
        }

        obj.imp().deck_mode.store(check_deck_mode(), Ordering::Relaxed);

        // Hot-plug: rescan every 3 s so controllers that appear later
        // (Steam Input's virtual pad in Game Mode, Bluetooth/USB pairing)
        // are attached too. Loops remove their path on disconnect.
        let obj_weak = obj.downgrade();
        glib::timeout_add_local(Duration::from_secs(3), move || {
            if let Some(m) = obj_weak.upgrade() {
                m.scan_evdev();
                m.scan_js();
            }
            glib::ControlFlow::Continue
        });

        obj
    }

    pub fn deck_mode(&self) -> bool {
        self.imp().deck_mode.load(Ordering::Relaxed)
    }

    pub fn connected(&self) -> bool {
        !self.imp().active_paths.lock().unwrap().is_empty()
    }


    fn emit_connected(&self) {
        self.emit_by_name::<()>("device-connected", &[]);
    }

    fn emit_disconnected(&self) {
        self.emit_by_name::<()>("device-disconnected", &[]);
    }

    /// Called by reader threads when their device disappears.
    fn detach(&self, path: &str) {
        {
            let mut active = self.imp().active_paths.lock().unwrap();
            if !active.remove(path) {
                return;
            }
        }
        println!("[ControllerManager] Detached {}", path);
        self.emit_disconnected();
    }

    fn try_claim(&self, path: &str) -> bool {
        let mut active = self.imp().active_paths.lock().unwrap();
        if active.contains(path) {
            return false;
        }
        active.insert(path.to_string());
        true
    }

    fn scan_evdev(&self) {
        let btn_map = button_map();
        let devices = list_event_devices();

        for (path, name) in devices {
            if !looks_like_gamepad(&name) && !is_gamepad_capability(&path) {
                continue;
            }
            if !self.try_claim(&path) {
                continue;
            }

            let obj_weak = self.downgrade();
            let name_clone = name.clone();
            let path_clone = path.clone();
            let btn_map_clone = btn_map.clone();

            thread::spawn(move || {
                evdev_read_loop(path_clone, name_clone, btn_map_clone, obj_weak);
            });

            println!("[ControllerManager] Attached evdev: {} @ {}", name, path);
            self.emit_connected();
        }
    }

    fn scan_js(&self) {
        let js_map = js_button_map();

        let Ok(entries) = std::fs::read_dir("/dev/input") else {
            return;
        };
        let mut entries: Vec<_> = entries.filter_map(|e| e.ok()).collect();
        entries.sort_by_key(|e| e.file_name());

        let evdev_names: Vec<String> = list_event_devices()
            .into_iter()
            .filter(|(_, n)| looks_like_gamepad(n))
            .map(|(_, n)| n.to_lowercase())
            .collect();

        for entry in entries {
            let name = entry.file_name();
            let name_str = name.to_string_lossy().to_string();
            if !name_str.starts_with("js") {
                continue;
            }
            let path = format!("/dev/input/{}", name_str);
            let dev_name = get_js_device_name(&path);

            if !looks_like_gamepad(&dev_name) && !is_gamepad_capability(&path) {
                continue;
            }

            let dev_lower = dev_name.to_lowercase();
            if evdev_names
                .iter()
                .any(|en| dev_lower.contains(en.as_str()) || en.contains(&dev_lower))
            {
                println!(
                    "[ControllerManager] Skipping {} (already reading evdev for '{}')",
                    path, dev_name
                );
                continue;
            }

            if !self.try_claim(&path) {
                continue;
            }

            let obj_weak = self.downgrade();
            let name_clone = dev_name.clone();
            let path_clone = path.clone();
            let js_map_clone = js_map.clone();

            thread::spawn(move || {
                js_read_loop(path_clone, name_clone, js_map_clone, obj_weak);
            });

            println!("[ControllerManager] Attached js: {} @ {}", dev_name, path);
            self.emit_connected();
        }
    }
}

fn evdev_read_loop(
    path: String,
    name: String,
    btn_map: Vec<(u16, &'static str)>,
    manager: glib::WeakRef<ControllerManager>,
) {
    let fd = unsafe {
        libc::open(
            format!("{}\0", path).as_ptr() as *const i8,
            libc::O_RDONLY | libc::O_NONBLOCK,
        )
    };
    if fd < 0 {
        eprintln!("[ControllerManager] Could not open {}: {}", path, std::io::Error::last_os_error());
        if let Some(mgr) = manager.upgrade() {
            mgr.detach(&path);
        }
        return;
    }

    println!("[ControllerManager] Monitoring {} @ {}", name, path);

    let event_size = std::mem::size_of::<InputEvent>();
    let mut axis_state: std::collections::HashMap<u16, f64> = std::collections::HashMap::new();

    loop {
        let mut fds = [libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        }];
        let ret = unsafe { libc::poll(fds.as_mut_ptr(), 1, 1000) };
        if ret < 0 {
            // EINTR and friends: retry
            continue;
        }
        if ret == 0 {
            continue;
        }
        if fds[0].revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0 {
            break;
        }
        if fds[0].revents & libc::POLLIN == 0 {
            continue;
        }

        let mut buf = [0u8; 64];
        let n = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, event_size) };
        if n == 0 {
            break;
        }
        if n < event_size as isize {
            continue;
        }

        let event: InputEvent = unsafe { std::ptr::read(buf.as_ptr() as *const InputEvent) };

        if debug_enabled() {
            println!(
                "[ControllerManager] evdev {} type={:#x} code={:#x} value={}",
                path, event.ev_type, event.code, event.value
            );
        }

        let action = handle_evdev_event(event, &btn_map, &mut axis_state);
        if let Some(action) = action {
            if debug_enabled() {
                println!("[ControllerManager] -> action '{}'", action);
            }
            let action = action.to_string();
            let manager = manager.clone();
            glib::idle_add_once(move || {
                if let Some(mgr) = manager.upgrade() {
                    if debug_enabled() {
                        println!("[ControllerManager] emitting action-activated '{}'", action);
                    }
                    mgr.emit_by_name::<()>("action-activated", &[&action]);
                }
            });
        }
    }

    unsafe { libc::close(fd) };
    if let Some(mgr) = manager.upgrade() {
        mgr.detach(&path);
    }
}

fn handle_evdev_event(
    event: InputEvent,
    btn_map: &[(u16, &'static str)],
    axis_state: &mut std::collections::HashMap<u16, f64>,
) -> Option<&'static str> {
    match event.ev_type {
        EV_KEY => {
            if event.value == 1 {
                btn_map
                    .iter()
                    .find(|(code, _)| *code == event.code)
                    .map(|(_, action)| *action)
            } else {
                None
            }
        }
        EV_ABS => {
            let prev = *axis_state.get(&event.code).unwrap_or(&0.0);
            axis_state.insert(event.code, event.value as f64);

            match event.code {
                ABS_HAT0X => {
                    if event.value < 0 {
                        Some("dpad_left")
                    } else if event.value > 0 {
                        Some("dpad_right")
                    } else {
                        None
                    }
                }
                ABS_HAT0Y => {
                    if event.value < 0 {
                        Some("dpad_up")
                    } else if event.value > 0 {
                        Some("dpad_down")
                    } else {
                        None
                    }
                }
                ABS_X | ABS_Y => {
                    let val = if event.value.abs() > 1 {
                        event.value as f64 / 32767.0
                    } else {
                        event.value as f64
                    };
                    let prev_norm = prev / 32767.0;

                    if event.code == ABS_X {
                        if val > STICK_THRESHOLD && prev_norm <= STICK_THRESHOLD {
                            Some("dpad_right")
                        } else if val < -STICK_THRESHOLD && prev_norm >= -STICK_THRESHOLD {
                            Some("dpad_left")
                        } else {
                            None
                        }
                    } else if event.code == ABS_Y {
                        if val > STICK_THRESHOLD && prev_norm <= STICK_THRESHOLD {
                            Some("dpad_down")
                        } else if val < -STICK_THRESHOLD && prev_norm >= -STICK_THRESHOLD {
                            Some("dpad_up")
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                _ => None,
            }
        }
        _ => None,
    }
}

fn js_read_loop(
    path: String,
    name: String,
    js_map: Vec<(u8, &'static str)>,
    manager: glib::WeakRef<ControllerManager>,
) {
    let fd = unsafe {
        libc::open(
            format!("{}\0", path).as_ptr() as *const i8,
            libc::O_RDONLY | libc::O_NONBLOCK,
        )
    };
    if fd < 0 {
        eprintln!("[ControllerManager] Could not open {}: {}", path, std::io::Error::last_os_error());
        if let Some(mgr) = manager.upgrade() {
            mgr.detach(&path);
        }
        return;
    }

    println!("[ControllerManager] Monitoring joystick {} @ {}", name, path);

    let event_size = std::mem::size_of::<JsEvent>();
    let mut axis_state: std::collections::HashMap<u8, i16> = std::collections::HashMap::new();

    loop {
        let mut fds = [libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        }];
        let ret = unsafe { libc::poll(fds.as_mut_ptr(), 1, 500) };
        if ret < 0 || ret == 0 {
            continue;
        }
        if fds[0].revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0 {
            break;
        }
        if fds[0].revents & libc::POLLIN == 0 {
            continue;
        }

        let mut buf = [0u8; 8];
        let n = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, event_size) };
        if n == 0 {
            break;
        }
        if n < event_size as isize {
            continue;
        }

        let event: JsEvent = unsafe { std::ptr::read(buf.as_ptr() as *const JsEvent) };
        let actual_type = event.ev_type & !0x80;

        let action: Option<&'static str> = match actual_type {
            0x01 => {
                if event.value == 1 {
                    js_map
                        .iter()
                        .find(|(num, _)| *num == event.number)
                        .map(|(_, action)| *action)
                } else {
                    None
                }
            }
            0x02 => {
                let prev = *axis_state.get(&event.number).unwrap_or(&0);
                axis_state.insert(event.number, event.value);
                let v = event.value;
                match event.number {
                    6 => {
                        if v > 16384 && prev <= 16384 {
                            Some("dpad_right")
                        } else if v < -16384 && prev >= -16384 {
                            Some("dpad_left")
                        } else {
                            None
                        }
                    }
                    7 => {
                        if v > 16384 && prev <= 16384 {
                            Some("dpad_down")
                        } else if v < -16384 && prev >= -16384 {
                            Some("dpad_up")
                        } else {
                            None
                        }
                    }
                    0 => {
                        if v > 16384 && prev <= 16384 {
                            Some("dpad_right")
                        } else if v < -16384 && prev >= -16384 {
                            Some("dpad_left")
                        } else {
                            None
                        }
                    }
                    1 => {
                        if v > 16384 && prev <= 16384 {
                            Some("dpad_down")
                        } else if v < -16384 && prev >= -16384 {
                            Some("dpad_up")
                        } else {
                            None
                        }
                    }
                    _ => None,
                }
            }
            _ => None,
        };

        if let Some(action) = action {
            let action = action.to_string();
            let manager = manager.clone();
            glib::idle_add_once(move || {
                if let Some(mgr) = manager.upgrade() {
                    mgr.emit_by_name::<()>("action-activated", &[&action]);
                }
            });
        }
    }

    unsafe { libc::close(fd) };
    if let Some(mgr) = manager.upgrade() {
        mgr.detach(&path);
    }
}

#[allow(dead_code)]
pub fn sleep_ms(ms: u64) {
    thread::sleep(Duration::from_millis(ms));
}
