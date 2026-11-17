<div align="center">

<img src="docs/banner.svg" alt="papers banner" width="1000">

# papers

### Paper and preprint packages: manuscript sources, artefacts, and the tooling that regenerates every number

[![Release](https://img.shields.io/github/v/release/kostyk348/papers?sort=semver&color=0ea5e9&label=release)](https://github.com/kostyk348/papers/releases)
[![Language](https://img.shields.io/badge/lang-Rust-blueviolet)](https://github.com/kostyk348/papers)

</div>

---


Paper and preprint packages by Konstantin Parashchuk (GitHub [@kostyk348](https://github.com/kostyk348)).
Each directory is a self-contained package: manuscript sources, the artefacts the manuscript claims, and the
tools that regenerate every number in it.

| Package | Version | What it contains | Status |
|---|---|---|---|
| [Memory Operations in MCP Agent Ecosystems](agent-memory-mcp/) | 1.0 | operation taxonomy (6 operations, 2 record axes), evaluation protocol (10 task families, machine-validated report format), verified reference base (135 sources), two completed protocol runs on two memory servers | [released v1.0](https://github.com/kostyk348/papers/releases/tag/agent-memory-mcp-v1.0) — DOI pending (Zenodo deposit not yet made) |
| [Four reversibility claims, re-measured](reversible-substrates-audit/) | 0.1 | independent audit of four reversible-computing laboratories in `timelock-ca`: 29 measured runs (lattice sizes, step counts, 80 lattice-gas seeds, depths 2–48, batches 100–6400) with peak working set sampled per process; verdicts: two claims hold, one weakens, one (emergent attraction) fails | package complete, `verify_all.sh` green — DOI pending |
| [Parity holonomy in Boolean constraint networks](parity-holonomy-reproduction/) | 0.1 | reproduction study of the five verdicts published by `paradox-logic`: eight experiments re-run from a pinned commit, byte-identical output, four claims checked against numbers, and the parity invariant's minimum distance measured (one) | package complete, `verify_all.sh` green — DOI pending |
| [Units before speed](dataflow-benchmark-units/) | 0.1 | measurement-methodology note on the `tensor-broth` benchmark: the implied 1138× paradigm advantage becomes 5.3× in a matched unit, and the backend choice moves the same engine by 3.5× | package complete, `verify_all.sh` green — DOI pending |

The three audit packages share one harness, [`tooling/papertool`](tooling/papertool) (Rust, standard library only):
`bib`/`cites` for verified references, `measure` for the experiment plan (raw logs, peak working set,
aggregation, derived quantities), `ledger` for the number macros and figure data, `numbers` for the
"no invented numbers" gate. `tooling/qa/` holds the two PDF-inspection scripts, which are report-only
and not part of any pipeline.

## Principles these packages follow

- **No number is typed by hand.** Every table and every value in the text is generated from the report files that
  the protocol produced, by the tooling in the package (`refslint tables`, `refslint bib`). A digit in the manuscript
  exists only as a macro resolved from those files.
- **A claim is published only with a machine-checkable support.** Each package carries a claim ledger with an
  epistemic status per assertion (verified artefact / verified literature / position / not confirmed) and commands
  that reproduce it.
- **Tooling is compiled, not scripted.** The pipelines are single Rust binaries with subcommands, built with no
  external crates where the job allows it, and they ship with self-tests (including negative tests: a validator that
  accepts a valid input proves nothing, so each package includes deliberately broken inputs that must be rejected).
- **Failures are published too.** Runs that expose a defect in a measured system, and limitations of the measurement
  itself, are stated in the manuscript rather than smoothed over.

## Building a package

```sh
cd agent-memory-mcp
bash build.sh          # builds tex/main.pdf from sources + generated tables
```

The three audit packages build the same way and additionally need their subject cloned once:

```sh
cd reversible-substrates-audit
bash scripts/fetch_subjects.sh   # clone the audited repository at the pinned commit
bash build.sh                    # bibliography, number checks, PDF
bash verify_all.sh               # all of it, plus the freshness checks
```

Their `experiments/plan.toml` invokes `python` from `PATH`; the interpreter used for the published
measurements is recorded in the plan's `[meta]` block and in `data/measurements.json`.

PDF builds use `tectonic` or a TeX Live installation (the engine is not vendored in this repository).

## Note on anonymised paths

Before publication, local absolute paths in the run logs, reports and evidence files were replaced:
the user home prefix became `{{HOME}}` and the interpreter location became `<hermes-venv>`. Nothing else was
changed — the executed code, the raw protocol exchanges and every reported value are as produced.
Hashes recorded in the port/parity evidence files were computed before this substitution, so re-running the tools
reproduces the same content except for those two placeholders.
