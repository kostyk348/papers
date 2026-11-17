#!/usr/bin/env bash
# Build the paper PDF.  One command, no configuration:  bash build.sh
#
# Two third-party programs are involved and nothing else:
#   * a LaTeX engine that fetches its own packages -- tectonic, a single binary with a
#     self-managed bundle, looked up in this order:
#        1. $TECTONIC if set    2. tectonic on PATH
#        3. tools/tectonic/tectonic.exe in this paper       4. ../tooling/tectonic/tectonic.exe
#     If none is found the build downloads the pinned release (needs curl + unzip).
#   * the LaTeX engine's own BibTeX run, which tectonic performs itself.
#
# Everything else -- the bibliography, the citation check, the number macros, the ledger,
# the figure data and the "no invented numbers" check -- is papertool (Rust, standard
# library only), built by cargo on first use.  No Python is involved in the paper pipeline.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$HERE"
. tools/papertool.sh

VERSION_TAG="tectonic@0.17.0"
ASSET="tectonic-0.17.0-x86_64-pc-windows-msvc.zip"
URL="https://github.com/tectonic-typesetting/tectonic/releases/download/tectonic%400.17.0/${ASSET}"

find_tectonic() {
  if [ -n "${TECTONIC:-}" ] && [ -x "${TECTONIC}" ]; then echo "${TECTONIC}"; return 0; fi
  if command -v tectonic >/dev/null 2>&1; then command -v tectonic; return 0; fi
  for cand in tools/tectonic/tectonic.exe tools/tectonic/tectonic \
              ../tooling/tectonic/tectonic.exe ../tooling/tectonic/tectonic; do
    if [ -x "$cand" ]; then echo "$cand"; return 0; fi
  done
  return 1
}

TEC="$(find_tectonic || true)"
if [ -z "${TEC}" ]; then
  echo "[build] tectonic not found; downloading ${VERSION_TAG}"
  mkdir -p tools
  curl -sSL -o "tools/${ASSET}" "${URL}"
  if command -v unzip >/dev/null 2>&1; then
    unzip -oq "tools/${ASSET}" -d tools/tectonic
  else
    powershell.exe -NoProfile -Command "Expand-Archive -Force 'tools/${ASSET}' 'tools/tectonic'"
  fi
  TEC="$(find_tectonic || true)"
fi
[ -n "${TEC}" ] || { echo "[build] could not obtain a LaTeX engine" >&2; exit 1; }
echo "[build] latex: ${TEC}"

PT="$(papertool_binary)" || exit 1
echo "[build] papertool: ${PT}"

# 1. bibliography from the curated, verified source list; then every \cite key must resolve
"${PT}" bib --root .
"${PT}" cites --root . --check

# 2. number macros, ledger and figure data from the measurements; the prose may only use
#    numbers that exist there
"${PT}" ledger --root .
"${PT}" numbers --root .

# 3. compile (tectonic runs BibTeX and repeats passes as needed)
"${TEC}" -X compile tex/main.tex --outdir tex --keep-logs

echo "[build] ok -> tex/main.pdf"
