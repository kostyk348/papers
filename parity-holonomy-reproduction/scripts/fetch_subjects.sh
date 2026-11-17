#!/usr/bin/env bash
# Clone the audited repository at the exact commit the paper measured.
#
# The subject is not vendored: it is somebody else's code, it is pinned by SHA in
# subjects.lock, and the plan records the SHA in data/measurements.json so a measurement
# can never be silently re-attributed to a different revision.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$HERE"
repo=$(sed -n 's/^repo=//p' experiments/subjects.lock)
sha=$(sed -n 's/^sha=//p' experiments/subjects.lock)
dest=$(sed -n 's/^dir=//p' experiments/subjects.lock)
if [ -z "$repo" ] || [ -z "$sha" ] || [ -z "$dest" ]; then
  echo "experiments/subjects.lock must define repo= sha= dir=" >&2
  exit 1
fi
if [ -d "$dest/.git" ]; then
  echo "[subjects] $dest already cloned at $(git -C "$dest" rev-parse HEAD)"
else
  mkdir -p "$(dirname "$dest")"
  git clone --quiet "https://github.com/${repo}.git" "$dest"
fi
git -C "$dest" fetch --quiet origin "$sha" 2>/dev/null || true
git -C "$dest" checkout --quiet "$sha"
actual=$(git -C "$dest" rev-parse HEAD)
if [ "$actual" != "$sha" ]; then
  echo "[subjects] expected $sha, have $actual" >&2
  exit 1
fi
echo "[subjects] $repo at $actual"
