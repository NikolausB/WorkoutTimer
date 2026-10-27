pkgdatadir = join_paths(get_option('datadir'), 'training')

cargo = find_program('cargo', required: true)

cargo_options = ['--manifest-path', meson.project_source_root() / 'Cargo.toml']
cargo_options += ['--target-dir', meson.project_build_root() / 'target']
cargo_options += ['--release']
rust_target = 'release'

# Honor the flatpak manifest's CARGO_HOME / CARGO_NET_OFFLINE: the
# cargo-sources.json vendor layout lives inside that CARGO_HOME. Only
# fall back to a build-dir cargo home for plain meson builds outside
# flatpak (which then resolve crates over the network).
cargo_home = run_command('sh', '-c', 'printf %s "${CARGO_HOME:-}"', check: true).stdout()
if cargo_home == ''
  cargo_home = meson.project_build_root() / 'cargo-home'
endif
cargo_net_offline = run_command('sh', '-c', 'printf %s "${CARGO_NET_OFFLINE:-false}"', check: true).stdout()

custom_target(
  'cargo-build',
  build_by_default: true,
  build_always_stale: true,
  output: 'training',
  console: true,
  install: true,
  install_dir: bindir,
  env: {
    'CARGO_HOME': cargo_home,
    'CARGO_NET_OFFLINE': cargo_net_offline,
  },
  command: [
    cargo, 'build', cargo_options, '&&',
    'cp', meson.project_build_root() / 'target' / rust_target / 'training', '@OUTPUT@',
  ],
)

install_data(
  'app_style.css',
  install_dir: pkgdatadir,
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