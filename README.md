# Workout Timer

A sport training application for Linux, built with GTK 4 and libadwaita.

## Features

- **Round Timer** — Configurable rounds, duration, and pause periods with audio alerts
- **Training Plan Builder** — Create plans with timed and rep-based exercises
- **Built-in Exercise Database** — 873 exercises with images from [free-exercise-db](https://github.com/yuhonas/free-exercise-db) (Unlicense)
- **11 Default Training Plans** — Including pop culture-inspired plans (Saitama, Demon Slayer Corps, Rocky)
- **Training History** — Per-session detail with per-exercise logging (actual vs planned), CSV export/import
- **Customizable Sounds** — Choose different sounds for round start, round end, exercise complete, and training complete
- **Fullscreen Mode** — F11 or header bar button for distraction-free workouts
- **Add Custom Exercises** — Add your own exercises with custom images

## Screenshots

### Round Timer
![Round Timer](screenshots/RoundTimer.png)

### Training Plans
![Training Plans List](screenshots/TrainingPlans.png)

### Training Plan Editor
![Training Plan Editor](screenshots/TrainingPlans2.png)

### Training Plan Runner
![Training Plan Runner](screenshots/TrainingPlans3.png)

### Training History
![Training History](screenshots/TrainingHistory.png)

### Settings
![Training Settings](screenshots/TrainingSettings.png)

## Installation

### Flatpak (recommended)

Build and install locally:

```bash
# Install runtimes
flatpak install flathub org.gnome.Platform//49 org.gnome.Sdk//49
flatpak install flathub org.flatpak.Builder

# Build and install
flatpak run --command=flatpak-builder org.flatpak.Builder \
  --user --install --force-clean build-dir/ io.github.NikolausB.WorkoutTimer.yml

# Run
flatpak run io.github.NikolausB.WorkoutTimer
```

### Run from source

```bash
bash run.sh
```

Requires: Python 3.9+, GTK 4, libadwaita, GStreamer, PyGObject

## Default Training Plans

| Plan | Exercises | Focus |
|------|-----------|-------|
| Full Body Beginner | 6 | Bodyweight basics |
| Upper Body Strength | 6 | Pushups, curls, rows |
| Lower Body Strength | 6 | Squats, lunges, calf raises |
| HIIT Cardio Blast | 8 | High-intensity intervals |
| Core and Abs | 6 | Crunches, planks, leg raises |
| Kettlebell Full Body | 6 | Swings, cleans, presses |
| Kettlebell Strength and Power | 6 | Snatches, windmills, Turkish get-ups |
| Sling Trainer Full Body | 6 | Suspension trainer exercises |
| Saitama's Workout | 4 | 100 push-ups, 100 sit-ups, 100 squats, 10km run |
| Demon Slayer Corps Training | 7 | High-intensity bodyweight circuit |
| Rocky Balboa's Training | 7 | Jump rope, pushups, dips, lunges, calf raises |

## Credits

See [CREDITS.md](CREDITS.md) for full credits and third-party resources.

## License

MIT