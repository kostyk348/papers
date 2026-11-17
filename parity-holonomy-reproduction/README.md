# Parity holonomy in Boolean constraint networks

A reproduction study of `kostyk348/paradox-logic`: a repository that formalises "paradox as
foundation" over GF(2), turns each claim into an experiment, and publishes its own verdicts
including the ones that failed.

**Status: 0.1.0, package complete, `bash verify_all.sh` → ALL CHECKS PASSED.**
PDF: `tex/main.pdf` (8 pages). Every number in it is generated from `data/measurements.json`.

## Verdict table

| # | Claim of the audited repository | What this audit found |
|---|---|---|
| 1 | Paradox is Mermin/GHZ contextuality | **confirmed**: the invariant is non-trivial exactly when no global assignment exists; the Mermin triangle gives the expected obstruction |
| 2 | Defect thermodynamics (criticality from paradox birth/death) | **not re-run** (runtime budget); verdict taken from the committed report and labelled as such |
| 3 | Topological (parity) memory | **reproduced and sharpened**: parity survives value noise completely (0.000), degrades under structural noise (0.138 → 0.497), the Hopfield baseline saturates (0.377 → 1.000); as a code its minimum distance is **one** — 40 single edge errors on bridges are invisible to it |
| 4 | Novelty as a change of topological class | **not re-run** (runtime budget); the repository itself marks it partial |
| 5 | Cohomological diagonal lemma (self-reference ⇒ paradox) | **refuted, reproduced**: 60 of 120 self-gauge systems admit a consistent fixed point |

Reproduction result: six experiments re-run from commit `02c66ff`; **all six outputs are
byte-identical** to the `results/` files committed with the repository, including the one that
draws 4000 random trials (it is deterministically seeded, which the paper discusses: reproducible,
but its Monte Carlo error is fixed and unquantified — §4 attaches a binomial standard error to one
row to make that width visible).

## How to reproduce

```bash
bash scripts/fetch_subjects.sh     # clone the subject at the pinned commit
papertool measure --root .         # re-run the plan, keep logs, compare bytes
papertool measure --root . --derive-only
papertool ledger --root .          # measurements -> tex/numbers.tex, data/numbers.tsv
bash verify_all.sh                 # all checks + PDF
```

## What is in the package

| Path | What it is |
|---|---|
| `experiments/plan.toml` | The predeclared plan: six experiments, repeats, extractions, derived quantities |
| `experiments/subjects.lock` | The audited repository and the exact commit |
| `data/measurements.json` | Every extracted value, the byte-identity verdict per run, and the derived quantities |
| `data/numbers.tsv` | One row per number in the paper, with the JSON path it came from |
| `data/sources.toml`, `data/refmeta.json`, `data/refs-verified.csv` | The curated sources, the machine snapshot, and the verification record (18 sources) |
| `data/numeral-allowlist.tsv` | Every numeral in the prose that is not a measurement, with the reason |
| `evidence/raw/<run>/` | stdout and stderr of every process that ran |
| `evidence/checks-<date>.txt` | The transcript of the verification run |
| `tex/main.tex`, `tex/numbers.tex`, `tex/references.bib` | The paper, the generated macros, the generated bibliography |

## Limits, stated where they belong

Two of the five claims were not re-run (runtime); their verdicts are the repository's own and are
marked as such in the paper. No new theory: the distance-one statement is a standard fact about
cycle-space codes, measured here on the audited artifact. One machine (Windows 11, CPython 3.14.6,
NumPy 2.5.1). The seeding issue cuts both ways and the paper says so in §6.

## Cite this

`CITATION.cff` (author: Konstantin Parashchuk). `.zenodo.json` holds the deposit metadata; the
paper is not yet deposited, so there is no DOI.

## Licence

Text: CC BY 4.0 (`LICENSE`). Code and scripts: Apache-2.0 (`LICENSE-CODE`). The audited
repository is distributed by its own author under MIT and is not redistributed here.
