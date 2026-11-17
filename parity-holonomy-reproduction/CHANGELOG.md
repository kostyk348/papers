# Changelog

One DOI per version. Edits after a deposit become a new version here, with the change
recorded before the DOI is minted.

## 0.1.0 -- 2026-11-17

First complete package.

- `experiments/plan.toml`: six re-run experiments (with repeats where the experiment's own RNG
  is seeded differently), twelve table extractions, three derived quantities.
- Reproduction: all six outputs byte-identical to `results/*.txt` at commit `02c66ff`.
- Two of the five claims (defect thermodynamics, novelty) are excluded from the re-run for
  runtime reasons and are reported as the repository's own verdicts; §6 says so.
- Paper: `tex/main.tex`, eight sections, three tables, one figure, 18 verified references.
- The paper's own correction to the audited text: the third verdict ("a checksum, not a
  memory") is turned into a measured statement about minimum distance, using the audited
  experiment that counts undetectable errors on bridges.
- `bash verify_all.sh` → `ALL CHECKS PASSED`, transcript in `evidence/checks-2026-11-17.txt`.
