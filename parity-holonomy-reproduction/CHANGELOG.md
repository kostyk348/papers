# Changelog

One DOI per version. Edits after a deposit become a new version here, with the change
recorded before the DOI is minted.

## 0.1.0 -- 2026-11-17

First complete package.

- `experiments/plan.toml`: six re-run experiments (with repeats where the experiment's own RNG
  is seeded differently), twelve table extractions, three derived quantities.
- Reproduction: all seven outputs byte-identical to `results/*.txt` at commit `02c66ff`.
- Claim 4 (novelty) was first excluded for a runtime reason that turned out to be wrong: the
  experiment finishes in minutes and now reproduces byte-identically (0 / 14 / 66 / 314 class
  changes). Included in §5.4 and in the verdict table.
- Claim 2 (defect thermodynamics) stays in the plan with a 45-minute budget but is not reported as
  a result; its verdict remains the repository's own. The paper states which of the two is a limit
  and which was an omission.
- Paper: `tex/main.tex`, eight sections, three tables, one figure, 18 verified references.
- The paper's own correction to the audited text: the third verdict ("a checksum, not a
  memory") is turned into a measured statement about minimum distance, using the audited
  experiment that counts undetectable errors on bridges.
- `bash verify_all.sh` → `ALL CHECKS PASSED`, transcript in `evidence/checks-2026-11-17.txt`.
