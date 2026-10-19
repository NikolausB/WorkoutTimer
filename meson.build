project('workout-timer', ['c', 'rust'],
  version: '1.3.0',
  meson_version: '>= 0.63.0',
)

prefix = get_option('prefix')
bindir = join_paths(prefix, get_option('bindir'))
datadir = join_paths(prefix, get_option('datadir'))
pkgdatadir = join_paths(datadir, 'training')

subdir('src')
subdir('data')

install_data('LICENSE',
  install_dir: join_paths(get_option('prefix'), 'share', 'licenses', 'io.github.NikolausB.WorkoutTimer')
)