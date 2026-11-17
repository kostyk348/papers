#!/usr/bin/env bash
# Everything this paper claims about itself, checked in one command.
#
#   bash verify_all.sh
#
# Each step is a refusal condition, not a report: a stale generated file, an unverified
# reference, an uncited source, a number with no measurement behind it, or a PDF that does
# not build all end with a non-zero exit code. The transcript is written to
# evidence/checks-<date>.txt so the run can be cited.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$HERE"
. tools/papertool.sh

PT="$(papertool_binary)" || { echo "FATAL: papertool did not build"; exit 1; }
STAMP="$(date +%Y-%m-%d)"
LOG="evidence/checks-${STAMP}.txt"
mkdir -p evidence

fail=0
run() {
  local label="$1"; shift
  echo "=== ${label}"
  if "$@"; then
    echo "    ok"
  else
    echo "    FAILED"
    fail=$((fail + 1))
  fi
}

{
  echo "verify_all.sh -- $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "papertool: ${PT}"
  "$PT" version 2>/dev/null || true
  echo

  run "JSON artefacts parse" "$PT" jsoncheck data/measurements.json data/refmeta.json
  run "measurements match the plan (every run present, right n)" \
      "$PT" measure --root . --check
  run "ledger and figure data are what the measurements produce" \
      "$PT" ledger --root . --check
  run "bibliography rebuilds from the curated, verified sources" \
      "$PT" bib --root .
  run "every \\cite has a curated source and every source is cited" \
      "$PT" cites --root . --check
  run "every number macro resolves and no bare numeral is unlisted" \
      "$PT" numbers --root .

  # the PDF is rebuilt last: it is the only step that depends on all of the above
  echo "=== PDF build"
  if bash build.sh > /tmp/build.log 2>&1; then
    echo "    ok ($(wc -c < tex/main.pdf) bytes)"
    if command -v sha256sum >/dev/null 2>&1; then
      echo "    sha256 $(sha256sum tex/main.pdf | cut -d' ' -f1)"
    fi
  else
    echo "    FAILED (see /tmp/build.log)"
    tail -20 /tmp/build.log
    fail=$((fail + 1))
  fi

  echo
  if [ "$fail" -eq 0 ]; then
    echo "ALL CHECKS PASSED"
  else
    echo "$fail CHECK(S) FAILED"
  fi
} 2>&1 | tee "$LOG"

case "$(tail -1 "$LOG")" in
  "ALL CHECKS PASSED") exit 0 ;;
  *) exit 1 ;;
esac
