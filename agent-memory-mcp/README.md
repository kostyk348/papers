# agent-memory-mcp — Memory Operations in MCP Agent Ecosystems

Paper folder for the first work of the series: **Memory Operations in MCP Agent Ecosystems: an
Operation Taxonomy, an Evaluation Protocol, and a Reference Implementation**.

Author: **Konstantin Parashchuk**. Status: **version 1.0 (release version, not yet deposited)** —
structure, taxonomy, protocol, verified reference base, the protocol harness, two validated runs, and
a paper §6 that is written from those two runs. Every table and every number in §6 is generated from
the two `report.json` files by `refslint tables`; nothing in that section is typed by hand (§2c, §4b).

This folder contains artefacts, not a report about ourselves: a taxonomy of memory operations, a
runnable evaluation protocol with a machine-validated report format, a verified reference base and
the measurements that protocol produced. The rule the whole folder obeys: **no number and no
citation exists that a command in this repository cannot reproduce.**

---

## 1. What is here

| File | What it is | Machine check |
|---|---|---|
| `outline.md` | Structure of the paper, contributions, non-claims, writing order | — |
| `taxonomy.md` | Six memory operations (O1 WRITE, O2 READ, O3 CONSOLIDATE, O4 REVISE, O5 INTEND, O6 ASSERT) + PROVENANCE/LIFECYCLE axes; mapping of 7 systems; explicit differences from the 3 closest works; reference-implementation gap map | `refslint cites --check --root .` |
| `protocol.md` | Evaluation protocol: 10 task families, metric table (formula, unit, who computes it), run procedure, report format, failure taxonomy, threats to validity | `refslint report --schema code/protocol_schema.json data/example-report.json` |
| `related-work.md` | Related work, organised by theme, every claim attributed | generated table `reference-verification.md` |
| `claims.md` | Claim ledger: claim → what supports it → what would refute it → status (A artefact / B literature / C position / D NOT CONFIRMED) | see §3 for the "not confirmed" list |
| `reference-verification.md` | **Generated.** 135 curated sources with identifier, title, the claim each supports, and the API/probe that verified it | `refslint bib --root .` |
| `tex/main.tex` | English paper that compiles to PDF (version 1.0): taxonomy, protocol, reference implementation, the measured comparison, limitations, data-and-code availability | `bash build.sh` |
| `tex/tables/*.tex`, `tex/numbers.tex` | **Generated.** The comparison tables of paper §6 (run identity, the ten task families, the per-operation profiles) and the `\val`/`\valci`/`\valn`/`\valt`/`\valtl` macros that carry every number printed in a sentence — all of them read from the two reports, never typed | `refslint tables --root . --check` |
| `data/run-numbers.tsv` | **Generated.** The number ledger: one row per metric with subject, scope, unit, value, 95% CI, `n`, `computer` and the JSON path it came from | `refslint tables --root . --check` |
| `data/refmeta.json` | Metadata snapshot: 290 arXiv records, 25 DOIs, 25 probed URLs | `refslint merge --raw evidence/raw --out data/refmeta.json` (raw inputs in `evidence/raw/`) |
| `data/refs-verified.csv` | **Generated** machine-readable verification table | `refslint bib --root .` |
| `data/repo-facts.json` | Facts about the reference implementation: commit SHAs, parsed MCP tool counts, file inventory, line counts | `refslint facts --refs-dir <refs dir> --out data/repo-facts.json` |
| `data/raw/upstream/` | Cached upstream READMEs plus `manifest.json` (20 SINT repositories with head commits taken from the GitHub API) — an independent collection path that agrees with `data/repo-facts.json` on every shared commit SHA | — |
| `data/example-report.json` | A **schema example only** — every metric value is `null`; it exists so the validator has something to validate | `refslint report --schema code/protocol_schema.json data/example-report.json` |
| `data/bad-reports/*.json` | Five deliberately broken reports (negative tests for the validator) | `refslint selftest --root .` → 5/5 rejected, or `refslint report --schema code/protocol_schema.json data/bad-reports/*.json` → must exit 1 |
| `code/refs.toml` | **Curated source list** — the single source of truth for every citation: key, kind, identifier, category and the exact claim the source is cited for | `refslint bib --root .` (fails on any unresolved entry) |
| `tools/refslint/` | **Rust source of the pipeline utility** (one binary, subcommands, standard library only, no crates) | `cd tools/refslint && cargo build --release` |
| `evidence/pre-port/` | The artefacts the Python pipeline produced, frozen before the port, with `SHA256SUMS.txt` — the reference for the byte-identity check | `bash verify_all.sh` (port-parity step) |
| `evidence/refslint-port-2026-10-16.txt` | Full record of the port: build, spec conversion, byte-identity, counts, validator parity, repo facts, snapshot merge, live network calls | see the file |
| `evidence/raw/` | Raw API responses: arXiv query dumps, arXiv id_list verification, Crossref results, URL probes, the query lists | — |
| `evidence/search-log.md` | Every search query, date, result count — the basis for the novelty claim, which is otherwise a fragile claim | — |
| `verify_all.sh` | Runs every integrity check in one command and exits non-zero if any fails | `bash verify_all.sh` |
| `tools/protorun/` | **Rust source of the protocol harness** — one binary with subcommands (`streams`, `servers`, `run`, `parity`), standard library only, no crates | `cd tools/protorun && cargo build --release` |
| `tools/protorun.sh` | Finds the harness binary or builds it with cargo; sourced by `verify_all.sh` | `. tools/protorun.sh && protorun_binary` |
| `code/run_protocol.py`, `code/streams.py`, `code/servers.json` | **The pre-port Python harness** (0.2.0–1.0) and the candidate-server registry. The tool of record is now `tools/protorun`; a byte-identical copy of the two Python files is kept in `evidence/pre-port-runner/` | `python code/run_protocol.py --list-servers` |
| `evidence/pre-port-runner/` | The Python harness frozen before the port, with `SHA256SUMS.txt` — the reference the port is checked against | see the file's `README.md` |
| `evidence/protorun-port-2026-10-16.txt` | Full record of the harness port: clean build, stream parity, the two runs, the field-by-field report comparison | see the file |
| `reports/parity-rust/<run>/` | The ported harness's runs of the same two subjects on the same seeds and commits; each `report.json` is what the parity check compares against the Python-produced one | `protorun parity run-vs-report --reference reports/<run> --run reports/parity-rust/<run>` |
| `reports/<run>/report.json`, `reports/<run>/summary.md`, `reports/<run>/raw/` | **The two runs that exist** (see §4b): one per subject, each with its raw responses and the per-repetition store directory next to it | `refslint report --schema code/protocol_schema.json reports/*/report.json` |
| `evidence/checks-*.txt` | Captured output of the last full check run (`ALL CHECKS PASSED`) | `bash verify_all.sh` |
| `TOOLING.md` | Tooling language policy (user rule: new utilities in Rust, one binary with subcommands): the acceptance criteria of the port, the record of its completion, and the record that no pipeline Python remains | see the file, and `evidence/refslint-port-2026-10-16.txt` |

## 2. Build the PDF (one command)

```bash
bash build.sh          # -> tex/main.pdf
```

The script regenerates `tex/references.bib` from the curated source list, checks every citation
key, regenerates the §6 tables and the number macros from the reports, and then compiles. Those
steps are done by **refslint** (`tools/refslint`, Rust); the binary is rebuilt by cargo on first
use, which needs no network and no crates. It needs a LaTeX engine; if none is found it downloads
**tectonic 0.17.0** (a single self-contained binary that fetches its own packages) into `tools/`
and unpackes it with `unzip`/PowerShell.
Overrides: `TECTONIC=/path/to/tectonic`, `TECTONIC_VERSION=tectonic@0.17.0`, `REFSLINT=<binary>`.

On this machine the command was verified to produce `tex/main.pdf` (22 pages, version 1.0, no
undefined citations, no overfull boxes) from a clean state, including the automatic download path.

## 2b. Run every check at once

```bash
bash verify_all.sh          # -> "ALL CHECKS PASSED", exit 0; captured in evidence/checks-<date>.txt
```

Checks: reference base resolves and the bibliography regenerates; every citation key is curated
and every curated source is cited; the protocol report schema example validates; **every report in
`reports/` validates**; **the §6 tables, the number macros and the number ledger still equal what
the reports produce, and every `\val{...}` lookup in the paper resolves to a report field**; five
deliberately broken reports are rejected; all JSON artefacts parse; the generated files are still
byte-identical to the pre-port ones; the PDF rebuilds.

## 2c. Tooling: one Rust binary (`refslint`)

The standing rule for this project is that **new utilities are written in Rust, one binary with
subcommands**; Python is allowed only where nothing else can do the job (the LaTeX engine and
`curl` are third-party CLIs, which the rule permits). `TOOLING.md` records the rule, the
acceptance criteria for the port and the outcome.

```bash
cd tools/refslint && cargo build --release    # stdlib only, no crates, no network
tools/refslint/target/release/refslint help
```

Subcommands: `import-spec`, `bib`, `cites`, `jsoncheck`, `report`, `selftest`, `tables`, `facts`,
`merge`, `abs`, `fetch`, `ids`, `doi`, `probe`, `query`. `build.sh` and `verify_all.sh` obtain the
binary through `tools/refslint.sh` (which builds it if needed), so a fresh clone needs only cargo.

`refslint tables` is what makes paper §6 unforgeable: it reads `reports/*/report.json`, writes
`tex/tables/run-identity.tex`, `tex/tables/family-T1…T10.tex`, `tex/tables/operations.tex`,
`tex/numbers.tex` and `data/run-numbers.tsv`, and with `--check` fails when a generated file is
stale, when `tex/main.tex` inputs a table that was not produced, or when the text asks for a number
(`\val{...}` and friends) that no report contains. A value in the paper therefore cannot be edited
into existence: it has to exist in a validated report first.

The port is verified, not declared: `bash verify_all.sh` includes a parity step in which
`refslint bib --legacy-banner` must reproduce `tex/references.bib`,
`reference-verification.md` and `data/refs-verified.csv` byte for byte against
`evidence/pre-port/`, and `evidence/refslint-port-2026-10-16.txt` records the build, the spec
conversion (135 sources), the byte-identity, the citation counts, the validator parity (example
accepted, five broken reports rejected with the same messages), the negative self-test, the repo
facts and the snapshot merge.

**No pipeline Python remains, apart from the frozen pre-port harness.** The harness that produces
`reports/` is now `tools/protorun` (Rust, standard library only, no crates) — the port is verified
against the reports the Python harness produced, see §4b and `evidence/protorun-port-2026-10-16.txt`.
`code/run_protocol.py` and `code/streams.py` still exist because `protocol.md`, the two committed
`summary.md` files and earlier sections of this README name them by path; they are frozen
byte-identically in `evidence/pre-port-runner/`, are not wired into `build.sh` or `verify_all.sh`,
and are not the tool of record any more. `code/validate_report.py` was deleted in 0.4.0 — the
validator of a report is `refslint report`.

## 3. Reference pipeline (how "no invented citations" is enforced)

```bash
refslint fetch --queries evidence/raw/queries1.json --out evidence/raw/arxiv1.json --max 8  # collect candidates
refslint ids   --ids <ids.txt>   --out evidence/raw/arxiv_ids.json   # verify exact IDs (arXiv id_list)
refslint doi   --dois <dois.txt> --out evidence/raw/dois.json        # verify DOIs (Crossref)
refslint probe --urls <urls.txt> --out evidence/raw/urls.json        # probe web sources (HTTP status)
refslint merge --raw evidence/raw --out data/refmeta.json    # merge the above into the snapshot
refslint bib   --root .        # -> tex/references.bib, reference-verification.md, data/refs-verified.csv
refslint cites --check --root .  # every \cite key exists; every curated source is cited
```

Failure behaviour that matters: `refslint bib` **refuses to emit anything** and exits non-zero
if a curated source cannot be resolved in the snapshot. Editing a citation means editing
`code/refs.toml` (key, kind, id, category, and the exact claim it supports) and re-running the
pipeline. The curated list was converted once from the previous `code/refs.py` by
`refslint import-spec`; that file now lives in `code/legacy/refs.py` and is not read by anything.

Current state: **135 curated sources, 0 unresolved** (98 arXiv, 15 DOIs, 22 web resources),
**0 unknown citation keys, 0 uncited curated sources**.

## 4. Reference implementation (artefact of this paper)

Five of the six operations are implemented in public MCP servers, read-only for this paper:

| Operation | Repository |
|---|---|
| O1/O4 (belief upsert, contradiction, revision chain) | https://github.com/kostyk348/sint-liquid-mcp |
| O6 (epistemic status, confidence, calibration) | https://github.com/kostyk348/sint-epistemic-mcp |
| O3 (offline consolidation) | https://github.com/kostyk348/sint-dream-mcp |
| O5 (prospective memory, intention stack) | https://github.com/kostyk348/sint-prospective-mcp |
| cross-checking writes by role | https://github.com/kostyk348/sint-llm-swarm |
| core memory system / monorepo | https://github.com/kostyk348/sint |

The working repositories are **not modified** by this paper's tooling; `refslint facts` only reads
existing clones and records commit SHAs. The implementation's four known gaps are listed in
`taxonomy.md` §6 — before any measurement, on purpose.

## 4b. Run the protocol (the harness)

```bash
. tools/protorun.sh && RL="$(protorun_binary)"   # builds tools/protorun with cargo if needed

"$RL" servers                                  # candidates and the ground for each decision
"$RL" streams --seed 1                         # the T1–T10 streams a run of seed 1 would use
"$RL" run --server sint-liquid-mcp \
    --server-root  <dir with the read-only clones> \
    --server-python <python that can import mcp 1.9.4> \
    --server-pythonpath <site dir holding mcp 1.9.4> \
    --seeds 1,2,3                              # -> reports/<run>/{report.json,summary.md,raw/,data/}
refslint report --schema code/protocol_schema.json reports/*/report.json  # exit 0 = every report valid
```

The run, in order: generate the T1–T10 streams for each seed (gold labels come from the generator,
never from a model); launch the server under test as a child process with `USERPROFILE`/`HOME`
pointing at a fresh data directory per repetition; drive it over MCP stdio JSON-RPC (`initialize`,
`tools/list`, `tools/call`); write every request, response and latency to
`reports/<run>/raw/<server>.seed<N>.<family>.jsonl`; compute the `protocol.md` §3 metrics from those
raw responses; aggregate over repetitions with a 1000-resample percentile bootstrap over the
repetition axis; write `report.json` and `summary.md`; then validate the report by spawning
`refslint report` (`tools/refslint`, obtained through `tools/refslint.sh`) on the file just written,
and exit 1 if it is rejected. The runner also exits 2 with `RUN SUSPECT` when more than 5 % of the
calls failed with harness-side errors, so a broken harness cannot masquerade as a bad subject.

The two Python files this command replaces are frozen in `evidence/pre-port-runner/`, and the port is
checked rather than asserted — the harness ships the check itself:

```bash
"$RL" parity streams --reference reports/sint-liquid-mcp-2026-10-16 --server sint-liquid-mcp
      # the facts this binary generates, and the order it writes them in,
      # against the requests the Python harness actually recorded: 237 write calls, all identical
"$RL" parity run-vs-report --reference reports/sint-liquid-mcp-2026-10-16 \
                           --run reports/parity-rust/sint-liquid-mcp-2026-10-16
      # every field of the two reports: 951 fields compared exactly and identical,
      # 24 timing fields equal by order of magnitude, 31 paths equal after normalising the run directory
```

`reports/parity-rust/<run>/` holds the ported harness's own runs of the same two subjects, on the
same seeds, against the same commits — the artefacts the comparison reads.
`evidence/protorun-port-2026-10-16.txt` is the full transcript (clean build, both checks for both
subjects, the `summary.md` and `report.json` diffs).

Dependencies: both subjects import `mcp.server.fastmcp`, i.e. the MCP Python SDK 1.x. On this host
`pip install` failed during the run (`Errno 28`, disk full), so the SDK was unpacked into a site
directory and passed with `--server-pythonpath`; the interpreter actually used is recorded in
each report under `disclosure.environment`. Nothing was installed into any shared environment.

The combination re-checked on this machine in 0.4.0 (the SDK there is 1.28.1; the 0.2.0 runs used
1.9.4 from another site directory, which no longer exists):

```bash
python code/run_protocol.py --server sint-memory-mcp \
    --server-python "<hermes-venv>/Scripts/python.exe" \
    --server-pythonpath "{{HOME}}/AppData/Local/Python/pythoncore-3.14-64/Lib/site-packages" \
    --seeds 1
```

The port's parity runs (2026-10-16) deliberately used the SDK the committed reports were produced
with, 1.9.4, so that the two reports are comparable down to `serverInfo.version` in the raw logs.
That site directory had been deleted with the run that created it, so it was re-created outside any
shared environment and inside the ignored build directory:

```bash
python -m pip install --target tools/protorun/target/mcp-1.9.4-site "mcp==1.9.4" "pydantic==2.13.4"
      # pydantic is pinned: mcp 1.9.4 imports pydantic._internal._typing_extra.eval_type_backport,
      # which newer pydantic releases no longer export

. tools/protorun.sh && RL="$(protorun_binary)"
"$RL" run --server sint-liquid-mcp \
    --server-root "{{HOME}}/AppData/Local/hermes/profiles/libraressa/workspace/refs" \
    --server-python "<hermes-venv>/Scripts/python.exe" \
    --server-pythonpath "{{HOME}}/AppData/Local/hermes/profiles/libraressa/workspace/papers/agent-memory-mcp/tools/protorun/target/mcp-1.9.4-site" \
    --seeds 1,2,3 --out reports/parity-rust --run-name sint-liquid-mcp-2026-10-16
      # the same for sint-memory-mcp -> reports/parity-rust/sint-memory-mcp-2026-10-16/
```

The two runs that exist in this folder:

| Run | Subject @ commit | Contents | Validator |
|---|---|---|---|
| `reports/sint-liquid-mcp-2026-10-16/` | `sint-liquid-mcp` @ `bfffce2cdec3` | `report.json`, `summary.md`, 33 raw log files, the per-repetition store | `VALID` |
| `reports/sint-memory-mcp-2026-10-16/` | `sint-memory-mcp` (`sint` monorepo) @ `7554f7eef853` | `report.json`, `summary.md`, 33 raw log files, the per-repetition store | `VALID` |

Both are **per-operation profiles, not a ranking**: the two `report.json` files are read side by
side, each carrying its own disclosure block, failure counts and `known_deviations`. Paper §6 is
written from them, and every table and every number in it is generated by `refslint tables`
(§2c) from those two files — including the numbers that look like prose, which are `\val{...}`
lookups. What §6 states as an observation, it states with its limits: two subjects from one
author's reference set, three repetitions, one machine, no model in the loop. The `null` values are
informative rather than missing: each marks a metric the protocol defines but the subject's tool
surface cannot produce, and says so in its note.

Every number in a report is computed by the harness from the raw logs beside it, and the store
directory the run wrote (`reports/<run>/data/seed<N>/`) is kept for the same reason. What is
reproducible **with one command** is everything downstream of the reports — validation, the tables,
the ledger and every number in the paper; re-running the protocol needs the host setup above, and
there is no offline command that rebuilds a `report.json` from its raw logs (the metric layer is
inside the harness: `tools/protorun run`, standard library only, and metric computation needs a live
subject — the `parity` and `streams` subcommands are the parts that work offline).

## 5. What is deliberately not claimed

From `claims.md` §3 (status **D — NOT CONFIRMED**):

1. No claim of superiority. Paper §6 is a pair of *profiles* of two servers from this author's
   reference set: the third-party systems of the taxonomy table were not run, so nothing is claimed
   about them (`claims.md` D1).
2. No claim that the reference implementation is better than the systems it cites.
3. No claim that human-like forgetting improves agent memory (evidence points the other way).
4. No claim that the taxonomy is complete.
5. No claim that the protocol transfers to industrial domains without domain-specific gold data.
6. No inter-rater agreement value yet for rubric-graded metrics.
7. The novelty claim ("no prior operation-level protocol for memory servers") is a *search*
   result and is published only with its queries, dates and screened set
   (`evidence/search-log.md`), and only in the "to the best of our search" form.
8. The measured pair does not support a ranking, and the paper does not publish one: both subjects
   come from one author's repositories, two families are `not_implemented` in both, three seeds is
   the protocol minimum, and no model is in the loop (token and USD metrics are `null` by
   construction).
9. H1 and H2 of §6 were stated before the run and are reported as an outcome, not as a test: H1's
   antecedent is absent in both subjects (no lifecycle field is exposed), H2 holds for one of the
   two (`claims.md` C6).

## 6. AI use

Artificial-intelligence assistance was used for literature retrieval and metadata resolution,
for drafting and editing the Russian and English text, and for writing the tooling in `code/`.
The author (Konstantin Parashchuk) is the sole author and takes responsibility for every claim,
and the AI use is disclosed in the paper's Acknowledgements. Automatically collected numbers
(token counts, tool inventories, API responses) are reproducible from the committed raw
responses; nothing was produced by asking a model to recall a fact.

Authorship is not shared with any tool, agent or model. Co-authorship by people is the author's
decision alone.

## 7. Licences

- **Text** (this file, `*.md`, `tex/`): CC BY 4.0 — https://creativecommons.org/licenses/by/4.0/
- **Code** (`code/`, `tools/refslint/`, `*.py`, `*.rs`, `build.sh`): Apache-2.0 (consistent with
  the upstream SINT repositories).
- Zenodo metadata records the text licence (`cc-by-4.0`); the code repositories carry their own
  licence files.

## 8. Citing this deliverable

Use `CITATION.cff` (GitHub renders it as "Cite this repository"). **Version 1.0 (16 October 2026)
is prepared for deposit**; the DOI is issued on deposit and applies to that version only, later
revisions go in as new versions with a `CHANGELOG.md` entry. Until the DOI exists, cite the version
number with the repository URL and the commit SHA. Note for the author: `CITATION.cff` and
`.zenodo.json` currently point at `https://github.com/kostyk348/papers`, which does not resolve yet
(checked with `gh repo view` on 2026-10-16) — create that repository, or change both URLs, before
the deposit.

## 9. Next steps (other cards / profiles)

1. **Harness** (owner: `fabricessa`) — **done, and now ported to Rust**: the harness runs
   `protocol.md` §4 against a server over MCP stdio and emits `reports/<run>/report.json`, which it
   validates by spawning the package's validator `refslint report` before exiting (two runs, both
   `VALID`; see §4b). The harness binary is `tools/protorun` (standard library only), its parity
   against the Python harness it replaces is a check the binary performs itself (§4b), and the
   Python files are frozen in `evidence/pre-port-runner/`. Still open inside the harness: subjects
   blocked by the `fastmcp` dependency (`sint-epistemic-mcp`, `sint-dream-mcp`),
   `sint-prospective-mcp` as the real O5_INTEND subject, and the deletion of the now-unreferenced
   `code/run_protocol.py` / `code/streams.py` (§2c says why they are still here).
2. **Validation unification** (owner: `fabricessa`) — **done in this version**: the harness validates
   its reports with `refslint report`, the same validator `verify_all.sh` and the negative tests use;
   `code/validate_report.py` is deleted and the package no longer ships a Python validator, and with
   the 1.1 harness port it no longer ships a Python harness either — only the frozen pre-port copy in
   `evidence/pre-port-runner/`, which nothing executes.
3. **Comparison section** (owner: `scriptessa`) — **done in 1.0**: paper §6 is written from the two
   validated reports, every table and every inline number is generated by `refslint tables` from
   them, `refslint tables --check` (also inside `build.sh` and `verify_all.sh`) fails on drift and on
   a number with no report field, and the section carries its measurement limits and the outcome of
   the pre-registered hypotheses H1/H2. Open after it: a third and fourth subject outside this
   author's repository set, a model in the loop (so the token/cost metrics stop being `null`), and
   more repetitions than the protocol minimum.
4. **Review** (owner: `libraressa`): literature gaps, novelty claim, positioning.
5. **Publication** (author's decision): arXiv first (for the DOI), then an LLM-agent workshop.
   Before the deposit, two identifiers in the metadata need the author's decision: the repository URL
   of the package (see §8) and the Zenodo `subjects` field, which is deliberately empty because the
   controlled term list could not be verified from this host.
