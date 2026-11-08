#!/usr/bin/env bash
# Runs the Tauri CLI in CI (tauri-action's `tauriScript`, and build.yml) and
# turns build errors into GitHub annotations, which are visible on the run
# page without opening (or being signed in to read) the full log.
#
# Bundling downloads its tools from GitHub (AppImage's AppRun and
# linuxdeploy, NSIS), which now and then answers 5xx or times out: a build
# that failed on a download is tried again, up to three times in all. Other
# failures are not retried.
set -o pipefail
log="$(mktemp)"
attempts=3
for attempt in $(seq 1 "$attempts"); do
  npx tauri "$@" 2>&1 | tee "$log"
  status=${PIPESTATUS[0]}
  [ "$status" -eq 0 ] && break
  if [ "$attempt" -lt "$attempts" ] \
    && grep -qiE "http status: 5[0-9][0-9]|failed to download|timed out|connection (reset|refused)|error sending request" "$log"; then
    echo "::warning title=tauri $1 retried::A download failed (attempt $attempt of $attempts); trying again in 30 s"
    sleep 30
    continue
  fi
  break
done
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
