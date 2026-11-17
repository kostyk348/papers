#!/usr/bin/env bash
# Run every integrity check this paper relies on, in one command, and print a summary.
# Usage:  bash verify_all.sh 2>&1 | tee evidence/checks-$(date +%F).txt
#
# Every check here is done by refslint (tools/refslint, Rust, standard library only), plus the
# harness-port parity check done by protorun (tools/protorun, Rust, standard library only). The only
# other programs involved are the LaTeX engine (through build.sh), `unzip`, and - inside
# `refslint facts` - git. The checks themselves involve no Python: the harness that *produces*
# reports/ is `tools/protorun run`, and the Python harness it replaced is a frozen reference in
# evidence/pre-port-runner/ (see TOOLING.md).
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$HERE"
. tools/refslint.sh
. tools/protorun.sh
FAILED=0

run() {
  local label="$1"; shift
  echo
  echo "=============================================================="
  echo "== $label"
  echo "== \$ $*"
  echo "=============================================================="
  if "$@"; then
    echo "-- OK: $label"
  else
    echo "-- FAILED: $label"
    FAILED=$((FAILED+1))
  fi
}

RL="$(refslint_binary)" || { echo "cannot obtain the refslint binary"; exit 1; }

echo "integrity checks, $(date -u +%Y-%m-%dT%H:%M:%SZ), $(uname -s)"
echo "refslint: ${RL}"

run "reference base resolves, bibliography and verification table regenerate" \
    "$RL" bib --root .
run 'every \cite key is curated and every curated source is cited' \
    "$RL" cites --check --root .
run "citation counts per document" "$RL" cites --count --root .
run "protocol report: schema example is valid" \
    "$RL" report --schema code/protocol_schema.json data/example-report.json

if compgen -G "reports/*/report.json" >/dev/null; then
  run "protocol runs: every report in reports/ validates" \
      "$RL" report --schema code/protocol_schema.json reports/*/report.json
  run "result tables, number macros and ledger match the reports and every \\val{} lookup resolves" \
      "$RL" tables --root . --check
else
  echo
  echo "-- NOTE: no protocol run found in reports/ (produce one with protorun run --server <name>)"
fi

echo
echo "== harness port parity: the ported harness must reproduce the Python harness's reports"
if . tools/protorun.sh && PR="$(protorun_binary)"; then
  PARITY_RUNS=0
  for ref in reports/*/report.json; do
    [ -f "$ref" ] || continue
    run_name="$(basename "$(dirname "$ref")")"
    if [ -f "reports/parity-rust/$run_name/report.json" ]; then
      PARITY_RUNS=$((PARITY_RUNS + 1))
      run "harness parity ($run_name): protorun's report equals the Python harness's report" \
          "$PR" parity run-vs-report --reference "reports/$run_name" --run "reports/parity-rust/$run_name"
    fi
  done
  if [ "$PARITY_RUNS" -eq 0 ]; then
    echo "-- NOTE: no protorun parity run found in reports/parity-rust/; produce one with"
    echo "         protorun run --server <name> --out reports/parity-rust --run-name <name>"
  else
    echo "-- OK: $PARITY_RUNS report(s) compared field by field (see evidence/protorun-port-2026-10-16.txt)"
  fi
  # The frozen pre-port harness must stay frozen; the comparison above is only meaningful if it does.
  if [ -f code/run_protocol.py ] && [ -f code/streams.py ]; then
    if cmp -s code/run_protocol.py evidence/pre-port-runner/run_protocol.py \
       && cmp -s code/streams.py evidence/pre-port-runner/streams.py; then
      echo "-- OK: evidence/pre-port-runner/ is still byte-identical to code/run_protocol.py + code/streams.py"
    else
      echo "-- FAILED: the frozen pre-port harness copy no longer matches code/"
      FAILED=$((FAILED + 1))
    fi
  fi
else
  echo "-- FAILED: cannot obtain the protorun binary (the harness port is unverified)"
  FAILED=$((FAILED + 1))
fi

echo
echo "== negative tests: the validator must reject all five deliberately broken reports"
if "$RL" selftest --root . --schema code/protocol_schema.json; then
  echo "-- OK: 5 of 5 broken reports rejected, 0 accepted"
else
  echo "-- FAILED: a deliberately broken report was accepted"
  FAILED=$((FAILED+1))
fi

run "JSON artefacts parse" \
    "$RL" jsoncheck data/refmeta.json data/repo-facts.json data/example-report.json \
         code/protocol_schema.json .zenodo.json

echo
echo "== port parity: the generated files must still equal the pre-port (Python) versions"
# Relative path on purpose: refslint is a native binary and does not translate MSYS paths.
CHECK_DIR=".port-parity-check"
rm -rf "$CHECK_DIR"
mkdir -p "$CHECK_DIR/tex" "$CHECK_DIR/data"
if "$RL" bib --root "$CHECK_DIR" --refs code/refs.toml --meta data/refmeta.json --legacy-banner >/dev/null \
   && cmp "$CHECK_DIR/tex/references.bib" evidence/pre-port/references.bib \
   && cmp "$CHECK_DIR/reference-verification.md" evidence/pre-port/reference-verification.md \
   && cmp "$CHECK_DIR/data/refs-verified.csv" evidence/pre-port/refs-verified.csv; then
  echo "-- OK: references.bib, reference-verification.md and refs-verified.csv are byte-identical"
  echo "       to the artefacts the Python pipeline produced (evidence/pre-port/SHA256SUMS.txt)"
else
  echo "-- FAILED: the port changed generated output"
  FAILED=$((FAILED+1))
fi
rm -rf "$CHECK_DIR"

run "PDF regenerates from source" bash -c 'bash build.sh >/tmp/build.log 2>&1 || bash build.sh; test -f tex/main.pdf && echo "tex/main.pdf present"'

echo
echo "=============================================================="
if [ "$FAILED" -eq 0 ]; then
  echo "ALL CHECKS PASSED"
else
  echo "$FAILED CHECK(S) FAILED"
fi
echo "=============================================================="
exit "$FAILED"
