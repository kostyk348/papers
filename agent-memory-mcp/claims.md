# Claims ledger

Every assertion this paper intends to make, with what supports it, what could refute it, and
its current epistemic status. A claim may be published only if the "supported by" column points
to a machine-checkable artefact (a file in this repository, produced by a script that is also
here) or to a verified source (`reference-verification.md`).

Status vocabulary:

- **A — verified artefact**: reproducible by running a command in this repository.
- **B — verified literature**: an external source resolved by API/probe, cited for exactly this
  claim (the `supports` column in `reference-verification.md`).
- **C — position/argument**: a reasoned stance, not a measurement. Must be stated as a claim in
  the text, not as a result.
- **D — NOT CONFIRMED**: not supported; listed in §3 and either framed as an open question or
  removed before publication.

---

## 1. Claims about the artefacts (A)

| # | Claim | Status | Supported by (command / file) | What would refute it |
|---|---|---|---|---|
| A1 | The reference base of this paper contains 135 curated sources, none of which was typed from memory | A | `refslint bib --root .` → `resolved 135 curated sources (0 unresolved)`; table in `reference-verification.md`; snapshot `data/refmeta.json` | a source that does not resolve, or a citation in the text whose key is not in `code/refs.toml` (`refslint cites --check` fails) |
| A2 | Every citation key used in the deliverables exists in the curated list, and every curated source is cited somewhere | A | `refslint cites --check --root .` → `citation keys used: 135 unique`, `curated keys never cited: 0` | a stray or unused key reported by the same command |
| A3 | The protocol has a machine-readable schema and a validator that rejects structurally invalid reports | A | `refslint report --schema code/protocol_schema.json data/example-report.json` → `VALID`; the same command on `data/bad-reports/*.json` → 5/5 rejected with a stated reason | a report that passes validation while missing a mandatory disclosure field |
| A4 | The validator catches the specific integrity rules the protocol promises (CI required with a value, κ required for graded rubric metrics, operations may not claim support without metrics, adversarial family may not be dropped silently, a commit SHA is required in place of a bare version) | A | `refslint selftest --root .` regenerates `data/bad-reports/*.json` and reports 5 of 5 rejected, 0 accepted | a tenth broken report that passes |
| A5 | The reference implementation consists of six public repositories whose operation coverage is stated per commit | A | `refslint facts --refs-dir <refs dir> --out data/repo-facts.json`; `taxonomy.md` §6 (commit SHAs, parsed tool counts) | a commit SHA that does not exist in the repository, or a tool count that does not reproduce |
| A5b | The commit SHAs in `data/repo-facts.json` are corroborated by an independent collection path | A | `data/raw/upstream/manifest.json` (20 repositories, head commits via the GitHub API) agrees with the `git clone` SHAs for every shared repository | the two sources disagreeing on any SHA |
| A6 | The paper produces an English PDF from the same source files by one command | A | `bash build.sh` (tectonic 0.17.0) → `tex/main.pdf` | the command failing on a clean checkout, or the PDF missing sections |
| A7 | The pipeline utilities are a single Rust binary (`tools/refslint`, standard library only, no crates) and the port changed no executed behaviour | A | `bash verify_all.sh` (port-parity step) and `evidence/refslint-port-2026-10-16.txt`: `refslint bib --legacy-banner` reproduces `tex/references.bib`, `reference-verification.md`, `data/refs-verified.csv` byte for byte against `evidence/pre-port/`; `refslint cites` (135 keys, 0 unknown, 0 uncited), `refslint report` (example valid, 5/5 rejected with the same messages), `refslint selftest` (5/5, regenerated files unchanged), `refslint facts` and `refslint merge` identical to the Python outputs apart from their own timestamps | a generated file that differs from `evidence/pre-port/`, or a broken report that the Rust validator accepts |
| A8 | One protocol produces machine-validated per-operation profiles of two memory servers that differ in storage design, and the reports pass the same schema validator the harness runs | **A** | `refslint report --schema code/protocol_schema.json reports/*/report.json` → `VALID` ×2; `bash verify_all.sh` ("protocol runs: every report in reports/ validates"); the two runs are `reports/sint-liquid-mcp-2026-10-16/` (`bfffce2cdec3`) and `reports/sint-memory-mcp-2026-10-16/` (`7554f7eef853`) | a report that fails validation, or a subject whose reported tool surface cannot exercise the family it is scored on |
| A9 | No measurement number in the paper is typed by hand: every table and every inline value of Section 6 is generated from the two `report.json` files, and each value carries its JSON path | **A** | `refslint tables --root . --check` → `tables up to date: 2 subjects, 10 families, 104 metric rows over families, 86 number lookups in tex/main.tex all resolve`, exit 0; ledger `data/run-numbers.tsv` (JSON path per value); `bash build.sh` regenerates the tables before compiling | a table edited by hand (the check reports the file as `STALE`), a `\val{...}` lookup with no field in any report (`UNKNOWN NUMBER`), or a value in the text that differs from the ledger |
| A10 | The two runs' raw evidence is committed beside the numbers: every request, response and latency, 33 JSONL files per run, plus the per-repetition store the run wrote | **A** | `reports/<run>/raw/*.jsonl` (66 files) and `reports/<run>/data/seed<seed>/`; counts are in the generated Table 3 (`raw responses … (33 JSONL files)`) and in `refslint tables` | a missing raw log, or a store directory without the records the report counts |

## 2. Claims about the state of the field (B)

| # | Claim | Status | Supported by | What would refute it |
|---|---|---|---|---|
| B1 | Agent memory is now a persistent store with storage, retrieval, update, consolidation and lifecycle governance — not merely retrieval augmentation | B | `\cite{zhou2026agentnative}` (its framing sentence), `\cite{huan2026secondhalf}`, `\cite{zhou2026externalization}` | a survey that shows the community treats memory as context management only |
| B2 | Existing evaluation of agent memory is dominated by end-to-end task-success metrics that treat the memory system as a black box | B | `\cite{zhou2026agentnative}`; corroborated by the metric practice in `\cite{wu2024longmemeval}`, `\cite{maharana2024locomo}` | a widely adopted evaluation suite that already reports per-operation store metrics |
| B3 | Recall and final-answer accuracy cannot detect inadmissible retrieval (wrong principal, policy, or lifecycle state) | B | `\cite{wang2026rightmemory}` (isolates and scores this), `\cite{mullick2026store}` (stale-record use), `\cite{chen2026ragwrong}` (answer correctness hides evidence use) | a demonstration that standard recall metrics already correlate with admissibility failures |
| B4 | Retention failure happens before retrieval: needed evidence can be evicted by the consolidation/compaction step that precedes any query | B | `\cite{song2026eviction}` (isolates the pre-retrieval failure), `\cite{wang2026emem}` (destructive de-contextualisation), `\cite{zhong2023memorybank}` (decay as a design choice) | a study showing eviction never removes evidence required later at non-trivial rates |
| B5 | Prospective memory is a distinct faculty that current agent benchmarks have only recently begun to measure separately | B | `\cite{einstein1990prospective}` (human evidence), `\cite{liu2026pmbench}` (PM-Bench), `\cite{zhao2026pmslm}` (typed intention stores) | a demonstration that recall-style benchmarks predict PM performance |
| B6 | Calibration of stored confidence requires the elicitation method and the outcome to be recorded together; reporting ECE without discrimination is uninformative | B | `\cite{xiong2023confidence}` (no strategy gives both), `\cite{ni2024probabilistic}` (probabilistic vs verbalised trade-off), `\cite{yang2024verbalized}` (prompt sensitivity), `\cite{guo2017calibration}` (metric definitions) | a store that reports calibratable confidence without storing the elicitation method |
| B7 | Write cost is a first-class design variable: append-and-evolve memories have reported quadratic write latency and high token cost | B | `\cite{song2026dmem}` (O(N²) write latency, token cost), `\cite{xu2026linkedin}` (deployment latency budgets), `\cite{zhou2026agentnative}` (localised maintenance cheaper) | measurements showing write cost is negligible at realistic store sizes |
| B8 | Writing to memory has an integrity problem independent of retrieval quality: poisoning and membership inference against agent memory are demonstrated | B | `\cite{sunil2026mempoison}`, `\cite{louck2026poisoning}`, `\cite{chen2026mrmmia}` | a store design with proven origin-bound write authority that removes the effect |
| B9 | MCP is the de facto interface for giving agents external capability, and its server ecosystem is already benchmarked at task level, with documented sampling bias | B | `\cite{mcpintro}`, `\cite{anthropicmcp2024}`, `\cite{mcpspec2025}`, `\cite{luo2025mcpbench}`, `\cite{luo2025mcpuniverse}`, `\cite{fan2025mcptoolbench}`, `\cite{afsar2026registry}`, `\cite{rodrigues2026mcppatterns}` | adoption figures showing a different dominant interface |
| B10 | Agent benchmark papers frequently omit the information needed to reproduce a comparison (scaffold, prompts, budgets) | B | `\cite{moghadasi2026disclose}` (audit of twelve papers), `\cite{rassheed2024multieval}` (grader variance) | an audit showing near-universal disclosure |

## 3. Claims of our own (C) and NOT CONFIRMED (D)

| # | Claim | Status | Basis / why not confirmed |
|---|---|---|---|
| C1 | A six-operation taxonomy (WRITE, READ, CONSOLIDATE, REVISE, INTEND, ASSERT) plus two record axes (PROVENANCE, LIFECYCLE) is sufficient to classify current agent-memory systems | C | Argument by construction over seven systems (`taxonomy.md` §4). *Not proven complete*: three of seven systems need O4–O6 and the axes are partial or absent in five of seven, which is evidence of useful coverage, not of sufficiency. A system found later that implements a seventh operation would falsify completeness. |
| C2 | An operation-level taxonomy is more portable across implementations than a type-level one, because store boundaries differ per architecture | C | Supports: `\cite{laird2022actrsoar}` (two architectures, different representations/metadata), `\cite{zhou2024structural}` (structure choice changes outcomes), plus the survey-type orders in `\cite{huan2026secondhalf}` and `\cite{zhang2024memorysurvey}`. This is an argument about design, not a measurement. |
| C3 | ~~A single protocol can produce comparable reports for two different memory systems, including one the protocol's authors did not write~~ | **promoted to A8 (first half only)** | The first half — one protocol, two systems, two validated comparable reports — is now an artefact claim (`refslint report … reports/*/report.json` → `VALID` ×2). The second half is **not** satisfied: both measured servers come from this author's reference set, and `sint-memory-mcp` lives in the author's own `sint` monorepo. A run against a server neither authored nor modified stays **D — NOT CONFIRMED**, and is the next step stated in the paper's Conclusion. |
| C4 | Grouping "memory identity, procedural memory and task state" under one operation set is the right decomposition for agents | C | Position. Competing decompositions exist: four internal modules (`\cite{zhou2026agentnative}`), five cognitive mechanisms (`\cite{huan2026secondhalf}`). We argue only that ours is the one that can be run against a foreign server. |
| C5 | Reporting cost (tokens, USD, latency) alongside accuracy changes which memory design looks preferable | C | Position with partial support: `\cite{song2026dmem}` and `\cite{zhou2026agentnative}` report cost-driven reversals of preference; `\cite{fan2025mcptoolbench}` reports cost in MCP tool-use settings. The two runs of Section 6 cannot test it: no model is in the loop, so `tokens_per_write`, `cost_per_1000_writes_usd` and `cost_per_consolidation_usd` are `null` in both reports by construction. Latency, the one cost axis the runs do carry, is reported as single-host wall clock and not used to rank. |
| C6 | Pre-registered hypotheses of the comparison (stated before the run, Section 6): **H1** explicit lifecycle metadata buys a lower stale-read rate at a measurable write-latency cost; **H2** a subject without a prospective-memory operation fails T7 entirely while its T2/T6 scores stay high | C — outcome recorded, not claimed | H1 is not testable on this pair: neither subject exposes a lifecycle field (`lifecycle_state_exposed` `0.0` in both) and both serve stale content at `1.0` — the antecedent is absent, so the hypothesis is neither supported nor refuted. H2 holds for one of the two subjects (`sint-liquid-mcp` cue hit rate `0.0` at read recall `0.5`; `sint-memory-mcp` answers the cue). With two subjects the observation is a data point, not a test; the hypotheses stay falsifiable and are reported with their numbers in Section 6. |
| D1 | Our reference implementation has *better* memory quality than MemGPT / Zep / Mem0 / the other systems in the comparison | **D — NOT CONFIRMED** | One server of the implementation set now has a measured profile (`sint-liquid-mcp`, Section 6), and the third-party systems of Table 2 were not run at all: no head-to-head comparison exists. Cost instrumentation is still absent (gap 4), and the two subjects both come from this author's repositories. Any sentence implying superiority must be removed before publication. |
| D2 | Human-like forgetting improves agent memory | **D — NOT CONFIRMED** | Evidence points the other way: LM memory is reported as far more reliable than human memory (`\cite{wang2026simulating}`). Decay is kept as a policy variant to be measured, and no claim of improvement is made. |
| D3 | The taxonomy is complete | **D — NOT CONFIRMED** | See C1. The paper states coverage over a named system set and dates the survey; completeness is not claimed. |
| D4 | The protocol's metrics transfer to industrial domains (PLC/HMI, electrical engineering) without new gold data | **D — NOT CONFIRMED** | Not tested. The closest precedent for executable domain evaluation is `\cite{trashchenkov2026power}` (power systems) and it is cited as precedent, not as transfer evidence. Any domain claim must come with a domain-specific gold set. |
| D5 | Rubric-scored metrics (e.g. faithfulness of a generalisation) are reproducible | **D — NOT CONFIRMED** | No double grading has been performed; κ is declared mandatory in the schema but no κ value exists yet. Until two graders have scored a sample and κ is reported, these metrics are exploratory. |
| D6 | The reference implementation is production-ready | **D — NOT CONFIRMED** | Nothing in this repository measures availability, concurrency, or failure rates under load; the repositories carry their own status statements, which this paper does not upgrade. |
| D7 | "No prior work defines an operation-level, implementation-agnostic evaluation protocol for memory servers" (the novelty claim behind C1/C3) | **D — pending a reproducible search** | This is the strongest and most fragile claim in the paper. It is a *search* result, so it must be published with the queries, the date and the screened set (`evidence/search-log.md`), phrased as "our search of arXiv and the MCP ecosystem on <date> returned N candidates, of which the closest were X, Y, Z", never as an unqualified first-ness claim. Nearest prior art already found: `\cite{luo2025mcpbench}` (MCP server evaluation, tool-use scope), `\cite{wang2026rightmemory}` (admissibility protocol, single property), `\cite{zhou2026agentnative}` (module-level evaluation of 12 systems). |

---

## 4. Rules that follow from this ledger

1. No number enters the paper unless it is produced by a command in `code/` or `tools/refslint`
   and its raw output path is stated.
2. No C-claim may be phrased as a result. C-claims use "we argue/we propose"; results use
   "was measured/was observed".
3. D-claims stay in §3 of this file, in the paper's Limitations section, or are deleted. They
   do not appear in the abstract or the introduction as motivation-by-implication.
4. The comparison section (paper §6) is written **only** from reports that pass
   `refslint report --schema code/protocol_schema.json reports/*/report.json`; two such reports
   now exist and §6 is written from them. Every table and every inline number in that section is
   generated from those two files by `refslint tables`, and `refslint tables --root . --check`
   (also a step of `bash build.sh` and of `bash verify_all.sh`) fails if a generated file drifts
   from the reports or if the text uses a number that has no report field. Hand-editing a table,
   a macro or the ledger is therefore not a documentation slip but a check failure.
5. Any further measurement (a third subject, a model in the loop, more repetitions) enters the
   paper through the same path: a new `reports/<run>/report.json` that validates, then
   `refslint tables`, then the text. Numbers are never carried across versions by hand.
6. If a curated source is later found to be mis-cited (its `supports` statement not borne out by
   the paper), the fix is an edit to `code/refs.toml` and a rebuild — which re-numbers the
   verification table and makes the change visible in `CHANGELOG.md`.
