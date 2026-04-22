#!/bin/bash
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
export PYTHONPATH="$SCRIPT_DIR/src:$PYTHONPATH"
export XDG_DATA_DIRS="$SCRIPT_DIR${XDG_DATA_DIRS:+:$XDG_DATA_DIRS}"
python3 "$SCRIPT_DIR/src/main.py" "$@"