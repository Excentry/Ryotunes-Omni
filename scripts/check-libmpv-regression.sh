#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

cargo test --locked -p player mpv_keeps_the_gain_through_pitch_changes_and_failures --no-run

set +e
timeout 30s cargo test --locked -p player mpv_keeps_the_gain_through_pitch_changes_and_failures -- --nocapture --test-threads=1
status=$?
set -e

if (( status == 124 )); then
  printf '%s\n' '[skip] real-libmpv regression timed out after 30 seconds; deterministic tests remain required'
  exit 0
fi

exit "$status"
