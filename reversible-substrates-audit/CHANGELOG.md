# Changelog

One DOI per version. Edits after a deposit become a new version here, with the change
recorded before the DOI is minted.

## 0.1.0 -- 2026-11-17

First complete package.

- `experiments/plan.toml`: 29 measured runs and 16 derived quantities, written before the
  numbers were read.
- Harness: `papertool measure` (peak working set polling, raw logs, aggregation) and
  `papertool ledger` (number macros, ledger, figure data), both Rust, standard library only.
- Paper: `tex/main.tex`, nine sections, five tables, two figures, 23 verified references.
- The audited repository's four printed verdicts reproduce exactly on this machine; the
  package records that as four runs (`margolus_size16`, `train_depth12`, `topo_size32`,
  `hpp_size64`) whose logs are in `evidence/raw/`.
- Deliberate start-over: the first pass of the plan omitted explicit `--size` flags, so
  three runs silently measured the CLI defaults. The plan now passes every parameter
  explicitly and the measurements were regenerated from scratch. The mistake is recorded
  here because it is the failure mode this package exists to prevent.
- `bash verify_all.sh` → `ALL CHECKS PASSED`, transcript in `evidence/checks-2026-11-17.txt`.
