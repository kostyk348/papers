# Taxonomy of agent-memory operations

Version 0.1 (first deliverable). Working language of the paper is English; this
specification is written in Russian for internal review, every definition is phrased so
that it translates one-to-one into the English text.

Scope: memory that an LLM agent accesses through an interface — in this paper, through
Model Context Protocol servers (`\cite{mcpspec2025}`, `\cite{mcpintro}`), but the
definitions are stated so that a non-MCP store (REST service, library, file tree) can be
mapped onto them.

---

## 1. Design rule

An **operation** here is not a data structure and not an algorithm. It is a unit of
behaviour identified by four things:

1. **Precondition** — what must be true of the store before the call is legal.
2. **Postcondition** — what must be true afterwards (state change, or an explicit no-op).
3. **Observable evidence** — what the interface returns that lets a third party decide
   whether the postcondition holds.
4. **Failure modes** — what it looks like when the postcondition silently does not hold.

Two consequences that make the taxonomy usable for comparison:

- A system that reaches the same postcondition with a different structure (embedding index
  vs. knowledge graph vs. plain rows) is classified the same. This is the point: current
  type-level taxonomies order the field by *memory type* (substrate, cognitive mechanism,
  subject — `\cite{huan2026secondhalf}`) or by *memory form* (source, form, operation —
  `\cite{zhang2024memorysurvey}`), and both orders change as new structures appear. An
  operation survives the change.
- An operation that a system does not have cannot be hidden behind end-to-end accuracy:
  absence is visible as a missing tool, a missing postcondition, or a failed evidence check.

Cross-check rule used throughout: for every operation in this taxonomy there must be at
least one **existing system** that implements it and at least one **measurement in the
literature** that shows the operation matters. The columns "реализующие системы" and
"зачем измерять" in §3 carry these obligations. Where the evidence is thin, the entry says
so explicitly instead of citing something tangentially related.

---

## 2. The two cross-cutting record axes

Before the operations, two properties that every record must carry, because without them a
postcondition cannot be checked externally.

### A. PROVENANCE — what a record must say about itself

Minimum set (value-free, i.e. readable without exposing the payload):

| Field | Meaning | Why the protocol needs it |
|---|---|---|
| `source` | identity of the origin (user, tool, model, URL, session) | conflict resolution and poisoning defence (`\cite{schuster2026sourcepref}`, `\cite{louck2026poisoning}`) |
| `author` | which agent/model wrote it | auditability of multi-agent memories (`\cite{souza2025provagent}`) |
| `written_at` | timestamp with timezone | temporal reasoning (`\cite{maharana2024locomo}`) |
| `derived_from` | ids of records this one was computed from | consolidation must not destroy derivability (`\cite{wang2026emem}`) |
| `confidence` | stored confidence, if any, plus how it was produced | calibration is only checkable if the score is stored (`\cite{ni2024probabilistic}`) |
| `lifecycle` | `active` / `superseded` / `expired` / `deleted` | the "knowing the store" failure (`\cite{mullick2026store}`) |

The vocabulary is deliberately aligned with PROV-O (`\cite{prov_o}`): `wasGeneratedBy`,
`wasDerivedFrom`, `wasAttributedTo`, `wasInvalidatedBy`. We do not invent a new provenance
model; we require that a store can export this subset, because a report about memory that
cannot say where a record came from is not checkable.

### B. LIFECYCLE — records are not append-only

An append-only store can answer "what did I see" and cannot answer "what is true now". The
lifecycle states above are the minimal machinery for the second question
(`\cite{mullick2026store}`, `\cite{wang2026rightmemory}`). A `write` that can only append
is therefore not equivalent to a `write` that can supersede.

---

## 3. The six operation families

### O1 — WRITE (запись/приём в хранилище)

- **Precondition:** writer identity and scope are known; the payload is non-empty.
- **Postcondition:** exactly one new record exists, or exactly one existing record is
  mutated; the record carries the PROVENANCE minimum; the operation is idempotent with
  respect to a supplied `idempotency_key`.
- **Evidence:** returned record id + the record as stored (read-back), plus a counter of
  records before/after.
- **Variants:** `append` (no comparison with existing state), `upsert` (match on a key or
  on semantic equality), `link` (create an explicit relation to other records),
  `distil` (store a critique or guideline produced by an earlier pass — the write-side ablation
  reported in `\cite{gallego2026feedback}`).
- **Failure modes:** silent duplicate; write that succeeds but is unreadable (no read-back
  path); payload stored without provenance; non-idempotent retry creating two records.
- **Реализующие системы:** MemGPT paging into external storage (`\cite{packer2023memgpt}`),
  RET-LLM's write memory unit (`\cite{modarressi2023retllm}`), A-MEM note construction with
  evolution of existing notes (`\cite{xu2025amem}`), Mem0 extraction+consolidation on write
  (`\cite{chhikara2025mem0}`), Zep episode→graph ingestion with validity intervals
  (`\cite{rasmussen2025zep}`), Liquid Memory `liquid_belief` upsert-by-claim
  (`\cite{sintliquid}`), Anthropic's memory tool (`\cite{anthropicmemorytool}`), ChatGPT
  saved memories (`\cite{openaimemory2024}`).
- **Зачем измерять:** write cost is a first-class design variable: append-and-evolve designs
  were reported at O(N²) write latency with unacceptable token cost
  (`\cite{song2026dmem}`), and deployed systems report latency budgets as the binding
  constraint (`\cite{xu2026linkedin}`).

### O2 — READ (чтение: поиск + допуск + выбор)

- **Precondition:** query (text / embedding / structured filter) and a token or result
  budget.
- **Postcondition:** the returned set is (a) relevant, (b) *admissible* — the caller is
  allowed to see it and its lifecycle state permits use, (c) traceable to record ids.
- **Evidence:** ordered list of (record id, score, provenance); the discriminates that
  dropped candidates are reported separately from "no candidate existed".
- **Variants:** `recall` (retrieve), `rank` (retrieve + order), `filter` (admissibility),
  `abstain` (return nothing and say why).
- **Failure modes:** "retrieval succeeded, answer is wrong"; retrieval of records the caller
  must not see (principal/policy/lifecycle — the failure isolated in
  `\cite{wang2026rightmemory}`); positional bias making the same evidence hard to use
  (`\cite{liu2023lostmiddle}`, `\cite{hseih2024foundmiddle}`); context inflation (many
  tokens, little evidence, `\cite{xu2026contextpipe}`).
- **Реализующие системы:** Generative Agents' recency/importance/relevance scoring
  (`\cite{park2023generative}`), MemoryBank retrieval with decay
  (`\cite{zhong2023memorybank}`), HippoRAG single-step multi-hop graph retrieval
  (`\cite{gutierrez2024hipporag}`), Self-RAG decided retrieval with critique
  (`\cite{asai2023selfrag}`), Zep hybrid search over a temporal graph
  (`\cite{rasmussen2025zep}`), store routing across specialised stores
  (`\cite{gaikwad2026routing}`), `liquid_query`/`epistemic_status`/`prospective_resume`
  (`\cite{sintliquid}`, `\cite{sintepistemic}`, `\cite{sintprospective}`).
- **Зачем измерять:** recall and answer accuracy cannot detect inadmissible retrieval
  (`\cite{wang2026rightmemory}`), and final-answer correctness hides whether the model used
  the retrieved evidence at all (`\cite{chen2026ragwrong}`).

### O3 — CONSOLIDATE (консолидация, включая офлайн-консолидацию)

- **Precondition:** the input set is closed (a fixed batch or a fixed time window) — the
  point of consolidation is that it does not depend on the next query.
- **Postcondition:** the input set is replaced by a smaller set of generalisations; every
  generalisation lists the ids it was derived from (`derived_from`); the number of stored
  items strictly decreases (or the operation reports itself as a no-op).
- **Evidence:** compression ratio, the `derived_from` links, and a *probe set* re-run after
  consolidation (the same questions asked before and after).
- **Variants:** `merge` (duplicates → one record), `abstract` (episodes → rule),
  `index` (records → derived structure, e.g. graph), `decay` (retention policy).
- **Failure modes:** destructive de-contextualisation — compressing sequential dependencies
  into structures that lose the dependency (`\cite{wang2026emem}`); consolidation that
  improves surface metrics and destroys provenance; unbounded cost of global reorganisation
  (localised maintenance was found more cost-efficient, `\cite{zhou2026agentnative}`).
- **Реализующие системы:** Generative Agents reflection (`\cite{park2023generative}`),
  MemoryBank summarisation (`\cite{zhong2023memorybank}`), Mem0 consolidation
  (`\cite{chhikara2025mem0}`), GraphRAG community indexing (`\cite{edge2024graphrag}`,
  `\cite{microsoftgraphrag}`), Agent Workflow Memory induction (`\cite{wang2024awm}`),
  Agent Dreaming (`\cite{sintdream}`), sleep-time compute (`\cite{lin2025sleeptime}`).
- **Зачем измерять:** retention after consolidation is a separate stage from retrieval —
  evidence can be evicted before a query ever arrives (`\cite{song2026eviction}`); and the
  biological rationale for offline consolidation (`\cite{walker2004sleep}`,
  `\cite{whelan2021replay}`) does not transfer automatically, since LM memory is already far
  more reliable than human memory (`\cite{wang2026simulating}`).

### O4 — REVISE (ревизия убеждений: обнаружение противоречия и выбор)

- **Precondition:** incoming claim and stored records overlap; a conflict is possible.
- **Postcondition:** for each conflict either (a) the old record becomes `superseded` with a
  pointer to the new one, (b) the new claim is rejected with a recorded reason, or (c) the
  conflict is stored as unresolved — no conflict may silently vanish; the revision chain is
  readable; unrelated records are unchanged.
- **Evidence:** revision chain (from → to, reason, time, author), unchanged-record hash set,
  and a post-revision read that must not return the superseded value.
- **Variants:** `detect` (contradiction detection only), `contract` (drop the old belief),
  `expand` (keep both with explicit unresolved status), `retract` (policy/temporal
  invalidation).
- **Failure modes:** the "knowing the store" failure — the agent answers from a record it
  should no longer use (`\cite{mullick2026store}`); collateral damage to unrelated records
  (see mass editing, `\cite{meng2022memit}`); revision without a chain (un-auditable);
  source-blind conflict resolution (`\cite{schuster2026sourcepref}`).
- **Реализующие системы:** Zep bi-temporal invalidation (`\cite{rasmussen2025zep}`),
  knowledge-editing methods applied to internal memory (`\cite{meng2022rome}`,
  `\cite{meng2022memit}`, `\cite{wang2024editsurvey}`), persistent belief statements in a
  symbolic memory (`\cite{kassner2021belief}`), Liquid Memory contradiction/revision chain
  (`\cite{sintliquid}`), ChatGPT's user-visible "what do you remember" controls
  (`\cite{openaimemory2024}`).
- **Формальная рамка:** revision here is a concrete instance of AGM belief change
  (`\cite{falakh2021agm}`) with a maintenance component in the truth-maintenance tradition
  (`\cite{falkenhainer2013belief}`); the paper does not claim that stores satisfy the
  postulates, it claims they must be *measured* against a stated revision policy.
- **Зачем измерять:** conflict classes are already catalogued
  (`\cite{xu2024conflictsurvey}`, `\cite{su2024conflictbank}`) and stale-read after revision
  is an observable, falsifiable failure (`\cite{mullick2026store}`, `\cite{panda2026twist}`).

### O5 — INTEND (проспективная память: намерение и его срабатывание)

- **Precondition:** deferred intention with a trigger condition (cue event, time, state) and
  a scope.
- **Postcondition:** the intention survives context loss (new session / compacted history);
  it fires exactly once at the cue, with the stored context available at fire time; it can be
  suspended and resumed; it does not fire at a non-cue.
- **Evidence:** intention id + trigger spec, fire/no-fire record with timestamps, preserved
  context at fire time.
- **Варианты:** `push`/`pop` stack semantics, `suspend`/`resume`, `snapshot`/`restore`,
  markers for interrupted work.
- **Failure modes:** intention lost across sessions; false firing (interference with the
  ongoing task); firing without the context needed to act (the intention is remembered, the
  reason is not); unbounded stack growth.
- **Реализующие системы:** Prospective Memory server (`\cite{sintprospective}`), PM
  benchmarks and typed-intention stores (`\cite{liu2026pmbench}`, `\cite{zhao2026pmslm}`).
- **Зачем измерять:** prospective memory is empirically distinct from retrospective recall
  in humans (`\cite{einstein1990prospective}`), and serious effort in 2026 is being spent on
  measuring it separately (`\cite{liu2026pmbench}`) — it cannot be folded into recall
  accuracy.
- **Отдельная позиция:** this is the operation most often missing entirely. A store that is
  evaluated only by recall cannot express "did the agent remember to do the thing at the
  right moment".

### O6 — ASSERT (эпистемический статус и метапамять)

- **Precondition:** the agent is about to state something (or to store a claim).
- **Postcondition:** the claim carries a status from a fixed vocabulary
  (`known` / `inferred` / `assumed` / `uncertain`, or `verified` / `reported` / `guessed`),
  a confidence score with a declared elicitation method, and — when the store is asked —
  an honest answer about *what the store does not contain*.
- **Evidence:** status, score + elicitation prompt, and the outcome (correct / wrong /
  abstained) recorded together, so calibration can be computed later.
- **Variants:** `register` (store a claim with status), `calibrate` (score stored confidence
  against outcomes), `abstain` (refuse or ask), `monitor` (report store-level state:
  what is expired, what was evicted, what is uncertain).
- **Failure modes:** confidence stored without the elicitation method (un-reproducible
  calibration); verbalised confidence that is well ordered but badly calibrated, or the
  reverse (`\cite{xiong2023confidence}`, `\cite{ni2024probabilistic}`); confident answers
  outside the knowledge boundary (`\cite{cheng2024knownotknow}`); a store that cannot report
  its own gaps (metamemory monitoring, `\cite{mullick2026store}`).
- **Реализующие системы:** Epistemic Status Tracking (`\cite{sintepistemic}`), verbalised
  confidence training (`\cite{lin2022uncertaintywords}`), self-evaluation of correctness
  (`\cite{kadavath2022know}`), calibration-elicitation studies (`\cite{tian2023justask}`),
  per-record reliability estimates (`\cite{feng2026baremem}`), uncertainty decomposition for
  clarification (`\cite{matsnev2026uncertdec}`).
- **Метрики-предки:** ECE and reliability diagrams (`\cite{guo2017calibration}`) and their
  human-rating origin (`\cite{lichtenstein1982calibration}`); recursive estimation with
  uncertainty (`\cite{kalman1960filter}`) as the classical form of the update.

---

## 4. Mapping of seven existing systems onto the taxonomy

Legend: `+` operation present and observable through the interface, `~` partially present
(possible but not exposed/checked), `-` absent.

| System | O1 WRITE | O2 READ | O3 CONSOL. | O4 REVISE | O5 INTEND | O6 ASSERT | Provenance | Lifecycle |
|---|---|---|---|---|---|---|---|---|
| MemGPT / Letta (`\cite{packer2023memgpt}`, `\cite{lettadocs}`) | + (paging, self-edit) | + (tiered read) | ~ (self-edit blocks) | ~ (rewrite in place) | - | ~ (agent-reported) | ~ | - |
| Generative Agents (`\cite{park2023generative}`) | + (observation) | + (scored retrieval) | + (reflection) | - | - | - | ~ (recency/importance) | - |
| MemoryBank (`\cite{zhong2023memorybank}`) | + | + (decay) | + (summaries) | - | - | - | - | ~ (decay) |
| A-MEM (`\cite{xu2025amem}`) | + (note + link) | + (link traversal) | ~ (evolution) | ~ (note update) | - | - | ~ (links) | - |
| Zep / Graphiti (`\cite{rasmussen2025zep}`, `\cite{graphiti}`) | + (episode→graph) | + (hybrid) | ~ (summaries) | + (validity intervals) | - | - | + (temporal edges) | + (invalidated edges) |
| Mem0 (`\cite{chhikara2025mem0}`, `\cite{mem0docs}`) | + (extract) | + | + (consolidate) | ~ (update/delete) | - | - | ~ | ~ |
| Liquid / Epistemic / Dream / Prospective servers (`\cite{sintliquid}`, `\cite{sintepistemic}`, `\cite{sintdream}`, `\cite{sintprospective}`) | + | + | + | + (revision chain) | + (stack, suspend/resume) | + (status + confidence) | ~ (reason strings) | + (states per record) |

The table is the argument for the taxonomy in one view: across seven systems, three
operation families (O4, O5, O6) are covered by exactly two systems, and the *cross-cutting*
axes are the weakest area overall — provenance and lifecycle metadata are partial or absent
in five of seven. Neither fact is visible in an end-to-end accuracy number.

## 5. Explicit differences from the three closest works

| Prior work | What it organises | What it does not give | Our difference |
|---|---|---|---|
| `\cite{huan2026secondhalf}` — survey, Jan 2026 | Memory along substrate (internal/external), cognitive mechanism (episodic, semantic, sensory, working, procedural) and subject (agent/user); reviews benchmarks and metrics | No interface-level operation definitions, no runnable procedure, no per-record metadata requirements | We define six operations by pre/postconditions and observable evidence, and we require the PROVENANCE/LIFECYCLE axes that make a postcondition externally checkable |
| `\cite{zhou2026agentnative}` — 4-module framework + 12 systems, Jun 2026 | The system into four modules (representation/storage, extraction, retrieval/routing, maintenance) and measures end-to-end cost/quality plus ablations (representation fidelity, retrieval precision, update correctness, long-horizon stability) | Modules are internal boundaries: a third party cannot run the framework against a server it does not implement; the operations are not exposed as a shareable contract | Our unit of comparison is the *interface operation*, so two vendors' servers can be run and reported with the same fields; their conclusion (no architecture dominates; localised maintenance is cheaper) becomes a hypothesis our protocol can test per operation |
| `\cite{zhang2024memorysurvey}` — memory mechanism survey, Apr 2024 | Memory sources, forms and operations of LLM agents | Pre-dates the MCP memory-server ecosystem; operations are described per system, not as testable postconditions | We add O5 (INTEND) and O6 (ASSERT) as first-class operations with their own tasks, and we tie each operation to a measurement that the literature already shows matters |
| `\cite{luo2025mcpbench}` — MCP server evaluation (MCPBench) | Task effectiveness and efficiency of MCP servers | Scoped to tool-use tasks; memory semantics (revision, forgetting, intentions) are outside its scope | We keep their spirit (implementation-agnostic, automated, cost-aware) and change the object: the memory store itself |
| `\cite{wang2026rightmemory}` — retrieval admissibility verification | One property: whether a retrieved record is admissible for the request (principal, policy, lifecycle) | Single-operation, not a protocol for a memory store as a whole | We adopt their finding (recall cannot detect inadmissibility) as a task family (T4) rather than duplicating their contribution |

## 6. Reference-implementation gap map (facts, with commits)

Taken from `data/repo-facts.json`, collected 2026-10-16 by `code/repo_facts.py` from
`git clone --depth 1` (read-only; the working repositories were not modified). An independent
collection path — `data/raw/upstream/manifest.json`, which reads head commits through the
GitHub API for 20 SINT repositories and caches their READMEs — agrees with these SHAs for every
shared repository. The table below names the six repositories whose tool inventory maps onto
O1--O6; the remaining SINT repositories in that manifest are teammate, router and domain
services that do not claim the memory operations of this taxonomy.

| Server | Commit | MCP tools parsed | Operations covered |
|---|---|---|---|
| `sint` (monorepo) | `7554f7eef853` | 125 | core memory system + the servers below |
| `sint-liquid-mcp` | `bfffce2cdec3` | 8 | O1 upsert, O2 read, O4 detect/contract/expand/retract + chain |
| `sint-epistemic-mcp` | `5d339df2cbb0` | 6 | O6 register/calibrate/gaps/graph |
| `sint-dream-mcp` | `4ab0c130eabd` | 6 | O3 consolidate/generalise/error patterns/skill suggestions |
| `sint-prospective-mcp` | `b16b7172ec33` | 8 | O5 push/pop/suspend/resume/marker/snapshot/restore |
| `sint-llm-swarm` | `fc705f8d7a3c` | 6 | cross-checking writes by role (provenance/uncertainty roles) |

Known gaps of the reference implementation, stated before any measurement:

1. **No unified store.** The operations live in six processes; a single record's provenance
   is not automatically shared between them. The PROVENANCE axis is therefore the weakest
   part of our own implementation, and the protocol (task family T7) is designed to expose it.
2. **No PROV-O export.** `derived_from` is carried as free-text reason strings, not as a
   checkable `wasDerivedFrom` graph.
3. **No measured baselines yet.** Every number in section 6 of the paper must come from a
   protocol run; none exists as of this deliverable.
4. **No cost instrumentation.** Token and latency accounting is not implemented, although
   the literature treats both as first-class (`\cite{song2026dmem}`,
   `\cite{xu2026linkedin}`, `\cite{fan2025mcptoolbench}`).

These four gaps are the reason the protocol specifies *cost*, *provenance* and *restart*
probes as mandatory rather than optional: they are exactly what an implementation tends to
skip.

---

## 7. What is deliberately *not* in the taxonomy

- **No accuracy claim.** The taxonomy says nothing about which operation is more valuable.
- **No anthropomorphic claim.** Decay and forgetting appear as *policy variants*, not as
  requirements: LM memory is already more reliable than human memory
  (`\cite{wang2026simulating}`), so "human-like" is not treated as a goal.
- **No assumption of a single store.** Multi-store, multi-user and multi-principal layouts
  are first-class (`\cite{rezazadeh2025collabmem}`, `\cite{li2025maas}`,
  `\cite{wang2026rightmemory}`).
- **No internal-structure requirements.** Embeddings, graphs, files and tables are all
  admissible; only pre/postconditions and evidence are constrained.
