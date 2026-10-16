# Evaluation protocol for agent-memory systems (v0.1, first deliverable)

Purpose: to make two different memory systems **comparable** by running them through the
same task stream and emitting a report with the same fields, produced by programs rather
than by prose. A report that a validator rejects is not a report.

Status of this document: the protocol is fully specified here, including the machine
readable task/report schema (`code/protocol_schema.json`) and a validator (`refslint report`,
subcommand of the pipeline's one Rust binary; the harness runs that same subcommand on every report
it writes). **The protocol has since been executed:** two
per-operation reports exist under `reports/` (see `README.md` §4b). Every statement about
results is marked as a hypothesis, and every number in the paper must come from a report
produced by this procedure. The protocol exists because context management in practice is a
manual engineering exercise (`\cite{anthropiccontext}`) that becomes comparable only when it is
written down.

Machine-checkable parts of this document:

| Claim | Checked by |
|---|---|
| Report structure is machine-verifiable | `refslint report --schema code/protocol_schema.json data/example-report.json` |
| Task and report schema exist and are consistent | `code/protocol_schema.json` (JSON Schema, draft 2020-12) |
| Metric definitions are unambiguous | metric table §3 (each row: formula, unit, computer) |
| Which parts have been measured | `README.md` §4b (two `reports/` runs) and `claims.md` section "Что не подтверждено" |

---

## 1. Task families

Each family is a *stream*: a scripted sequence of store operations and probes, generated
from a seed, with gold labels produced by the generator (never by a model). The stream is
the same for every system; only the adapter differs.

| ID | Family | What the stream injects | What is probed | Gold labels |
|---|---|---|---|---|
| **T1** | Write fidelity | scripted facts with unique keys, duplicates, retries of the same `idempotency_key` | read-back of each fact; record count before/after | the fact set is generated, so write precision/recall/duplicate counts are exact |
| **T2** | Read accuracy and ranking | a corpus of facts, then queries whose answers require 1..3 records | answer + retrieved ids + scores | which records are required, which are distractors |
| **T3** | Temporal reasoning | facts with explicit dates, superseding events over a simulated timeline, queries "as of date X" | answer + the record's `written_at`/validity used | the intended time slice of the gold fact |
| **T4** | Admissibility | facts scoped to principals/policies plus a caller identity; some records expired or superseded | retrieved ids for each caller; lifecycle metadata | which records are admissible for the caller and the moment |
| **T5** | Update and conflict | a first wave, then contradicting facts from sources of different stated authority (user vs. web page vs. earlier model output) | post-update answers, revision chain, kept/rejected record, unrelated records | which claim should win per a *stated* policy, plus the intended revision-chain edges |
| **T6** | Consolidation retention | a batch of episodes, then an offline consolidation run, then the *same* probe set as before consolidation | answers and retrievable ids before/after; compression counts; `derived_from` links | the probe set is fixed, so degradation is measured on identical questions |
| **T7** | Prospective memory | deferred intentions with cues, interleaved with an unrelated primary task; a session boundary in the middle | whether the intention fires at the cue, whether it fires at non-cues, whether the stored context is available at fire time | cue schedule generated with the stream |
| **T8** | Epistemic status and calibration | questions inside and outside the store's coverage; claims that are known / inferred / assumed | status + confidence + answer/abstain decision; store-level state report | for out-of-coverage items the gold outcome is "abstain"; for covered items there is a correct answer |
| **T9** | Durability and determinism | the same stream replayed after a process restart, and with the same seed twice | answers/ids identical where the policy is deterministic; store snapshot readable by a second process | identical-input equality, not similarity |
| **T10** | Adversarial and privacy | poisoning attempts through ordinary query-only interactions; targeted membership queries | whether poisoned content changes later answers; whether membership of a given record can be inferred | the injected poison's intended effect; membership labels |

Rationale links: T4 follows the finding that recall metrics cannot detect inadmissible
retrieval (`\cite{wang2026rightmemory}`); T5/T6 are separated because retention failure
happens *before* retrieval (`\cite{song2026eviction}`); T7 exists because prospective memory
is a distinct, separately benchmarked faculty (`\cite{liu2026pmbench}`,
`\cite{einstein1990prospective}`); T8 exists because calibration is only computable if the
score, its elicitation method and the outcome are stored together
(`\cite{ni2024probabilistic}`, `\cite{yang2024verbalized}`); T10 follows
(`\cite{sunil2026mempoison}`, `\cite{chen2026mrmmia}`).

Task counts per family are declared in the report, not here — the protocol fixes the
*fields*, not the size, so that a small pilot and a large study are both reportable and
distinguishable.

---

## 2. What is measured for every operation of the taxonomy

| Taxonomy operation | Task families that exercise it | Mandatory metric block |
|---|---|---|
| O1 WRITE | T1, T5, T9 | write precision / recall / duplicate rate, write latency p50/p95, tokens per write, cost per 1000 writes, idempotency violations |
| O2 READ | T2, T3, T4 | recall@k, MRR, required-record hit rate, provenance correctness, admissibility error rate, context inflation (retrieved tokens per useful token), read latency p50/p95 |
| O3 CONSOLIDATE | T6 | compression ratio, probe survival rate (Δanswers vs. pre-consolidation), derived_from completeness, cost per consolidation run |
| O4 REVISE | T5 | contradiction detection F1, revision correctness, chain completeness, **stale-read rate**, collateral-change rate |
| O5 INTEND | T7 | cue hit rate, false-fire rate, fire latency (calls/lines between cue and fire), context availability at fire, interference cost on the primary task |
| O6 ASSERT | T8 | ECE (binned, 10 bins), discrimination AUROC(confidence → correctness), abstention correctness on out-of-coverage items, monitoring accuracy (store-state self-report vs. ground truth) |
| Cross-cutting: PROVENANCE | T1, T2, T5, T6 | fraction of returned records carrying the required provenance fields; fraction of generalisations with a checkable `derived_from` |
| Cross-cutting: LIFECYCLE | T3, T4, T5, T9 | fraction of records whose lifecycle state is exposed and correct after supersede/expire/delete |
| T10 security | T10 | poisoning influence rate (Δanswer attributable to injected content), membership-inference advantage |

---

## 3. Metric definitions

Notation: `k` = number of returned records; `q` = number of queries; `n` = number of
repetitions. All rates are reported as mean over repetitions with a bootstrap 95 % CI
(1000 resamples over the repetition axis), never as a single run.

| Metric | Definition (computable from the report) | Unit | Computed by | Failure it exposes |
|---|---|---|---|---|
| write precision | `correct_written_records / written_records` where correctness is judged against generated facts | ratio | harness (programmatic) | storing wrong/extra facts |
| write recall | `correct_written_records / generated_facts` | ratio | harness | silent loss on write |
| duplicate rate | `extra_records_from_retries / written_records` | ratio | harness | non-idempotent write |
| write/read latency | wall-clock ms around the tool call, p50 and p95 | ms | harness | designs whose cost lives in the write path (`\cite{song2026dmem}`, index-maintenance cost `\cite{bayer1972btree}`) |
| tokens per operation | tokens in+out attributed to the call, from the runner's accounting | tokens | harness | context inflation; payload and context sizes are reported in the same unit as the information they carry (`\cite{shannon1948communication}`) |
| cost per 1000 writes / reads | tokens × pinned price table (price table date and source recorded in the report) | USD | harness | cost hidden by accuracy-only reporting (`\cite{zhou2026agentnative}`) |
| recall@k | `required_records_found / required_records` | ratio | harness | retrieval misses |
| MRR | mean of `1/rank(first required record)` | ratio | harness | good recall, bad ordering |
| provenance correctness | fraction of returned records whose `source`/`written_at` matches the gold record | ratio | harness | retrieval of the right text from the wrong source (`\cite{schuster2026sourcepref}`) |
| admissibility error rate | fraction of returned records that are inadmissible for the caller (principal/policy) or lifecycle-invalid (superseded/expired) | ratio | programmatic check | "recall looks fine, the answer must not be given" (`\cite{wang2026rightmemory}`) |
| context inflation | retrieved tokens / tokens of the records actually required | ratio | harness | hiding a bad ranker behind a large context (`\cite{xu2026contextpipe}`) |
| probe survival rate | `probes answered correctly after consolidation / same probes before` | ratio | harness | destructive de-contextualisation (`\cite{wang2026emem}`) |
| compression ratio | `items_before / items_after` a consolidation run | ratio | harness | consolidation that does not consolidate |
| derived_from completeness | fraction of post-consolidation records whose parents are resolvable by id | ratio | programmatic check | provenance destroyed by summarisation (`\cite{souza2025provagent}`) |
| contradiction detection F1 | standard F1 over detected conflict pairs vs. injected conflicts | ratio | harness | conflicts ignored (`\cite{xu2024conflictsurvey}`) |
| revision correctness | fraction of conflicts where the surviving record matches the *stated* policy (authority + recency) | ratio | programmatic check | newest-wins without a policy |
| chain completeness | fraction of revisions with a resolvable from→to edge, reason and timestamp | ratio | programmatic check | un-auditable revision |
| stale-read rate | fraction of post-revision queries returning the superseded value | ratio | harness | answering from a record that must no longer be used (`\cite{mullick2026store}`) |
| collateral-change rate | fraction of records unrelated to the conflict whose content or lifecycle changed during revision | ratio | programmatic check (hashes) | mass-editing damage (`\cite{meng2022memit}`) |
| cue hit rate | fraction of intentions fired at the correct cue | ratio | harness | lost intention across context loss |
| false-fire rate | fraction of non-cues at which an intention fired | ratio | harness | interference |
| context availability at fire | fraction of fires where the stored context needed to act is present in the store | ratio | programmatic check | "remembered to do it, forgot why" |
| abstention correctness | fraction of out-of-coverage items on which the system abstained (or asked) instead of answering | ratio | harness | confident answers outside coverage (`\cite{cheng2024knownotknow}`) |
| ECE | `Σ (|bin| / N) · |accuracy − mean confidence|`, 10 equal-width bins over reported confidence | ratio | harness | overconfidence; requires the stored confidence and the outcome (`\cite{guo2017calibration}`) |
| discrimination | AUROC of reported confidence against correctness | ratio | harness | confidence that is calibrated but useless (`\cite{xiong2023confidence}`) |
| monitoring accuracy | agreement between the store's self-reported state (expired / uncertain / evicted counts by attribute name) and ground truth | ratio | programmatic check | store cannot describe itself (`\cite{mullick2026store}`) |
| poisoning influence rate | fraction of adversarial trials whose post-poison answer matches the injected intent | ratio | harness | write path without authority checks (`\cite{louck2026poisoning}`) |
| membership-inference advantage | `TPR − FPR` of distinguishing a true member record | ratio | harness | privacy leakage from memory (`\cite{chen2026mrmmia}`) |
| restart survival | fraction of probes answered identically after a process restart with the same store directory | ratio | harness | in-process memory pretending to be durable |
| rubric scores (human) | only for answer *quality* dimensions that a program cannot judge (e.g. is a generalisation a faithful abstraction) | 1–5 | **two independent human/AI graders + Cohen's κ**, disagreements adjudicated by the author | grader subjectivity |

Rule about automation: everything in this table is a programmatic computation except the
last row. **No LLM judge is used for a metric that can be computed by code.** The rubric
subset is bounded, reported with κ, and never used for the headline comparison.

---

## 4. Run procedure

1. **Server sample and rule.** Servers are selected by a stated rule (e.g. "all memory-oriented
   servers in the MCP reference collection at revision R, plus two hosted memory APIs such as
   `\cite{zepdocs}` and `\cite{mem0docs}`"), not
   by convenience. The report lists every candidate, why it was included, and why others were
   not. This addresses the documented selection bias in MCP ecosystem studies
   (`\cite{afsar2026registry}`) and the comparability problem in agent benchmarks
   (`\cite{moghadasi2026disclose}`).
2. **Environment pinning.** For each server: repository URL + commit SHA, MCP specification
   revision (`2025-03-26` vs `2025-06-18` — `\cite{mcpspec2025mar}`, `\cite{mcpspec2025}`),
   transport (stdio / HTTP), runtime version, and the model + exact parameters used for any
   model-dependent step. Each server runs in its own fresh data directory.
3. **Stream generation.** All streams come from `seed` values recorded in the report. Gold
   labels are produced by the generator. Sample sizes per family are declared.
4. **Repetitions.** `n ≥ 3` per (server, family) for stochastic components; model temperature
   0 does not remove nondeterminism in retrieval or in MCP servers, so repetitions are
   mandatory and dispersion is reported.
5. **Budgets.** Per run: `max_calls`, `max_tokens`, `wall_clock_s`. Exhausting a budget is a
   *recorded outcome* (`budget_exhausted`), not a missing row; truncating a family silently is
   forbidden (`\cite{xu2026contextpipe}`).
6. **Masking and order control.** The position of the required evidence in the assembled
   context is recorded; for T2/T4 required records are placed at varied depths so that
   positional effects are measurable rather than assumed (`\cite{liu2023lostmiddle}`,
   `\cite{hseih2024foundmiddle}`).
7. **Scoring.** Programmatic metrics are computed from the raw returned JSON; the harness
   stores raw responses so a third party can recompute every number.
8. **Restart/durability (T9).** The server process is stopped, relaunched against the same
   data directory, and the probe set is re-run; the delta is reported. Where the store is a
   documented on-disk format (`\cite{sqlitefileformat}`), the store file is also copied and
   re-opened by a second process, which distinguishes durable state from in-process state.
9. **Adversarial order (T10).** Adversarial streams run *last* for each server, because they
   mutate the store; a server that fails T10 is not thereby disqualified from T1–T9.
10. **Report.** `report.json` (validated) + human-readable summary. Failures are reported with
    their taxonomy code (§6), never smoothed.

---

## 5. Report format

Machine-readable (JSON) with the mandatory blocks below; schema in
`code/protocol_schema.json`; validator `refslint report`, which the harness runs on the report it has
just written, so producer and judge use one validator rather than two copies. A run whose report
fails validation is reported as *not executed*.

```
report.json
├── protocol: {version, schema_uri, mcp_spec_revision, taxonomy_version}
├── subject: {name, repo_url, commit_sha, transport, runtime, deployment}
├── runner: {harness_commit, model, model_params, seeds[], repetitions}
├── budgets: {max_calls, max_tokens, wall_clock_s, price_table: {date, source}}
├── families[]: {id, n_tasks, repetition_results[], metrics{}, raw_log_paths[], failures[]}
├── operations{}: per-operation aggregate block from §2
├── statistics: {ci_method, n_resamples, per_metric: {mean, ci_low, ci_high, n}}
├── failures: {counts_by_code, examples[]}
└── disclosure: mandatory block (below)
```

Mandatory disclosure block — the fields a reader needs to reproduce the comparison, taken
from the audit of what agent benchmark papers fail to state (`\cite{moghadasi2026disclose}`):

```
disclosure:
  server_selection_rule      # free text + candidate list path
  candidate_servers[]        # every considered server, with include/exclude reason
  mcp_spec_revision
  mcp_servers_sha            # commit of the server under test
  transport
  model_under_test           # name + version string, or "none (store-only)"
  model_params               # temperature, top_p, max_tokens, system prompt hash
  embedding_model            # name/version or "server-internal, unspecified"
  seeds[]
  repetitions
  n_tasks_per_family
  budgets
  price_table                # date + source of per-token prices
  grader_info                # for rubric metrics only: who graded, κ value
  prompt_templates_hash      # hash of the fixed prompts used
  environment                # OS, runtime version, hardware class
  known_deviations           # anything that differs from this protocol, itemised
```

Cost reporting: cost is derived from the runner's token accounting and a price table whose
date and source are part of the report; costs quoted without a price-table date are not
comparable and the validator warns.

---

## 6. Failure taxonomy (recorded, not deleted)

| Code | Meaning |
|---|---|
| `server_error` | MCP tool returned a JSON-RPC error (`\cite{jsonrpc2}`) |
| `protocol_error` | malformed response, unknown tool, schema violation |
| `timeout` | call exceeded the per-call timeout in the report |
| `budget_exhausted` | run-level budget reached |
| `not_implemented` | the operation does not exist in the server (recorded as a capability fact, cf. `\cite{zhang2024memorysurvey}` for per-system operation coverage) |
| `nondeterministic` | identical input produced different output across repetitions |
| `leakage` | inadmissible record returned (T4) |
| `stale_read` | superseded record used (T5) |
| `lost_intention` | intention not fired at its cue (T7) |
| `harness_fault` | our fault; the trial is discarded and the discard is reported |

---

## 7. Threats to validity (must appear in the paper, not in a footnote)

1. **Small n.** Pilot-scale counts per family; per-family conclusions are exploratory.
2. **Repository snapshot.** MCP ecosystem studies show that samples select for servers that
   work (`\cite{afsar2026registry}`); we mitigate by listing all candidates, not by claiming
   representativeness.
3. **Store-internal model steps.** Extraction/consolidation may be performed by a model
   *inside* the server, unknown to us; recorded as `embedding_model: server-internal,
   unspecified` when it cannot be pinned.
4. **Prompt sensitivity.** Verbalised confidence and extraction quality are prompt-sensitive
   (`\cite{yang2024verbalized}`, `\cite{xiong2023confidence}`) — prompts are fixed by hash in
   the report, but a different prompt is a different experiment.
5. **Positional and budget effects** are controlled, not eliminated
   (`\cite{liu2023lostmiddle}`, `\cite{xu2026contextpipe}`).
6. **Rubric metrics** (last row of §3) carry grader subjectivity; κ is reported and they are
   never the headline.
7. **Domain transfer untested.** No claim is made that these tasks transfer to industrial
   domains without domain-specific gold data; the closest precedent for executable,
   domain-specific evaluation is `\cite{trashchenkov2026power}` and it is cited as such.
8. **No claim of superiority.** Following the finding that no single architecture dominates
   across workloads (`\cite{zhou2026agentnative}`), a per-operation profile is reported
   instead of a ranking.
