#!/usr/bin/env bash
# Runs the Tauri CLI for tauri-action (its `tauriScript`) and turns build
# errors into GitHub annotations, which are visible on the run page without
# opening (or being signed in to read) the full log.
set -o pipefail
log="$(mktemp)"
npx tauri "$@" 2>&1 | tee "$log"
status=${PIPESTATUS[0]}
if [ "$status" -ne 0 ]; then
  sed -E 's/\x1b\[[0-9;]*m//g' "$log" \
    | grep -E "error|Error|failed|panicked" \
    | grep -vE "^\s*warning" \
    | tail -8 \
    | while IFS= read -r line; do
        line="${line//'%'/%25}"
        echo "::error title=tauri $1 failed::${line}"
      done
fi
exit "$status"
