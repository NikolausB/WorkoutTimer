use gstreamer::prelude::*;
use gstreamer::{ElementFactory, State};
use std::path::PathBuf;
use std::sync::Mutex;

pub const AVAILABLE_SOUNDS: &[&str] = &[
    "beep",
    "round_start",
    "round_end",
    "exercise_complete",
    "training_complete",
];

fn get_sounds_dir() -> PathBuf {
    let flatpak_dir = PathBuf::from("/app/share/training/resources/sounds");
    if flatpak_dir.exists() {
        return flatpak_dir;
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("resources")
        .join("sounds")
}

pub struct SoundPlayer {
    playbin: Mutex<Option<gstreamer::Element>>,
}

impl SoundPlayer {
    pub fn new() -> Self {
        let _ = gstreamer::init();
        Self {
            playbin: Mutex::new(None),
        }
    }

    pub fn play_sound(&self, name: Option<&str>) {
        let Some(name) = name else { return };
        if !AVAILABLE_SOUNDS.contains(&name) {
            return;
        }
        let path = get_sounds_dir().join(format!("{}.ogg", name));
        if !path.exists() {
            return;
        }

        self.stop();

        let Ok(playbin) = ElementFactory::make("playbin").build() else {
            return;
        };
        let uri = format!("file://{}", path.display());
        playbin.set_property("uri", &uri);
        let volume = crate::settings::load_settings().sound_volume.clamp(0.0, 1.0);
        playbin.set_property("volume", volume);

        {
            let mut guard = self.playbin.lock().unwrap();
            *guard = Some(playbin.clone());
        }

        playbin.set_state(State::Playing).ok();
    }

    #[allow(dead_code)]
    pub fn play_beep(&self) {
        self.play_sound(Some("beep"));
    }

    pub fn stop(&self) {
        let mut guard = self.playbin.lock().unwrap();
        if let Some(ref playbin) = *guard {
            playbin.set_state(State::Null).ok();
        }
        *guard = None;
    }
}

impl Default for SoundPlayer {
    fn default() -> Self {
        Self::new()
    }
}