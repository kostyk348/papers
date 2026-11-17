# Changelog

One DOI per version. Edits after a deposit become a new version here.

## 0.1.0 -- 2026-11-17

First complete package.

- `experiments/plan.toml`: nine measured configurations (three cube sizes on the torch backend,
  three on the NumPy backend, three VM instruction counts), five repeats each, plus thirteen derived
  quantities.
- The derived quantities are the point of the note: the ratio the audited table implies (1138), the
  matched-unit ratios (4.2 at cube 16, 5.3 at cube 24, 14.4 at cube 32), the backend factor (3.53 in
  NumPy's favour at cube 24) and the volume of the reported cube.
- Paper: `tex/main.tex`, eight sections, two tables, 16 verified references.
- A correction found while writing: the note first reported the backend spread as 0.28 (torch over
  NumPy); the derive was redefined as NumPy over torch so the sentence reads "3.5 times faster"
  instead of "a factor of 0.28 in its favour". Both the wrong and the right version are recorded
  here rather than silently dropped.
- `bash verify_all.sh` → `ALL CHECKS PASSED`, transcript in `evidence/checks-2026-11-17.txt`.
