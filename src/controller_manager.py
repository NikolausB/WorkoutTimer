import gi
import os
import struct
import fcntl
import select
import threading
from gi.repository import GObject, GLib

# evdev constants ----------------------------------------------------------
EVIOCGNAME = 0x81004506
EVIOCGRAB = 0x40044590
JSIOCGNAME = 0x80136A13  # JSIOCGNAME(len) – deprecated but works on most systems

# event types
EV_SYN = 0x00
EV_KEY = 0x01
EV_ABS = 0x03

# ABS axis codes
ABS_X = 0x00
ABS_Y = 0x01
ABS_Z = 0x02
ABS_RX = 0x03
ABS_RY = 0x04
ABS_RZ = 0x05
ABS_HAT0X = 0x10
ABS_HAT0Y = 0x11

# common gamepad button evdev keycodes
_BTN_A = 0x130
_BTN_B = 0x131
_BTN_C = 0x132
_BTN_X = 0x133
_BTN_Y = 0x134
_BTN_Z = 0x135
_BTN_TL = 0x136
_BTN_TR = 0x137
_BTN_TL2 = 0x138
_BTN_TR2 = 0x139
_BTN_SELECT = 0x13A
_BTN_START = 0x13B
_BTN_MODE = 0x13C
_BTN_THUMBL = 0x13D
_BTN_THUMBR = 0x13E
_BTN_DPAD_UP = 0x220
_BTN_DPAD_DOWN = 0x221
_BTN_DPAD_LEFT = 0x222
_BTN_DPAD_RIGHT = 0x223

# Joystick button map (for /dev/input/js0 fallback)
_JS_BTN_A = 0
_JS_BTN_B = 1
_JS_BTN_X = 2
_JS_BTN_Y = 3
_JS_BTN_TL = 4
_JS_BTN_TR = 5
_JS_BTN_SELECT = 6
_JS_BTN_START = 7
_JS_BTN_MODE = 8
_JS_BTN_THUMBL = 9
_JS_BTN_THUMBR = 10

_BUTTON_MAP = {
    _BTN_A: "a",
    _BTN_B: "b",
    _BTN_X: "x",
    _BTN_Y: "y",
    _BTN_TL: "l1",
    _BTN_TR: "r1",
    _BTN_TL2: "l2",
    _BTN_TR2: "r2",
    _BTN_SELECT: "select",
    _BTN_START: "start",
    _BTN_MODE: "guide",
    _BTN_THUMBL: "l3",
    _BTN_THUMBR: "r3",
    _BTN_DPAD_UP: "dpad_up",
    _BTN_DPAD_DOWN: "dpad_down",
    _BTN_DPAD_LEFT: "dpad_left",
    _BTN_DPAD_RIGHT: "dpad_right",
}

_JS_BUTTON_MAP = {
    _JS_BTN_A: "a",
    _JS_BTN_B: "b",
    _JS_BTN_X: "x",
    _JS_BTN_Y: "y",
    _JS_BTN_TL: "l1",
    _JS_BTN_TR: "r1",
    _JS_BTN_SELECT: "select",
    _JS_BTN_START: "start",
    _JS_BTN_MODE: "guide",
    _JS_BTN_THUMBL: "l3",
    _JS_BTN_THUMBR: "r3",
}

STICK_THRESHOLD = 0.5


def _list_event_devices():
    try:
        entries = os.listdir("/dev/input")
    except OSError:
        return
    for entry in sorted(entries):
        if not entry.startswith("event"):
            continue
        path = os.path.join("/dev/input", entry)
        try:
            with open(path, "rb") as fh:
                buf = bytearray(256)
                fcntl.ioctl(fh, EVIOCGNAME, buf, True)
                name = buf.split(b"\x00")[0].decode("utf-8", errors="replace")
        except Exception:
            name = ""
        yield path, name


_js_struct_cached = None


def _js_event_struct():
    global _js_struct_cached
    if _js_struct_cached is None:
        for fmt in ("IhBB", "IhHb"):
            try:
                struct.calcsize(fmt)
                _js_struct_cached = fmt
                break
            except struct.error:
                continue
        else:
            _js_struct_cached = "IhBB"
    return _js_struct_cached


class _EvdevDevice:
    def __init__(self, path, name, manager):
        self.path = path
        self.name = name
        self._manager = manager
        self._fd = None
        self._thread = None
        self._alive = False
        self._axis_state = {}

    def start(self):
        if self._alive:
            return
        try:
            self._fd = os.open(self.path, os.O_RDONLY | os.O_NONBLOCK)
            # We intentionally do *not* EVIOCGRAB here.  Grabbing silences
            # the real mouse / keyboard devices when the controller has
            # virtual mouse/keyboard slave nodes (common on Xbox pads on
            # Linux).  Reading in non-blocking mode is sufficient.
        except OSError as e:
            print(f"[ControllerManager] Could not open {self.path}: {e}")
            return
        self._alive = True
        self._thread = threading.Thread(target=self._read_loop, daemon=True)
        self._thread.start()
        print(f"[ControllerManager] Monitoring {self.name} @ {self.path}")

    def stop(self):
        self._alive = False
        if self._fd is not None:
            try:
                os.close(self._fd)
            except OSError:
                pass
            self._fd = None

    def _read_loop(self):
        EVENT_SIZE = struct.calcsize("llHHi")
        while self._alive and self._fd is not None:
            try:
                ready, _, _ = select.select([self._fd], [], [], 1.0)
                if not ready:
                    continue
                data = os.read(self._fd, EVENT_SIZE)
            except OSError:
                break
            if not data or len(data) < EVENT_SIZE:
                continue
            _, _, ev_type, ev_code, ev_value = struct.unpack("llHHi", data)
            self._handle_event(ev_type, ev_code, ev_value)

    def _handle_event(self, ev_type, ev_code, ev_value):
        if ev_type == EV_KEY and ev_value == 1:
            action = _BUTTON_MAP.get(ev_code)
            if action:
                GLib.idle_add(self._manager._emit_action, action)
        elif ev_type == EV_ABS:
            prev = self._axis_state.get(ev_code, 0.0)
            self._axis_state[ev_code] = ev_value
            action = None
            if ev_code == ABS_HAT0X:
                action = "dpad_left" if ev_value < 0 else "dpad_right" if ev_value > 0 else None
            elif ev_code == ABS_HAT0Y:
                action = "dpad_up" if ev_value < 0 else "dpad_down" if ev_value > 0 else None
            elif ev_code in (ABS_X, ABS_Y):
                val = ev_value / 32767.0 if abs(ev_value) > 1 else float(ev_value)
                if abs(ev_value) <= 1:
                    val = float(ev_value)
                if ev_code == ABS_X:
                    if val > STICK_THRESHOLD and prev / 32767.0 <= STICK_THRESHOLD:
                        action = "dpad_right"
                    elif val < -STICK_THRESHOLD and prev / 32767.0 >= -STICK_THRESHOLD:
                        action = "dpad_left"
                elif ev_code == ABS_Y:
                    if val > STICK_THRESHOLD and prev / 32767.0 <= STICK_THRESHOLD:
                        action = "dpad_down"
                    elif val < -STICK_THRESHOLD and prev / 32767.0 >= -STICK_THRESHOLD:
                        action = "dpad_up"
            if action:
                GLib.idle_add(self._manager._emit_action, action)


class _JoystickFallbackDevice:
    def __init__(self, path, name, manager):
        self.path = path
        self.name = name
        self._manager = manager
        self._fd = None
        self._thread = None
        self._alive = False
        self._axis_state = {}

    def start(self):
        if self._alive:
            return
        try:
            self._fd = os.open(self.path, os.O_RDONLY | os.O_NONBLOCK)
        except OSError as e:
            print(f"[ControllerManager] Could not open {self.path}: {e}")
            return
        self._alive = True
        self._thread = threading.Thread(target=self._read_loop, daemon=True)
        self._thread.start()
        print(f"[ControllerManager] Monitoring joystick {self.name} @ {self.path}")

    def stop(self):
        self._alive = False
        if self._fd is not None:
            try:
                os.close(self._fd)
            except OSError:
                pass
            self._fd = None

    def _read_loop(self):
        fmt = "IhBB"
        size = struct.calcsize(fmt)
        self._last_debug = 0
        while self._alive and self._fd is not None:
            try:
                ready, _, _ = select.select([self._fd], [], [], 0.5)
                if not ready:
                    continue
                data = os.read(self._fd, size)
            except OSError:
                break
            if len(data) < size:
                continue
            time_val, value, typ, number = struct.unpack(fmt, data)
            import time as _time
            now = _time.time()
            if now - self._last_debug > 2:
                print(f"[ControllerManager] js0 raw: typ={typ} number={number} value={value}")
                self._last_debug = now
            actual_type = typ & ~0x80
            if actual_type == 0x01:  # JS_EVENT_BUTTON
                if value == 1:
                    action = _JS_BUTTON_MAP.get(number)
                    if action:
                        print(f"[ControllerManager] BUTTON {number} -> {action}")
                        GLib.idle_add(self._manager._emit_action, action)
            elif actual_type == 0x02:  # JS_EVENT_AXIS
                action = None
                # Xbox controller js mapping:
                #   0 = left stick X, 1 = left stick Y
                #   2 = right stick X, 3 = right stick Y
                #   4 = left trigger, 5 = right trigger
                #   6 = dpad X, 7 = dpad Y
                prev = self._axis_state.get(number, 0)
                self._axis_state[number] = value
                if number == 6:
                    if value > 16384 and prev <= 16384:
                        action = "dpad_right"
                    elif value < -16384 and prev >= -16384:
                        action = "dpad_left"
                elif number == 7:
                    if value > 16384 and prev <= 16384:
                        action = "dpad_down"
                    elif value < -16384 and prev >= -16384:
                        action = "dpad_up"
                elif number == 0:
                    if value > 16384 and prev <= 16384:
                        action = "dpad_right"
                    elif value < -16384 and prev >= -16384:
                        action = "dpad_left"
                elif number == 1:
                    if value > 16384 and prev <= 16384:
                        action = "dpad_down"
                    elif value < -16384 and prev >= -16384:
                        action = "dpad_up"
                if action:
                    print(f"[ControllerManager] AXIS {number} -> {action}")
                    GLib.idle_add(self._manager._emit_action, action)


class ControllerManager(GObject.GObject):
    __gsignals__ = {
        "action_activated":    (GObject.SignalFlags.RUN_FIRST, None, (str,)),
        "device_connected":    (GObject.SignalFlags.RUN_FIRST, None, ()),
        "device_disconnected": (GObject.SignalFlags.RUN_FIRST, None, ()),
    }

    def __init__(self):
        super().__init__()
        self._device_count = 0
        self._deck_mode = False
        self._devices = []

        print("[ControllerManager] Scanning for controllers...")
        self._scan_evdev()
        self._scan_js()

        if self._device_count == 0:
            print("[ControllerManager] WARNING: No controllers detected.")
            print("                Bluetooth Xbox pads often need the 'xpadneo' driver.")
            print("                Try connecting via USB cable for an instant test.")
        else:
            print(f"[ControllerManager] Total attached: {self._device_count}")

        self._check_deck_mode()

    @property
    def deck_mode(self):
        return self._deck_mode

    @property
    def connected(self):
        return self._device_count > 0

    def _emit_action(self, action: str):
        self.emit("action_activated", action)
        return GLib.SOURCE_REMOVE

    def _check_deck_mode(self):
        env = os.environ
        if env.get("GAMESCOPE_WAYLAND_DISPLAY"):
            self._deck_mode = True
            return
        if "deck" in env.get("XDG_CURRENT_DESKTOP", "").lower():
            self._deck_mode = True
            return
        self._deck_mode = False

    def _looks_like_gamepad(self, name: str) -> bool:
        lower = name.lower()
        strong = [
            "gamepad", "joystick", "xbox", "playstation",
            "dualshock", "dualsense",
            "8bitdo", "usb gamepad", "usb joystick",
            "microsoft", "360",
        ]
        if not any(k in lower for k in strong):
            return False
        exclusions = [
            "consumer control", "keyboard", "mouse",
            "radio", "lid switch", "touchpad",
            "headphone", "hdmi", "speaker", "headset",
            "audio", "microphone", "radio control",
        ]
        return not any(exc in lower for exc in exclusions)

    def _scan_evdev(self):
        for path, name in _list_event_devices():
            if not self._looks_like_gamepad(name):
                continue
            dev = _EvdevDevice(path, name, self)
            dev.start()
            self._devices.append(dev)
            self._device_count += 1
            print(f"[ControllerManager] Attached evdev: {name} @ {path}")
            self.emit("device_connected")

    def _scan_js(self):
        # If we already found the same controller via evdev, skip the js node
        # to avoid duplicate actions for every button press.
        evdev_names = {dev.name.lower() for dev in self._devices}
        try:
            entries = os.listdir("/dev/input")
        except OSError:
            return
        for entry in sorted(entries):
            if not entry.startswith("js"):
                continue
            path = os.path.join("/dev/input", entry)
            try:
                fd = os.open(path, os.O_RDONLY | os.O_NONBLOCK)
                buf = bytearray(256)
                fcntl.ioctl(fd, JSIOCGNAME, buf, True)
                name = buf.split(b"\x00")[0].decode("utf-8", errors="replace")
                os.close(fd)
            except Exception:
                name = entry
            if not self._looks_like_gamepad(name):
                continue
            # Deduplicate: skip js if a matching evdev node already exists
            if any(name.lower() in ev_name or ev_name in name.lower() for ev_name in evdev_names):
                print(f"[ControllerManager] Skipping {path} (already reading evdev for '{name}')")
                continue
            dev = _JoystickFallbackDevice(path, name, self)
            dev.start()
            self._devices.append(dev)
            self._device_count += 1
            print(f"[ControllerManager] Attached js: {name} @ {path}")
            self.emit("device_connected")
