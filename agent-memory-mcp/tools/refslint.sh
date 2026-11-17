#!/usr/bin/env bash
# Locate the refslint binary, building it with cargo if it has not been built yet.
#
# Source this file (`. tools/refslint.sh`) and call `refslint_binary`; it prints the path
# of the binary on success. Search order:
#   1. $REFSLINT if set and executable
#   2. tools/refslint/target/{release,debug}/refslint(.exe)
#   3. `cargo build --release` inside tools/refslint, then search again
#
# The binary is never committed (see .gitignore): the build is reproducible from the
# committed Rust source, standard library only, no network and no crates to fetch.

refslint_candidates() {
  echo "${REFSLINT:-}"
  echo "tools/refslint/target/release/refslint.exe"
  echo "tools/refslint/target/release/refslint"
  echo "tools/refslint/target/debug/refslint.exe"
  echo "tools/refslint/target/debug/refslint"
}

refslint_binary() {
  local cand
  for cand in $(refslint_candidates); do
    if [ -n "$cand" ] && [ -x "$cand" ]; then
      echo "$cand"
      return 0
    fi
  done
  if command -v cargo >/dev/null 2>&1 && [ -d tools/refslint ]; then
    echo "[refslint] building tools/refslint with cargo (stdlib only, no downloads)" >&2
    (cd tools/refslint && cargo build --release) >&2 || return 1
    for cand in $(refslint_candidates); do
      if [ -n "$cand" ] && [ -x "$cand" ]; then
        echo "$cand"
        return 0
      fi
    done
  fi
  echo "[refslint] not found: set REFSLINT=<path> or install cargo" >&2
  return 1
}
