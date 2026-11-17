# Four reversibility claims, re-measured

An independent audit of `kostyk348/timelock-ca`: four "laboratories" of reversible,
non-floating-point computing, each re-run with a predeclared sweep instead of the single
example the repository ships.

**Status: 0.1.0, package complete, `bash verify_all.sh` → ALL CHECKS PASSED.**
PDF: `tex/main.pdf`. Everything in it is generated from `data/measurements.json`.

## Verdict table

| # | Claim of the audited repository | What this audit measured | Verdict |
|---|---|---|---|
| 1 | Margolus block automaton runs backwards bit-exactly | 5 lattice sizes × 10 seeds, 7 step counts × 5 seeds, peak sampled per run | **holds**; the inverse is exact every time, including odd step counts |
| 2 | Euler characteristic is a conserved "topological charge" | 3 sizes × 10 seeds, plus a ten-times-longer run | **conserved, but nearly vacuous**: acceptance falls 0.382 → 0.0365 as the lattice grows |
| 3 | Lattice gas shows an emergent attractive force ("gravity from bits") | 4 sizes × 20 seeds = 80 runs | **not supported**: 46/80 positive, every mean inside its own standard error |
| 4 | Symplectic network trains XOR with O(1) memory | 6 depths × 10 seeds, 4 batch sizes × 5 seeds, peak working set sampled | **memory holds** (depth 2 vs 48 differ by 0.025 MB); **accuracy claim weakens** (reported 94.75 % is below the 10-seed mean) |

The paper's own report of these is in `tex/main.tex`, with §5 ("What this paper does not
establish") listing everything the audit did not test.

## How to reproduce

```bash
bash scripts/fetch_subjects.sh     # clone the subject at the pinned commit (c5c94bb)
papertool measure --root .         # ~9 min: runs the plan, keeps every raw log
papertool measure --root . --derive-only
papertool ledger --root .          # measurements -> tex/numbers.tex, data/numbers.tsv, figures
bash verify_all.sh                 # all checks + PDF
```

`papertool` is the Rust binary in `../tooling/papertool` (standard library only); `tools/papertool.sh`
builds it on first use with `cargo`. The LaTeX engine is tectonic, fetched by `build.sh` if missing.

## What is in the package

| Path | What it is |
|---|---|
| `experiments/plan.toml` | The predeclared experiment plan: every run, its seeds, its purpose |
| `experiments/subjects.lock` | The audited repository and the exact commit |
| `data/measurements.json` | Every aggregate with its raw value list, plus the derived quantities |
| `data/numbers.tsv` | One row per number in the paper: value, n, and the JSON path it came from |
| `data/sources.toml` | The curated source list, one `supports` sentence per reference |
| `data/refmeta.json` | The machine-collected metadata snapshot (Crossref, arXiv, HTTP probes) |
| `data/refs-verified.csv` | Every reference with the query that verified it |
| `reference-verification.md` | The same as a table, generated |
| `data/numeral-allowlist.tsv` | Every numeral in the prose that is *not* a measurement, with the reason |
| `data/figures/*.csv` | The figure data, written from the measurements by `papertool ledger` |
| `evidence/raw/<run>/` | One stdout and one stderr file per process that ran |
| `evidence/checks-<date>.txt` | The transcript of the verification run |
| `tex/main.tex` | The paper. No numeric literal that a program did not write |
| `tex/numbers.tex` | Generated number macros (`\val{run.metric}`) |
| `tex/references.bib` | Generated from the curated list; a source that does not resolve blocks the build |
| `tools/papertool.sh` | Builds and locates the Rust harness |
| `build.sh`, `verify_all.sh` | Build the PDF; check everything and refuse to pass on drift |

## The rule this package enforces

No number reaches the paper unless something in this folder produces it, and no reference
reaches it unless a machine query resolved it. `papertool numbers` fails the build on an
unknown `\val` key or on an unexplained numeral; `papertool bib` fails on an unresolved
source; `papertool cites` fails on a cited-but-uncured or curated-but-uncited key;
`papertool ledger --check` fails when a generated file does not match the measurements.

## Limits, stated where they belong

Single machine (Windows 11, CPython 3.14.6, NumPy 2.5.1), one process at a time, no GPU. Peak
working set includes the interpreter. The lattice-gas sweep rules out a single-seed force
claim; it does not establish that the force is exactly zero, and it does not identify the
mechanism. The XOR result is training-set accuracy with the repository's own radius-based
objective, not a test of parity representation. Nothing here measures energy.

## Cite this

`CITATION.cff` (author: Konstantin Parashchuk). A `.zenodo.json` is included for the deposit
metadata; the paper is not yet deposited, so there is no DOI to quote.

## Licence

Text: CC BY 4.0 (`LICENSE`). Code and scripts in this package: Apache-2.0 (`LICENSE-CODE`).
The audited repository is distributed by its own authors under Apache-2.0 and is not
redistributed here.
