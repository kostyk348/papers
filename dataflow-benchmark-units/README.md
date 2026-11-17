# Units before speed

A measurement-methodology note on the benchmark published by `kostyk348/tensor-broth`, which
compares a von Neumann interpreter loop with a 3D "spatial dataflow" engine and reads the
resulting table as a paradigm verdict.

**Status: 0.1.0, package complete, `bash verify_all.sh` → ALL CHECKS PASSED.**
PDF: `tex/main.pdf` (6 pages). Every number in it is generated from `data/measurements.json`.

## The finding in three numbers

| What the audited table implies | What a common unit gives | What the implementation does |
|---|---|---|
| **1138×** advantage, from dividing giga-ops by mega-ops | **5.3×** advantage in state updates/s at the reported cube 24, **14.4×** at cube 32 | at cube 24 the "slow" NumPy backend of the same engine is **3.5× faster** than the torch path the code calls fast |

1. The VM column counts instructions, each writing one cell. The broth column counts 81
   multiply-accumulates per voxel update. A ratio between the columns is mostly a units conversion.
2. With a matched unit the advantage exists, and it grows with the volume per tick (4.2× at cube
   16, 5.3× at 24, 14.4× at 32). That growth is the scientifically interesting part and it is
   invisible in a single row.
3. The same engine moves by a factor of 3.5 when the backend changes at a fixed cube, in the
   direction opposite to the code comment, which is larger than the paradigm factor at the reported
   size. The comparison as printed does not isolate its own variable.

The note ends with the five-line list of what such a benchmark should print (units, both
implementations, repeats with spread, machine and versions, size scaling).

## How to reproduce

```bash
bash scripts/fetch_subjects.sh     # clone the subject at the pinned commit
papertool measure --root .         # run the plan, keep logs, aggregate
papertool measure --root . --derive-only
papertool ledger --root .          # measurements -> tex/numbers.tex, data/numbers.tsv
bash verify_all.sh                 # all checks + PDF
```

## What is in the package

| Path | What it is |
|---|---|
| `experiments/plan.toml` | The predeclared plan: nine configurations, five repeats each, twelve derived quantities with their formulas |
| `experiments/subjects.lock` | The audited repository and the exact commit |
| `data/measurements.json` | Every measurement, its raw value list, the per-run peak working set, and the derived values |
| `data/numbers.tsv` | One row per number in the note, with the JSON path it came from |
| `data/sources.toml`, `data/refmeta.json`, `data/refs-verified.csv` | The curated sources, the machine snapshot, the verification record (16 sources) |
| `data/numeral-allowlist.tsv` | Every numeral in the prose that is not a measurement, with the reason |
| `evidence/raw/<run>/` | stdout and stderr of every process that ran |
| `tex/main.tex`, `tex/numbers.tex`, `tex/references.bib` | The note, the generated macros, the generated bibliography |

## Limits, stated where they belong

The note does not rank the paradigms: a Python interpreter loop is not a von Neumann machine and a
torch convolution is not a dataflow chip, so what is measured is the size of the confound. Absolute
Gops/s differ from the README by several times because the hardware differs, which is one of the
reasons absolute cross-machine rates should not be printed without the machine. Nothing here
measures energy. One machine, one library version; the NumPy-versus-torch crossover is a function
of cube size and of those versions, and the note measures one point on it.

## Cite this

`CITATION.cff` (author: Konstantin Parashchuk). `.zenodo.json` holds the deposit metadata; the note
is not yet deposited, so there is no DOI.

## Licence

Text: CC BY 4.0 (`LICENSE`). Code and scripts: Apache-2.0 (`LICENSE-CODE`). The audited repository
is distributed by its own author under Apache-2.0 and is not redistributed here.
