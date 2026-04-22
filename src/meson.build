pkgdatadir = join_paths(get_option('datadir'), 'training')

python_modules = [
  '__init__.py',
  'ai_coach.py',
  'csv_io.py',
  'data_store.py',
  'exercise_picker.py',
  'history.py',
  'home.py',
  'image_utils.py',
  'llm_client.py',
  'models.py',
  'preferences.py',
  'round_timer.py',
  'settings.py',
  'sound.py',
  'timer_core.py',
  'training_plan.py',
  'ui_scaling.py',
  'user_exercises.py',
  'window.py',
]

install_data(
  python_modules,
  install_dir: pkgdatadir,
)

install_data(
  'main.py',
  install_dir: get_option('bindir'),
  install_mode: 'rwxr-xr-x',
  rename: 'training',
)

install_data(
  'resources' / 'sounds' / 'beep.ogg',
  'resources' / 'sounds' / 'exercise_complete.ogg',
  'resources' / 'sounds' / 'round_end.ogg',
  'resources' / 'sounds' / 'round_start.ogg',
  'resources' / 'sounds' / 'training_complete.ogg',
  install_dir: pkgdatadir / 'resources' / 'sounds',
)

install_data(
  'resources' / 'exercises' / 'exercises.json',
  install_dir: pkgdatadir / 'resources' / 'exercises',
)

install_data(
  'resources' / 'plans' / 'default_plans.json',
  install_dir: pkgdatadir / 'resources' / 'plans',
)

install_subdir(
  'resources' / 'exercises' / 'images',
  install_dir: pkgdatadir / 'resources' / 'exercises',
)