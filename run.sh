#!/bin/bash
# Build and run Workout Timer from source (development mode).
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Pick up cargo if installed via rustup
if [ -f "$HOME/.cargo/env" ]; then
  # shellcheck disable=SC1091
  source "$HOME/.cargo/env"
fi

cd "$SCRIPT_DIR" || exit 1

if command -v cargo >/dev/null 2>&1; then
  cargo build --manifest-path "$SCRIPT_DIR/Cargo.toml" --target-dir "$SCRIPT_DIR/target" || exit 1
else
  echo "Error: cargo not found. Install Rust via https://rustup.rs" >&2
  exit 1
fi

exec "$SCRIPT_DIR/target/debug/training" "$@"