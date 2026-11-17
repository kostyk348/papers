# Outline — Memory Operations in MCP Agent Ecosystems

Working title: **Memory Operations in MCP Agent Ecosystems: an Operation Taxonomy, an
Evaluation Protocol, and a Reference Implementation**

Author: Konstantin Parashchuk. AI assistance: see `README.md` (section "AI use") and
`claims.md` (section "Что не подтверждено").

Document status: **first deliverable of the paper** — outline, taxonomy, protocol,
verified related work, claims ledger. Not a publication draft; publication only after the
author decides.

---

## 0. Why this paper can be cited

The paper does not report "our software is nice". It ships three reusable artefacts:

| Artefact | Where it lives | What a third party can do with it |
|---|---|---|
| Operation taxonomy for agent memory | `taxonomy.md`, section 3 of the paper | Classify any memory system, including one that did not exist when this was written |
| Evaluation protocol (tasks, metrics, procedure, report schema) | `protocol.md`, `protocol_schema.json` | Run two different memory servers and obtain a comparable report |
| Reference implementation + gap map | six public MCP servers (`\cite{sintrepo}`) + `data/repo-facts.json` | Reproduce the implementation, or see exactly which operation it fails to cover |

The claim "comparability" is only meaningful if a reader can check what was compared. That
is why every citation in this paper is machine-resolved (`reference-verification.md`) and
every number about our own implementation carries a commit SHA (`data/repo-facts.json`).

---

## 1. Introduction (structure, ~1 page)

1.1 The problem: an agent's memory has become a *store* with its own lifecycle, while the
literature still evaluates it as a black box behind end-to-end task success. The 2026
surveys state this explicitly: agent memory now "supports persistent information storage,
retrieval, update, consolidation, and dynamic lifecycle governance", yet "existing
evaluations still benchmark agent memory mainly through end-to-end task success metrics
(e.g. F1, BLEU), while treating the underlying system as a monolithic black box"
(`\cite{zhou2026agentnative}`).

1.2 Why MCP makes this urgent: MCP is the dominant way agents discover and call external
capability (`\cite{mcpintro}`, `\cite{anthropicmcp2024}`), tool-use benchmarking on real MCP
servers already exists (`\cite{luo2025mcpuniverse}`, `\cite{fan2025mcptoolbench}`,
`\cite{luo2025mcpbench}`), and the ecosystem's sampling problems are documented
(`\cite{afsar2026registry}`). Memory servers are being published into that ecosystem with
no shared evaluation vocabulary.

1.3 Gap in one sentence: there is a *type-level* taxonomy of agent memory
(`\cite{huan2026secondhalf}`, `\cite{zhang2024memorysurvey}`) and there are *task-level*
benchmarks (`\cite{wu2024longmemeval}`, `\cite{maharana2024locomo}`), but no
*operation-level, implementation-agnostic protocol* that any MCP memory server can be run
against to produce a comparable, machine-checked report.

1.4 Contributions (C1–C4, mapped to sections and artefacts).
1.5 Non-claims: what this paper does not say (no claim of superior accuracy, no claim that
the taxonomy is complete, no anthropomorphic claim that artificial memory should decay like
human memory — see `\cite{wang2026simulating}`).

## 2. Background and terminology (structure)

2.1 Agent memory as a data-management problem: the four-module view
(`\cite{zhou2026agentnative}`) and where an operation-level view differs (it can be checked
across vendors without opening the system).
2.2 MCP: tool discovery, JSON-RPC framing, revisions (`\cite{mcpspec2025}`, `\cite{jsonrpc2}`).
2.3 Cognitive-architecture vocabulary borrowed deliberately and narrowly: declarative vs
procedural stores (`\cite{laird2012soar}`, `\cite{anderson2004actr}`, `\cite{laird2022actrsoar}`),
encoding specificity and source monitoring (`\cite{tulving1973encoding}`,
`\cite{johnson1993source}`), prospective memory as a distinct faculty
(`\cite{einstein1990prospective}`), offline consolidation (`\cite{walker2004sleep}`).
Explicit warning: the analogy is used for *interface design*, not for claiming equivalence.
2.4 Belief change vocabulary: AGM postulates (`\cite{falakh2021agm}`), truth maintenance
(`\cite{falkenhainer2013belief}`), knowledge conflicts (`\cite{xu2024conflictsurvey}`,
`\cite{su2024conflictbank}`).

## 3. Taxonomy of memory operations (content: `taxonomy.md`)

3.1 Design rule: an operation is defined by its *preconditions, postconditions and
observable evidence*, not by the data structure used.
3.2 The six operation families — O1 WRITE, O2 READ, O3 CONSOLIDATE, O4 REVISE, O5 INTEND,
O6 ASSERT — plus two cross-cutting record axes (PROVENANCE, LIFECYCLE).
3.3 Mapping table: seven existing systems/servers expressed in the taxonomy; for each,
which operations exist, which are absent, and what the absence costs.
3.4 Explicit differentiators against the three closest works
(`\cite{huan2026secondhalf}`, `\cite{zhou2026agentnative}`, `\cite{zhang2024memorysurvey}`).
3.5 Reference implementation coverage: which operations the six public servers cover, at
which commit, and which are still missing (the gap map).

## 4. Evaluation protocol (content: `protocol.md`)

4.1 Task families T1–T8 with fixed prompts, gold data construction and leakage controls.
4.2 Metrics: definition, formula, what computes it (harness / programmatic check / fixed
rubric with two graders), and the failure mode each metric is designed to expose.
4.3 Run procedure: isolated server instances, seeds, repetitions, budgets, masking,
restart durability probes, adversarial streams.
4.4 Report format and mandatory reporting block (motivated by
`\cite{moghadasi2026disclose}`); machine-readable schema + validator.
4.5 Threats to validity: small samples, self-selection of servers
(`\cite{afsar2026registry}`), positional confounds (`\cite{liu2023lostmiddle}`),
token-budget confounds (`\cite{xu2026contextpipe}`), grader subjectivity.

## 5. Reference implementation and gap map (content: C3, `data/repo-facts.json`)

5.1 Architecture of the six servers; operation-by-operation mapping to section 3.
5.2 What the implementation does *not* do — stated before any measurement.
5.3 Cost of the design: write latency and token cost are design variables, and the design
is criticised in the literature for O(N²) write behaviour in append-and-evolve systems
(`\cite{song2026dmem}`) — so the protocol measures it instead of asserting it.

## 6. Worked comparison (structure; results to be produced, not asserted)

6.1 Selection of two or more servers from different families, with the selection rule
reported.
6.2 Per-operation results table, each number with its report field and CI.
6.3 What differed and what did not; per `\cite{zhou2026agentnative}`, the expectation is
that no single architecture dominates.
6.4 Threats: n, cost, versions, prompt sensitivity of verbalised confidence
(`\cite{yang2024verbalized}`).

## 7. Limitations and open problems (must be a full section, not a footnote)

Mapped from `claims.md` section "Что не подтверждено": the measurement is a pair of profiles of two
servers from one reference set (not a ranking, and no third-party system was run), the confidence
intervals rest on the protocol minimum of three repetitions, one machine produced the latencies, no
model is in the loop (token/USD metrics are `null` by construction), the harness is still Python and
there is no offline recompute of a report from its raw logs, taxonomy completeness is unproven,
human-scored metrics are not yet double-graded, coverage is limited to MCP-exposed stores, domain
transfer is untested (`\cite{trashchenkov2026power}`), and safety metrics are scoped to write-path
poisoning and membership inference (`\cite{sunil2026mempoison}`, `\cite{chen2026mrmmia}`).

## 8. Conclusion

What can be reused on Monday: the operation checklist, the protocol schema, the report
validator, the reference implementation, and the gap map.

## Appendices

A. Full verification table (`reference-verification.md`).
B. Repository facts with commit SHAs (`data/repo-facts.json`).
C. Protocol schema and report validator (`code/protocol_schema.json`; the validator is the
`refslint report` subcommand, which the harness runs on every report it writes).
D. Search log for the "no prior protocol" claim (`evidence/search-log.md`).

---

## Writing order and next steps

1. `tex/main.tex` — English paper that compiles (version 1.0): taxonomy, protocol, reference
   implementation, and §6 written from the two validated reports with generated tables.
2. Harness implementation against two servers (owner: `fabricessa`) — **done**: two runs,
   both `VALID`; §6 is written from them and every number in it is generated by `refslint tables`.
3. Review by `libraressa` → author decision on venue (arXiv first).
4. Open after 1.0: a third and fourth subject outside this author's repository set, a model in the
   loop (so token/cost metrics stop being `null`), more repetitions, and the Rust port of the harness.
