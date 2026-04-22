project('workout-timer', 'c',
  version: '1.0.2',
  meson_version: '>= 0.59.0',
)

prefix = get_option('prefix')
bindir = join_paths(prefix, get_option('bindir'))
datadir = join_paths(prefix, get_option('datadir'))
pkgdatadir = join_paths(datadir, 'training')

subdir('src')
subdir('data')