#!/bin/bash
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
export PYTHONPATH="$SCRIPT_DIR/src:$PYTHONPATH"
python3 "$SCRIPT_DIR/src/main.py" "$@"