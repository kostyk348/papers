# Tooling: language policy, the ported pipeline, and the harness

## The rule (from the user, posted 2026-10-16 mid-run on card t_6b4b6255)

> Новых инструментов на Python не добавлять — утилиты и скрипты пишем на Rust (лучше одна
> утилита с подкомандами), Python только там, где без него нельзя (сборка PDF через зрелые
> библиотеки, чужие CLI).

This rule arrived **after** the tooling in `code/` had already been written and verified (0.1.0).
It was recorded here rather than silently ignored, and the port became the first item of the
continuation card (t_6b4b6255). **The port was carried out in 0.3.0**: the whole pipeline now runs
from one Rust binary, `tools/refslint` (standard library only, no crates), and the change is
verified rather than declared — see "Acceptance criteria" below and
`evidence/refslint-port-2026-10-16.txt`.

## Current state (honest)

The pipeline in `code/` was Python 3 (stdlib only) and worked: `bash verify_all.sh` printed
`ALL CHECKS PASSED`. It has since been ported to `tools/refslint`; the table below is the honest
mapping from the old file to the new subcommand, including the two files that had to stay Python
longer than the rest (`validate_report.py`, deleted in 0.4.0, and the harness itself).

The protocol harness (`run_protocol.py`, `streams.py`) was added in 0.2.0 and was Python for the same
reason the rest of `code/` was: it has to interoperate with the Python MCP servers under test, and
the card that commissioned it named `code/run_protocol.py`. It was therefore listed as to-port like
the rest. That port is larger than the others: it needs an MCP stdio JSON-RPC client and a process
supervisor, and its acceptance criterion is the same as point 2 below — a Rust run of the two
subjects must reproduce the two committed `report.json` files field for field, modulo the timing
metrics. **The port was carried out in 1.1 as `tools/protorun`** (a second binary rather than a
`refslint run` subcommand: `refslint` answers for the bibliography and the report schema, and the
harness is a process supervisor that spawns a subject server — two audiences, two binaries), and it
is verified rather than declared: `protorun parity run-vs-report` compares the two reports field by
field, `protorun parity streams` compares the generated facts with the recorded requests, and the
record is `evidence/protorun-port-2026-10-16.txt`. The two Python files are frozen byte-identically
in `evidence/pre-port-runner/`; they are no longer the tool of record. **The harness carries no
validator of its own**: it spawns `refslint report` on the report it just wrote, so nothing in
`code/` is a second implementation of anything Rust already does.

| File before the port | What it does | Where it is now |
|---|---|---|
| `arxiv_search.py` | arXiv Atom queries → candidate records | `refslint fetch` (ported) |
| `verify_ids.py` | arXiv `id_list` verification of exact identifiers | `refslint ids` |
| `verify_dois.py` | Crossref `/works/{doi}` verification | `refslint doi` |
| `probe_urls.py` | HTTP GET probe of web sources with status/title capture | `refslint probe` |
| `crossref_query.py` | Crossref bibliographic search (title → DOI) | `refslint query` |
| `collect_refmeta.py` | merge raw snapshots → `data/refmeta.json` | `refslint merge` |
| `build_refs.py` | curated spec + snapshot → `references.bib`, verification table, CSV; **fails the build on any unresolved source** | `refslint bib` (deleted; output byte-identical) |
| `check_cites.py` | every `\cite` key is curated; every curated source is cited | `refslint cites --check` |
| `count_cites.py` | citation counts per document | `refslint cites --count` |
| `repo_facts.py` | walk cloned repos: HEAD SHA, tool inventory, line counts | `refslint facts` |
| `validate_report.py` | JSON-Schema-subset validator for protocol reports + semantic checks | `refslint report` — **deleted in 0.4.0**; the harness spawns the binary (it no longer imports a Python validator) |
| `make_bad_reports.py` | generates the negative tests | `refslint selftest` |
| `dump_abs.py` | dump abstracts for curation | `refslint abs` |
| `refs.py` | curated source list (Python literal) | `code/refs.toml`, converted by `refslint import-spec`; the old file is kept in `code/legacy/refs.py` |
| `run_protocol.py` | **new in 0.2.0** — the protocol harness: streams → MCP stdio JSON-RPC → raw logs → metrics → `report.json` (self-validated) | `tools/protorun run` / `streams` / `servers` (ported in 1.1); frozen copy in `evidence/pre-port-runner/` |
| `streams.py` | **new in 0.2.0** — deterministic T1–T10 stream generator with programmatic gold labels | `tools/protorun streams` (ported in 1.1, CPython-compatible MT19937 so the streams are identical); frozen copy in `evidence/pre-port-runner/` |
| `build.sh` | calls the pipeline, then compiles `tex/main.tex` | stays a shell script (calls `refslint bib` / `refslint cites`); the LaTeX engine is a third-party CLI |
| `unzip_tectonic.py` | unpacks the pinned tectonic release | deleted; `build.sh` uses `unzip`, with a PowerShell fallback |
| `code/legacy/*.py` | previous run's scripts, kept for provenance only | not ported; superseded |

## Target: one binary, `refslint` — as built

```
refslint import-spec --py code/legacy/refs.py --out code/refs.toml  # one-time spec conversion
refslint bib    [--root .] [--refs code/refs.toml] [--meta data/refmeta.json]
                [--check-only] [--legacy-banner]      # bib + verification table + csv
refslint cites  --root . [--check | --count]          # cite keys
refslint jsoncheck <file.json> ...                    # JSON artefacts parse
refslint report --schema code/protocol_schema.json <report.json> ...   # validator
refslint selftest [--root .]                          # negative tests (5/5 rejections)
refslint tables [--root .] [--check] [<report.json> ...]  # reports -> tex/tables/*.tex,
                                                      # tex/numbers.tex, data/run-numbers.tsv
refslint facts  --refs-dir <dir> --out data/repo-facts.json
refslint merge  --raw evidence/raw --out data/refmeta.json
refslint abs    --store data/refmeta.json --ids ids.txt [--n 420]
refslint fetch  --queries queries.json --out evidence/raw/arxiv.json [--max 8]   # arXiv search
refslint ids    --ids ids.txt --out evidence/raw/arxiv_ids.json                 # arXiv id_list
refslint doi    --dois dois.txt --out evidence/raw/dois.json                    # Crossref
refslint probe  --urls urls.txt --out evidence/raw/urls.json                    # HTTP probe
refslint query  --queries queries.txt --out evidence/raw/crossref.json          # Crossref search
```

The protocol harness is a **second binary**, `tools/protorun` (same policy: one binary, subcommands,
standard library only, no crates), because it is a process supervisor rather than a document tool —
it launches the subject server as a child process and talks JSON-RPC to it:

```
protorun servers [--root .]
protorun streams --seed 1 [--json | --out FILE]
protorun run     --server <name> [--server-root <dir>] [--server-python <exe>]
                 [--server-pythonpath <dir>] [--seeds 1,2,3] [--max-calls 400] [--timeout 30]
                 [--wall-clock-s 900] [--out reports] [--run-name <name>] [--quiet]
protorun parity streams       --reference <run dir> --server <name> [--seeds 1,2,3]
protorun parity run-vs-report --reference <run dir> --run <run dir>
```

`refslint run`, listed as "NOT BUILT YET" while the port was open, was realised as `protorun run`
instead of as a `refslint` subcommand; nothing calls `refslint run` and no such subcommand exists.

Crates: **none were needed.** The planned `serde`/`serde_json`/`toml`/`clap`/`quick-xml`/`ureq`
set was replaced by the documented fallback — a hand-written argument parser, JSON reader/writer,
Atom extractor, a small regex engine for the two patterns the schema uses, and a TOML-subset
reader for `code/refs.toml`. The only third-party programs involved are `curl` (HTTPS, permitted
as a foreign CLI), `git` (inside `facts`) and the LaTeX engine (through `build.sh`). The binary is
~850 KB and builds in ~20 s with no network access.

## Acceptance criteria for the port (must be verified, not declared)

1. `cargo build --release` in `tools/refslint` succeeds; the binary is not committed (add to
   `.gitignore`, exactly like `tools/tectonic`).
2. `refslint bib` reproduces `tex/references.bib`, `reference-verification.md` and
   `data/refs-verified.csv` **byte-identical** to the Python-produced versions for the current
   snapshot — that is the strongest available proof that the port changed no behaviour.
3. `refslint cites --check` exits 0 with the same counts currently reported (135 curated,
   0 unknown, 0 uncited).
4. `refslint report --schema ... --report data/example-report.json` accepts the example and
   rejects all five files in `data/bad-reports/` with the same reasons.
5. `refslint selftest` regenerates the negative tests and asserts 5/5 rejections.
6. `verify_all.sh` runs the Rust binary instead of the Python scripts; the streamed output is
   saved to `evidence/checks-<date>.txt` and says `ALL CHECKS PASSED`.
7. Only then are the Python files deleted, and the change is recorded in `CHANGELOG.md` as a new
   version with the before/after evidence.

Point 2 is the reason the port is safe to attempt at all: file-level equality against an existing
implementation turns "I rewrote it in Rust" into a checkable claim.

## Outcome (0.3.0) — each criterion with its evidence

Full record: `evidence/refslint-port-2026-10-16.txt`; the parity check is also a permanent step of
`verify_all.sh`, so it cannot silently rot.

| # | Criterion | Result |
|---|---|---|
| 1 | `cargo build --release` succeeds; binary not committed | `tools/refslint/target/release/refslint.exe`, 848 KB, built in ~20 s, no crates, no network; `.gitignore` covers `tools/refslint/target/` |
| 2 | `bib` reproduces the three generated files byte-identically | 3/3 `cmp`-clean against `evidence/pre-port/` via `--legacy-banner` (sha256 `da209f93…`, `4ad7be69…`, `2262577e…`). The default run differs from the pre-port files in exactly three lines, the banner naming its generator |
| 3 | `cites --check` exits 0 with the same counts | `citation keys used: 135 unique`, `curated keys never cited: 0`, `unknown 0`; `--count` is identical to the Python output for all 7 files |
| 4 | Validator accepts the example and rejects all five broken reports with the same reasons | `VALID` for `data/example-report.json`; 5/5 rejected, 7 problems, `diff` against the Python output empty |
| 5 | `selftest` regenerates the negative tests and asserts 5/5 | 5 of 5 rejected, 0 accepted; the regenerated `data/bad-reports/*.json` keep their exact sha256 (byte-identical to the Python-generated files) |
| 6 | `verify_all.sh` runs the Rust binary and says `ALL CHECKS PASSED` | yes — no Python in `build.sh` or `verify_all.sh`; output captured in `evidence/checks-2026-10-16.txt` |
| 7 | Only then are the Python files deleted, change recorded in `CHANGELOG.md` | 13 files deleted + `__pycache__`; `refs.py` moved to `code/legacy/`; recorded as 0.3.0. **Exception:** `code/validate_report.py` stayed, because the 0.2.0 harness imported it by name — resolved in 0.4.0 (see the last section) |

Also ported beyond the criteria and verified in the same run: `facts` (identical to the Python
`data/repo-facts.json` apart from the collection timestamp, including all six repositories' MCP tool
inventories) and `merge` (identical to the committed `data/refmeta.json` apart from the timestamp:
290 arXiv records, 25 DOIs, 25 probed URLs). The network subcommands were run live: `ids` 2/2 arXiv,
`doi` 2/2 Crossref, `probe` (HTTP 200 with the page title), `fetch` (2 queries → 6 records),
`query` (Crossref search) and `abs` from the committed snapshot.

Two honest limitations of the port:

- `refslint abs` (abstracts for curation) was ported, but its line wrapping was not compared
  line-by-line against the deleted `dump_abs.py`; it is a reading aid and no metric depends on it.
- The generated JSON keeps CRLF line endings, because that is what the Python originals produced on
  Windows and byte-identity was the acceptance criterion. A later version may re-baseline to LF with
  new hashes, as a visible change.

Environment note (cost me two runs): the binary is a native Windows program, so when it is called
from the bash shell it does **not** translate MSYS paths — `/c/tmp/x` becomes `C:\c\tmp\x`. Pass
`C:/...` or a path relative to the current directory. `verify_all.sh` therefore uses the relative
`.port-parity-check/`.

## Outcome (0.4.0) — the last Python file of the old pipeline is gone

`code/validate_report.py` has been deleted and the harness validates by spawning `refslint report`
(`tools/refslint`), obtained through `tools/refslint.sh` — the same validator `verify_all.sh`, the
five negative tests and `refslint selftest` use, so the harness can no longer drift from the
pipeline that judges its output. What was verified rather than declared:

| # | Check | Result |
|---|---|---|
| 1 | both committed runs still validate | `refslint report --schema code/protocol_schema.json reports/*/report.json` → `VALID` ×2, exit 0 |
| 2 | the harness validates with the binary and ships no Python validator | a run of `sint-memory-mcp` (seed 1) prints `VALID … / validated with: tools/refslint/target/release/refslint.exe report`, exit 0; `grep -rn "validate_report" code/*.py` returns nothing |
| 3 | exit 1 on an invalid report | with a deliberately broken subject SHA (`v1.2.3`, temporary edit, restored byte-identically) the harness prints `REPORT INVALID (2 problems): … .subject.commit_sha: 'v1.2.3' does not match pattern ^(none|[0-9a-f]{7,40})$` and exits 1 |
| 4 | exit 2 on a suspect run | a stub server that answers `initialize`/`tools/list` and dies on `tools/call` gives `RUN SUSPECT: 214 of 215 calls failed with harness-side errors`, exit 2 |
| 5 | exit 2 when no validator can be obtained | with the binary and `tools/refslint.sh` both moved aside the harness prints `RUN UNCERTIFIED: …` and exits 2 (fail-closed: an unvalidated report is not citable) |
| 6 | a fresh clone can still validate | with `tools/refslint/target/release/refslint.exe` deleted, the run rebuilds it through `tools/refslint.sh` (`cargo build --release`, stdlib only, offline) and ends `VALID`, exit 0 |
| 7 | the negative tests are unchanged by the switch | `refslint selftest` → 5 of 5 rejected, and the five `data/bad-reports/*.json` keep their exact sha256 (before/after identical) |
| 8 | `bash verify_all.sh` | `ALL CHECKS PASSED`, exit 0 (captured in `evidence/checks-2026-10-16-0.4.0.txt`) |

Full transcript: `evidence/refslint-report-2026-10-16.txt`. One deliberate loose end: the schema
example and the five broken reports still contain the sentence "this report exists to exercise
`code/validate_report.py`" inside their `disclosure` text. `refslint selftest` regenerates those
files byte for byte and this change's acceptance criterion pins their sha256, so the wording is
left untouched for a follow-up rather than silently re-baselined here.

## 1.0 — `refslint tables`: the paper's numbers stop being typed

The rule this subcommand enforces is older than the code: *no number enters the paper unless
something in this repository produces it*. Until 1.0 that held for the bibliography and for the
report validator, but paper §6 would have been the one place where a human copied 104 metric values
from a report into LaTeX — the exact failure mode the rule exists to prevent.

`refslint tables` (module `src/tables.rs`, standard library only, no crates — same rule as the rest
of the binary) reads every `reports/*/report.json`, and writes:

- `tex/tables/run-identity.tex` — subject, repository, commit, runtime, deployment, budgets,
  environment, failure counts, raw-log counts and paths of the two runs, side by side;
- `tex/tables/family-T1.tex` … `family-T10.tex` — one side-by-side table per task family: value,
  95% bootstrap CI and `n` per subject, plus the `computer` column; a metric the subject's tool
  surface cannot produce is printed as `null` and keeps its `n=0`, never as a blank;
- `tex/tables/operations.tex` — the same reading pooled by taxonomy operation, with the subject's
  own `supported` verdict;
- `tex/numbers.tex` — `\val`/`\valci`/`\valn`/`\valt`/`\valtl` lookups for every value the prose
  mentions, under both the short alias (`liquid.T1.write_precision`) and the full subject name;
- `data/run-numbers.tsv` — the ledger: subject, scope, scope id, metric, unit, JSON path, exact
  value, CI bounds, `n`, `computer`.

Design decisions worth recording:

- **Tables are not floats.** The evaluation tables are `longtable`s (and the identity table too), so
  they are printed where §6 refers to them instead of drifting behind the other floats; the earlier
  `table`+`tabular` version of the identity table ended up after the bibliography, which is how this
  was found.
- **`\small` cannot go between `\begin{longtable}` and the caption**: longtable treats it as content
  and the following `\noalign`-based rules (`\caption`, `\toprule`) fail with *Misplaced \noalign*.
  The size is set by a group around the environment instead.
- **Break opportunities, not hyphenation.** Column widths are computed from the subject count, metric
  names get `\_\hspace{0pt}` between their parts and long identity values (commit SHAs, environment
  strings) get a break opportunity after each separator and every tenth character, so a 40-character
  SHA cannot overflow its column. The build reports zero overfull boxes with two subjects.
- **Rounding is stated, exactness is kept.** The tables print at most four decimals; the ledger and
  the macros keep the value as the report has it, and the caption says which is which.
- **`--check` fails on four different things**: a generated file that differs from what the reports
  produce (`STALE`), a file that is missing, an `\input{tables/...}` in `main.tex` that no generation
  produces, and a number lookup in `main.tex` that has no field in any report (`UNKNOWN NUMBER`).
  Comment text is stripped before the scan, so a comment may mention `\val{...}` without tripping it.

`build.sh` regenerates the tables before compiling and `verify_all.sh` runs `--check`, so a number
that is edited into the text without a report behind it cannot survive a build. The `\val` macros
print `??key??` if a lookup ever fails at compile time, which is the visual counterpart of the same
check.
