# Changelog

One DOI is one version. After a deposit, corrections go into a new version with an entry here —
never into an already published record.

## Repository release — 2026-10-16

- Package published at https://github.com/kostyk348/papers, tag `agent-memory-mcp-v1.0`
  (manuscript version 1.0; the 1.1 entry below is unreleased work already present in the tree).
- Local absolute paths in run logs, reports and evidence were replaced by `{{HOME}}` and
  `<hermes-venv>` before publication. Values, raw protocol exchanges and code are unchanged;
  hashes recorded in the port/parity evidence were computed before that substitution.

## 1.1 — 2026-10-16 (unreleased; the protocol harness is now one Rust binary)

The port promised since 0.2.0 in `TOOLING.md`: the harness that produces `reports/` was the last
Python in the pipeline, and it is now `tools/protorun` (Rust, standard library only, **no crates**).
Full record: `evidence/protorun-port-2026-10-16.txt`.

Added
- `tools/protorun/` — one Rust binary, subcommands `streams` (generate T1–T10 from a seed, with a
  CPython-compatible MT19937 so the streams are the Python ones), `servers` (read `code/servers.json`),
  `run` (spawn the subject over MCP stdio JSON-RPC, log every request/response, compute the
  `protocol.md` §3 metrics, write `report.json` + `summary.md`, self-validate with `refslint report`,
  exit 0/1/2) and `parity` (`streams` and `run-vs-report`, the two port checks). ~7.2k lines replacing
  ~2.3k lines of Python.
- `tools/protorun.sh` — finds the binary or builds it with cargo, exactly like `tools/refslint.sh`.
- `evidence/pre-port-runner/` — the Python harness (`run_protocol.py`, `streams.py`) frozen
  byte-identically with `SHA256SUMS.txt` (`fb051d36281f1094…`, `d7975f0266587f3c…`), plus a README
  stating what it is and is not.
- `evidence/protorun-port-2026-10-16.txt` — build, both checks for both subjects, the `summary.md`
  and `report.json` diffs, and the commands that reproduce all of it.
- `reports/parity-rust/<run>/` — the ported harness's own runs of the same two subjects, on seeds
  1,2,3, against the same commits, with the same SDK (mcp 1.9.4) the reference reports were produced
  with; these are the artefacts the report comparison reads.

Changed
- `README.md` §1 (table rows for `tools/protorun`, `evidence/pre-port-runner/`, the parity runs),
  §2c, §4b (the run commands, the parity commands, the SDK note), §9; `TOOLING.md` (the port table,
  the target block — `refslint run` was realised as `protorun run`, a second binary, because the
  harness is a process supervisor and not a document tool) and this changelog.
- `.gitignore` — ignores `tools/protorun/target/`, which is also where the MCP SDK used for the
  parity runs is unpacked (`pip install --target`, never a shared environment).
- `verify_all.sh` — runs the harness-port parity check (`protorun parity run-vs-report` for every
  pair of `<run>` / `reports/parity-rust/<run>`) when those runs are present, so the claim cannot
  silently rot.

Verified (not declared) — every command below is in `evidence/protorun-port-2026-10-16.txt`
- Cold build: `CARGO_TARGET_DIR=<empty dir> cargo build --release` → `Finished`, **no warnings**, no
  crates fetched, no network, ~30 s.
- `protorun parity streams` for both subjects: **237 write calls** (3 seeds × 10 families) whose
  `arguments` are identical to the requests the Python harness recorded, including the drawn
  confidences and balances. The generator is the same generator, not a re-implementation of it.
- `protorun run` on `sint-liquid-mcp` and `sint-memory-mcp` (seeds 1,2,3, SDK mcp 1.9.4): both
  reports written and accepted by `refslint report` (10 families, 52 metric entries each; 227 and 220
  calls per repetition — the same counts as the Python runs, which is what makes the raw logs
  comparable file by file).
- `protorun parity run-vs-report` for both subjects: **951 fields compared exactly and identical**
  (every metric `value`, every 1000-resample bootstrap CI, every `n`, `computer` and full `notes`
  string, the failure codes/counts/examples, the `operations` blocks and the disclosure blocks),
  **24 timing fields equal by order of magnitude**, 31 paths equal after normalising the run
  directory, 4 fields exempt as machine-specific (the harness hash, the hardware class, the
  environment string, and the one `known_deviations` item that describes the harness — the other
  nine are carried over verbatim, which the tool asserts). By way of a plain diff: the `summary.md`
  files differ in **23 lines out of 167**, and the `report.json` files only in the four latency
  metrics and the five metadata lines above.
- The numbers in the card that commissioned the port (T1 `write_precision` 0.75,
  `idempotency_violations` 4, T2 `mrr` 0.75) do **not** match the committed reference report of
  `sint-liquid-mcp`, which carries 1.0, 0 and 0.5 — the committed `report.json` is the reference the
  port was checked against, and the parity tool says so field by field.

Known gaps at this version (stated, not hidden)
- `code/run_protocol.py` and `code/streams.py` are **kept**: `protocol.md`, `README.md` §4b and the
  two committed `summary.md` files name them by path (the summaries quote the Python command line in
  their Reproduce block). Removing them is a follow-up that also rewrites those references. They are
  frozen in `evidence/pre-port-runner/` and nothing executes them — they are not wired into
  `build.sh`, `verify_all.sh` or any card.
- The metric layer is not available offline: `protorun run` needs a live subject server, and no
  subcommand recomputes a `report.json` from a `raw/` log. (`streams` and `parity` are offline.)
- Timing values differ between the harnesses (the Rust harness's own I/O inside the measured window
  differs from Python's); they are compared by order of magnitude, as the protocol's timing metrics
  allow.
- The two parity runs are evidence of this port, not a third and fourth subject: same two subjects,
  same seeds, same commits.

## 1.0 — 2026-10-16 (release version: measurements in the paper)

The 0.x entries below are the build-up of the package; this is the first version that is
submitted as a paper with results rather than as a draft of a protocol.

Added
- Section 6 "Worked comparison: two memory servers under one protocol" — per-family and
  per-operation profiles of `sint-liquid-mcp` and `sint-memory-mcp`, written from the two validated
  reports, with the pre-registered hypotheses H1/H2 answered as far as a two-subject pair allows
  and the failures, `null` values and non-measures stated beside the numbers.
- Section "Data and code availability" — the four commands that reproduce the paper (build,
  report validation, table regeneration check, full integrity set) and the identifiers of the runs.
- `refslint tables` (Rust, `tools/refslint/src/tables.rs`) — the subcommand that generates
  `tex/tables/run-identity.tex`, `tex/tables/family-T1…T10.tex`, `tex/tables/operations.tex`,
  `tex/numbers.tex` (the `\val`/`\valci`/`\valn`/`\valt`/`\valtl` lookups the text uses) and the
  ledger `data/run-numbers.tsv` (every value with its JSON path) from `reports/*/report.json`.
  `--check` fails on a stale generated file, on a missing `\input`, and on a number the text uses
  that no report contains.
- Claim ledger entries A8 (one protocol, two validated per-operation profiles), A9 (no
  hand-typed measurement number: the tables and the inline values are generated), A10 (raw logs and
  per-repetition store committed beside every run) and C6 (the pre-registered hypotheses with their
  outcome).

Changed
- `tex/main.tex` is a paper now, not a skeleton: `\draftmodetrue`, `\todo{}`, `\claimc{}` and the
  "Draft … first deliverable" date line are gone; the date line reads **Version 1.0, 16 October
  2026**; the abstract reports what was measured (two servers, ten task families, three
  repetitions) instead of stating that nothing was.
- Contributions are C1–C5, with C4 the measured profiles and C5 the integrity artefacts; the
  "what this paper does not claim" paragraph no longer says that no run exists.
- Section 5 states the gaps after measurement: the liquid store is measured, the epistemic,
  dreaming and prospective servers could not be launched on this host and remain unmeasured.
- Limitations carries the measurement limits explicitly: two subjects from one reference set, three
  repetitions, one machine, no model in the loop (token/USD metrics `null` by construction), the
  Python harness (identified by content hash) still to be ported, and the T3/T4/T6 coverage gaps.
- `build.sh` and `verify_all.sh` regenerate and check the generated tables, so a table that drifts
  from the reports fails the build.
- `CITATION.cff` and `.zenodo.json`: version 1.0, `type: article`, abstract and Zenodo description
  rewritten for a measured paper; keywords refreshed; the controlled `subjects` field is
  deliberately absent because the term list could not be verified against Zenodo's vocabulary API
  from this host (an invented subject term would be worse than none).
- `claims.md` C3 was promoted to A8 in its first half only: "including one the protocol's authors
  did not write" is **not** satisfied — both measured servers come from this author's repositories.

Verification of this version
- `refslint report --schema code/protocol_schema.json reports/*/report.json` → `VALID` ×2.
- `refslint tables --root . --check` → `tables up to date: 2 subjects, 10 families, 104 metric rows
  over families, 86 number lookups in tex/main.tex all resolve`.
- `bash build.sh` → `tex/main.pdf` (22 pages, 0 overfull boxes, 0 undefined references).
- `bash verify_all.sh` → `ALL CHECKS PASSED` (transcript in `evidence/checks-2026-10-16-1.0.txt`);
  the record of the table/number work is `evidence/tables-and-numbers-2026-10-16.txt`.

Still not confirmed at this version
- D1–D7 below are unchanged. In particular: no head-to-head run against MemGPT/Zep/Mem0 (D1); no
  κ for rubric metrics (D5); the domain-transfer claim is untested (D4); the novelty claim remains
  a dated search result (D7).

## 0.1.0 — 2026-10-16 (first deliverable, not yet deposited)

Working draft: structure, taxonomy, evaluation protocol, verified reference base. No measurements.

Added
- `outline.md` — paper structure, contributions C1–C4, non-claims, writing order.
- `taxonomy.md` — six operations (O1 WRITE, O2 READ, O3 CONSOLIDATE, O4 REVISE, O5 INTEND,
  O6 ASSERT) with pre/postconditions, variants and failure modes; PROVENANCE and LIFECYCLE record
  axes; mapping table for seven systems; explicit differences from the three closest prior works
  plus two adjacent ones; reference-implementation gap map with commit SHAs.
- `protocol.md` — ten task families, metric table naming the computer of every metric, run
  procedure (server-selection rule, pinning, repetitions, budgets, masking, restart probes,
  adversarial ordering), report format with mandatory disclosure block, failure taxonomy, threats
  to validity.
- `related-work.md` — thematic review with every claim attributed to a verified source.
- `claims.md` — claim ledger with statuses A (artefact) / B (literature) / C (position) /
  D (not confirmed); seven explicit NOT-CONFIRMED entries.
- `reference-verification.md` (generated) — 135 sources, 0 unresolved, each with identifier,
  title, the claim it supports and the API/probe that verified it.
- `code/refs.py` — curated source list, the single source of truth for citations.
- `code/build_refs.py` — refuses to build if any curated source is unresolved; emits
  `tex/references.bib`, `reference-verification.md`, `data/refs-verified.csv`.
- `code/check_cites.py` — fails on unknown citation keys or uncited curated sources.
- `code/collect_refmeta.py`, `code/arxiv_search.py`, `code/verify_ids.py`, `code/verify_dois.py`,
  `code/probe_urls.py` — the collection pipeline; raw responses kept in `evidence/raw/`.
- `code/repo_facts.py` — read-only commit-pinned facts about the reference implementation.
- `code/protocol_schema.json`, `code/validate_report.py`, `code/make_bad_reports.py` — report
  schema plus validator verified by five negative tests.
- `tex/main.tex` — English draft that compiles; `build.sh` builds the PDF with one command
  (tectonic, auto-downloaded if absent).
- `CITATION.cff`, `.zenodo.json`, `README.md`, `evidence/search-log.md`.

Known gaps at this version (stated, not hidden)
- No protocol run has been executed; `data/example-report.json` is a schema example with all
  metric values `null` and must never be quoted as a result.
- No inter-rater agreement (κ) for rubric-graded metrics.
- The novelty statement is a search result, dated and bounded by `evidence/search-log.md`.
- Domain transfer to industrial settings is untested.
- **Tooling language:** the pipeline in `code/` is Python 3 (stdlib only). A user rule posted
  after this version's tooling was written requires new utilities in Rust (one binary with
  subcommands); the port is specified in `TOOLING.md` with the acceptance criterion that the Rust
  binary must reproduce the generated files byte-identically before the Python files are removed.

## 0.2.0 — 2026-10-16 (unreleased; harness + first two validated reports)

Added
- `code/run_protocol.py` — the protocol harness. Generates the T1–T10 streams from a seed, launches
  the server under test as a child process with a fresh home/data directory per repetition, drives it
  over MCP stdio JSON-RPC (`initialize`, `tools/list`, `tools/call`), logs every request, response and
  latency to `reports/<run>/raw/*.jsonl`, computes the `protocol.md` §3 metrics from those raw
  responses, aggregates with a 1000-resample percentile bootstrap over the repetition axis, writes
  `report.json` + `summary.md`, validates its own output with `code/validate_report.py` and exits 1 on
  an invalid report, 2 on a harness-suspect run.
- `code/streams.py` — deterministic stream generator for T1–T10 with programmatic gold labels
  (gold comes from the generator, never from a model); identical output for identical (seed, sizes).
- `code/servers.json` — candidate-server registry: repo, commit SHA, launch spec, adapter, storage
  layout, and for every candidate the ground for including or excluding it.
- `reports/sint-liquid-mcp-2026-10-16/`, `reports/sint-memory-mcp-2026-10-16/` — the first two runs:
  `report.json` (validated), `summary.md`, raw response logs (33 files per run) and the per-repetition
  store directory next to them. Subjects: `sint-liquid-mcp` @ `bfffce2cdec3`, `sint-memory-mcp`
  (`sint` monorepo) @ `7554f7eef853`; seeds 1,2,3; MCP revision `2025-03-26`; transport stdio; no model
  in the loop.
- `README.md` §4b — the reproduction command and what the run does, in order.

Changed
- `verify_all.sh` validates every report in `reports/` with `refslint report` as well, so a run that
  stops validating fails the package-wide check. Both reports are accepted by that validator **and**
  by `code/validate_report.py` with the same verdict (10 families, 52 metric entries each).
- `.gitignore` keeps the run evidence (`report.json`, `summary.md`, `raw/`) and still excludes the
  machine-local store directories.
- `TOOLING.md` records the two new Python files as to-port, with the `refslint run` subcommand they
  would become.

Known gaps at this version (stated, not hidden)
- Only two of the six candidate servers could be executed on this host: `sint-epistemic-mcp` and
  `sint-dream-mcp` import the standalone `fastmcp` distribution, which could not be installed during
  the run (`Errno 28`, disk full). Both exclusions are itemised in the reports' `candidate_servers`.
- T3 (as-of-date reads), T4 (principal-scoped reads) and T6 (consolidation) are `not_implemented` for
  both subjects, and T7 had to be operationalised as store-mediated cue retrieval; each deviation is
  itemised in the reports' `known_deviations` with the metric-level notes.
- No rubric metric carries a value: nothing in either run was graded, so no κ is reported.
- Token and USD metrics are `null` by construction (store-only runs, no price table applies).
- **Tooling language:** the harness is Python for the same reason the rest of `code/` is
  (interoperability with the Python servers and the Python validator, and a card that names
  `code/run_protocol.py`); the Rust port is specified in `TOOLING.md`.

## 0.3.0 — 2026-10-16 (unreleased; the pipeline is now one Rust binary)

The user rule posted mid-run on card t_6b4b6255 — *new utilities in Rust, one binary with
subcommands; Python only where nothing else can do the job* — is carried out. Port specified in
`TOOLING.md`, verified in `evidence/refslint-port-2026-10-16.txt`, and re-checked by
`verify_all.sh` on every run.

Added
- `tools/refslint/` — one Rust binary (standard library only, **no crates**, ~850 KB, ~20 s build):
  `import-spec`, `bib`, `cites`, `jsoncheck`, `report`, `selftest`, `facts`, `merge`, `abs`, `fetch`,
  `ids`, `doi`, `probe`, `query`.
- `code/refs.toml` — the curated source list as data (135 entries), converted once from the Python
  literal by `refslint import-spec`; it is now the single source of truth edited by hand.
- `tools/refslint.sh` — finds the binary or builds it with cargo; sourced by `build.sh` and
  `verify_all.sh`, so a fresh clone needs only cargo.
- `evidence/pre-port/` — the artefacts the Python pipeline produced, frozen with `SHA256SUMS.txt`
  before the port.
- `evidence/refslint-port-2026-10-16.txt` — the port record: build, spec conversion, byte-identity,
  citation counts, validator parity, negative self-test, repo facts, snapshot merge, live network
  calls and the default-banner diff.

Changed
- `build.sh`, `verify_all.sh` — call refslint; neither script mentions Python any more. `build.sh`
  unpacks tectonic with `unzip` (PowerShell fallback) instead of `unzip_tectonic.py`.
- `verify_all.sh` — new port-parity step: `refslint bib --legacy-banner` must reproduce the three
  generated files byte for byte against `evidence/pre-port/`.
- `tex/references.bib`, `reference-verification.md` — the generator line now names `refslint bib`;
  the Python-era text remains reachable with `--legacy-banner`, which is what the parity step uses.
  The only difference from the pre-port files is those banner lines (one in each file).
- `README.md` (§2/§2b/§2c/§3/§9), `TOOLING.md`, `claims.md` (A1–A5 commands, new **A7**),
  `protocol.md`, `outline.md` — tooling described as it now is, with A7 stating the port as a
  verified artefact claim and naming what would refute it.
- `.gitignore` — ignores `tools/refslint/target/`, the tectonic download and `.port-parity-check/`
  instead of the whole `tools/` tree (the Rust source is committed).

Removed
- `code/arxiv_search.py`, `build_refs.py`, `check_cites.py`, `collect_refmeta.py`, `count_cites.py`,
  `crossref_query.py`, `dump_abs.py`, `make_bad_reports.py`, `probe_urls.py`, `repo_facts.py`,
  `verify_dois.py`, `verify_ids.py`, `unzip_tectonic.py`, `code/__pycache__/`; `code/refs.py` moved
  to `code/legacy/refs.py` (provenance only, nothing reads it).
- **Kept on purpose:** `code/validate_report.py`, because the 0.2.0 harness imports it by name.
  `refslint report` is the same validator; switching the harness over is the follow-up that lets the
  last Python file go (README §9).

Verified (not declared) — all of it reproducible by the commands in `verify_all.sh`
- `refslint bib --legacy-banner` vs `evidence/pre-port/`: 3/3 `cmp`-identical (sha256 `da209f93…`,
  `4ad7be69…`, `2262577e…`).
- `refslint cites --check` → 135 keys used, 0 unknown, 0 uncited; `--count` identical to the Python
  output for all 7 files.
- `refslint report`: the schema example is `VALID`; the five broken reports are rejected with
  identical messages (diff against the Python output empty).
- `refslint selftest`: 5/5 rejected, and the regenerated `data/bad-reports/*.json` keep their exact
  sha256.
- `refslint facts` vs `data/repo-facts.json`: identical apart from the collection timestamp (commit
  SHAs, file counts, line counts, all six tool inventories).
- `refslint merge` vs `data/refmeta.json`: identical apart from the timestamp (290 arXiv, 25 DOIs,
  25 URLs).
- Live network runs: `ids` 2/2, `doi` 2/2, `probe` (HTTP 200, title captured), `fetch` (2 queries →
  6 records), `query`, `abs`.
- `bash verify_all.sh` → `ALL CHECKS PASSED`.

Known gaps at this version (stated, not hidden)
- `code/validate_report.py` remains, and the harness itself (`run_protocol.py`, `streams.py`) is
  still Python; `refslint run` needs an MCP stdio JSON-RPC client in Rust (see `TOOLING.md`).
- `refslint abs` line wrapping was not compared line-by-line with the deleted `dump_abs.py`; no
  metric depends on it.
- Generated JSON keeps CRLF line endings for byte-identity with the Python originals; a later
  version may re-baseline to LF as a visible change.
- `data/refmeta.json` and `data/repo-facts.json` are untouched: their `collector`/`collected` fields
  still name the pre-port collection runs (honest provenance). Re-running now reproduces both with
  `collector: refslint merge` / the ported facts collector.
- The snapshot's own dates did not move: no source was re-queried in this version, so every
  verification date in `reference-verification.md` is still 2026-10-16.

## 0.4.0 — 2026-10-16 (unreleased; the harness validates with the package's Rust validator)

The follow-up promised in 0.3.0: the last Python file of the old pipeline is gone. The harness
(`code/run_protocol.py`) no longer imports a validator — it spawns `refslint report` on the report it
has just written, i.e. the same validator `verify_all.sh`, the five negative tests and
`refslint selftest` use, so the judge of a report and the producer of a report can no longer drift
apart. Full transcript: `evidence/refslint-report-2026-10-16.txt`.

Changed
- `code/run_protocol.py` — validation is `refslint report` (the Rust binary, obtained through
  `tools/refslint.sh`, which builds it with cargo when it is missing; `cargo build --release` is run
  directly only when no shell is reachable from Python). Documented exit codes: 0 = report written and
  valid, 1 = report written but rejected by the validator, 2 = run suspect (more than 5 % harness-side
  call failures) or the validator could not be obtained at all — an unvalidated report is not citable
  either. The run-summary template and the `known_deviations` disclosure text no longer name a Python
  validator.
- `README.md` (§1 table, §2c, §4b, §9), `TOOLING.md` (port table, the "one Python file left" note, a
  new 0.4.0 outcome table), `protocol.md` (§1, §5), `claims.md` (§2 rule 4), `outline.md`
  (appendix C) — the validator is described as `refslint report`, and every statement that the
  harness imports `code/validate_report.py` is gone.

Removed
- `code/validate_report.py` and its `__pycache__` entry. `code/legacy/*.py` stays (provenance, not
  part of any pipeline).

Verified (not declared)
- both committed runs still validate: `refslint report --schema code/protocol_schema.json
  reports/*/report.json` → `VALID` ×2, exit 0;
- a fresh harness run of `sint-memory-mcp` (seed 1) → `VALID`, `validated with:
  tools/refslint/target/release/refslint.exe report`, exit 0; with the binary deleted beforehand the
  same run rebuilds it through `tools/refslint.sh` (offline, stdlib only) and still ends `VALID`;
- invalid report → exit 1 carrying the validator's own message (`.subject.commit_sha: 'v1.2.3' does
  not match pattern ^(none|[0-9a-f]{7,40})$`); suspect run → exit 2 (`RUN SUSPECT: 214 of 215 calls
  failed with harness-side errors`); validator unobtainable → exit 2 (`RUN UNCERTIFIED`);
- `refslint selftest` → 5 of 5 broken reports rejected, and the five regenerated
  `data/bad-reports/*.json` are byte-identical (sha256 before = after);
- `bash build.sh` → PDF rebuilt; `bash verify_all.sh` → `ALL CHECKS PASSED`, exit 0
  (`evidence/checks-2026-10-16-0.4.0.txt`; the 0.3.0 capture deliberately stays in
  `evidence/checks-2026-10-16.txt`, which the port record references).

Known gaps at this version (stated, not hidden)
- This version removes the Python *validator*, not the harness: `code/run_protocol.py` and
  `code/streams.py` stay Python (stdlib only) and are unchanged apart from the validation call and
  the disclosure text they write; their port is a separate card (`TOOLING.md`).
- `data/example-report.json` and `data/bad-reports/*.json` still carry the sentence "this report
  exists to exercise `code/validate_report.py`" in their disclosure text. `refslint selftest`
  regenerates those five files byte for byte and this change's acceptance criterion pins their
  sha256, so the wording is left for a follow-up instead of being re-baselined silently.
- The two committed reports keep the disclosure text of the runs that actually produced them
  (0.2.0/0.3.0), including the sentence naming `code/validate_report.py`; this change does not re-run
  the 3-repetition protocol, so a fresh run differs from them in that disclosure sentence alone.

Next version is expected to add: paper §6 written from the two validated reports; a re-run of the
literature search with an updated date; the harness port (`refslint run`); and a Zenodo deposit of
the English PDF.

## 1.0 — 2026-10-16 (manuscript version 1.0, published)
