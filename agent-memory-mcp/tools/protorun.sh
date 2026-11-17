#!/usr/bin/env bash
# Locate the protorun binary, building it with cargo if it has not been built yet.
#
# Source this file (`. tools/protorun.sh`) and call `protorun_binary`; it prints the path
# of the binary on success. Search order:
#   1. $PROTORUN if set and executable
#   2. tools/protorun/target/{release,debug}/protorun(.exe)
#   3. `cargo build --release` inside tools/protorun, then search again
#
# The binary is never committed (see .gitignore): the build is reproducible from the
# committed Rust source, standard library only, no network and no crates to fetch.

protorun_candidates() {
  echo "${PROTORUN:-}"
  echo "tools/protorun/target/release/protorun.exe"
  echo "tools/protorun/target/release/protorun"
  echo "tools/protorun/target/debug/protorun.exe"
  echo "tools/protorun/target/debug/protorun"
}

protorun_binary() {
  local cand
  for cand in $(protorun_candidates); do
    if [ -n "$cand" ] && [ -x "$cand" ]; then
      echo "$cand"
      return 0
    fi
  done
  if command -v cargo >/dev/null 2>&1 && [ -d tools/protorun ]; then
    echo "[protorun] building tools/protorun with cargo (stdlib only, no downloads)" >&2
    (cd tools/protorun && CARGO_TARGET_DIR=target cargo build --release) >&2 || return 1
    for cand in $(protorun_candidates); do
      if [ -n "$cand" ] && [ -x "$cand" ]; then
        echo "$cand"
        return 0
      fi
    done
  fi
  echo "[protorun] not found: set PROTORUN=<path> or install cargo" >&2
  return 1
}
