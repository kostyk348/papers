#!/usr/bin/env bash
# Build the paper PDF.  One command, no configuration:  bash build.sh
#
# Two third-party CLIs are used and nothing else:
#   * a LaTeX engine that can fetch its own packages - tectonic, a single binary with a
#     self-managed bundle, looked up in this order:
#        1. $TECTONIC if set      2. tectonic on PATH
#        3. tools/tectonic(.exe) inside this folder, downloaded from the pinned release below
#     Override the pinned version with TECTONIC_VERSION=<tag>.
#   * `unzip` (or PowerShell) to unpack the pinned release into tools/tectonic.
#
# The bibliography and the citation-key check are done by refslint (tools/refslint, Rust,
# built by cargo on first use - see tools/refslint.sh).  No Python is involved.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$HERE"
. tools/refslint.sh

VERSION="${TECTONIC_VERSION:-tectonic@0.17.0}"
ASSET="tectonic-0.17.0-x86_64-pc-windows-msvc.zip"
URL="https://github.com/tectonic-typesetting/tectonic/releases/download/tectonic%400.17.0/${ASSET}"

find_tectonic() {
  if [ -n "${TECTONIC:-}" ] && [ -x "${TECTONIC}" ]; then echo "${TECTONIC}"; return 0; fi
  if command -v tectonic >/dev/null 2>&1; then command -v tectonic; return 0; fi
  for cand in tools/tectonic/tectonic.exe tools/tectonic/tectonic tools/tectonic.exe tools/tectonic; do
    if [ -x "$cand" ]; then echo "$cand"; return 0; fi
  done
  return 1
}

unpack_zip() {
  local archive="$1" dest="$2"
  mkdir -p "$dest"
  if command -v unzip >/dev/null 2>&1; then
    unzip -oq "$archive" -d "$dest"
  elif command -v powershell.exe >/dev/null 2>&1; then
    powershell.exe -NoProfile -Command "Expand-Archive -Force '${archive}' '${dest}'"
  else
    echo "[build] need unzip or PowerShell to unpack ${archive}" >&2
    return 1
  fi
}

TEC="$(find_tectonic || true)"
if [ -z "${TEC}" ]; then
  echo "[build] tectonic not found; downloading ${VERSION} (${ASSET})"
  mkdir -p tools
  if command -v curl >/dev/null 2>&1; then
    curl -sSL -o "tools/${ASSET}" "${URL}"
  else
    echo "[build] curl is required to fetch tectonic, or install tectonic manually" >&2
    exit 1
  fi
  unpack_zip "tools/${ASSET}" tools/tectonic
  TEC="$(find_tectonic || true)"
fi

if [ -z "${TEC}" ]; then
  echo "[build] could not obtain a LaTeX engine" >&2
  exit 1
fi
echo "[build] using ${TEC}"

RL="$(refslint_binary)" || exit 1
echo "[build] using ${RL}"

# 1. regenerate the bibliography from the curated, verified source list, then check every
#    \cite key against it
"${RL}" bib --root .
"${RL}" cites --check --root .

# 1b. regenerate the result tables, the number macros and the number ledger from the protocol
#     reports, and refuse to compile if the text uses a number that no report contains
"${RL}" tables --root .

# 2. compile (tectonic runs BibTeX as needed and repeats passes)
"${TEC}" -X compile tex/main.tex --outdir tex --keep-logs

echo "[build] ok -> tex/main.pdf"
