application_id = 'io.github.NikolausB.WorkoutTimer'

install_data(
  '@0@.desktop'.format(application_id),
  install_dir: join_paths(get_option('datadir'), 'applications'),
)

install_data(
  '@0@.metainfo.xml'.format(application_id),
  install_dir: join_paths(get_option('datadir'), 'metainfo'),
)

icondir = join_paths('icons', 'hicolor', 'scalable', 'apps')
install_data(
  join_paths(icondir, '@0@.svg'.format(application_id)),
  install_dir: join_paths(get_option('datadir'), icondir),
)

icondir = join_paths('icons', 'hicolor', 'symbolic', 'apps')
install_data(
  join_paths(icondir, '@0@-symbolic.svg'.format(application_id)),
  install_dir: join_paths(get_option('datadir'), icondir),
)