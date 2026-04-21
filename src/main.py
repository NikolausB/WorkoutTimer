#!/usr/bin/env python3
import sys
import os
import gi

sys.path.insert(0, "/app/share/training")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

gi.require_version("Adw", "1")
gi.require_version("Gst", "1.0")

from gi.repository import Adw, Gtk, Gio, Gdk
from window import MainWindow


class TrainingApp(Adw.Application):
    def __init__(self):
        super().__init__(
            application_id="io.github.NikolausB.WorkoutTimer",
            flags=Gio.ApplicationFlags.FLAGS_NONE,
        )

    def do_activate(self):
        self.set_accels_for_action("win.toggle-fullscreen", ["F11"])
        css_provider = Gtk.CssProvider()
        css_provider.load_from_data(b"""
            .timer-display {
                font-size: 72px;
                font-weight: 800;
            }
        """)
        Gtk.StyleContext.add_provider_for_display(
            Gdk.Display.get_default(),
            css_provider,
            Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION,
        )
        win = self.props.active_window
        if not win:
            win = MainWindow(application=self)
        win.present()


def main():
    app = TrainingApp()
    return app.run(sys.argv)


if __name__ == "__main__":
    main()