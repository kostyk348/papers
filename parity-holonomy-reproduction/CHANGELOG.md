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
- Claim 2 (defect thermodynamics) turned out to run in about five minutes as well, so it is now
  reproduced too: stationary density (0.70-0.72), Poisson-like event counts, Hurst 0.092. The
  repository's negative verdict is confirmed rather than overturned. No claim is left unchecked.
- Plans invoke `${PYTHON}` instead of an absolute interpreter path; `papertool measure` expands it
  from the environment and fails loudly if it is unset. This replaces the first attempt at path
  anonymisation, which replaced the interpreter with a bare `python` that had no NumPy and silently
  produced an empty run.
- Paper: `tex/main.tex`, eight sections, three tables, one figure, 18 verified references.
- The paper's own correction to the audited text: the third verdict ("a checksum, not a
  memory") is turned into a measured statement about minimum distance, using the audited
  experiment that counts undetectable errors on bridges.
- `bash verify_all.sh` → `ALL CHECKS PASSED`, transcript in `evidence/checks-2026-11-17.txt`.
