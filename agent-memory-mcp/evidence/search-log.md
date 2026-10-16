# Search log — basis for the novelty claim

This log exists because the paper's strongest claim ("no prior operation-level,
implementation-agnostic evaluation protocol for MCP memory servers") is a **search result**, not
a measurement. A search result is publishable only with its queries, its date and the screened
set, phrased as "our search on <date> returned N candidates, the closest being X, Y, Z". Anything
stronger would be a first-ness claim the authors cannot back.

Collection date: **2026-10-16** (all raw responses in `evidence/raw/`, all times UTC).

---

## 1. Queries actually run

### 1.1 Round 1 — broad mapping of the field (22 queries, 146 unique records)

Raw: `evidence/raw/queries1.json`, `evidence/raw/arxiv1.json`

```
all:LLM agent memory long-term
all:agent memory consolidation
all:Model Context Protocol agents tools
ti:MemGPT
ti:Generative Agents interactive human behavior
all:memory retrieval augmented generation agent
all:belief revision knowledge conflict LLM
all:confidence calibration large language models
all:knowledge editing large language models survey
all:long-term conversational memory dialogue agent
all:episodic memory LLM agent architecture
all:reflection self-improvement LLM agent
all:benchmark memory LLM agents evaluation
all:multi-agent LLM collaboration communication
all:vector database approximate nearest neighbor search
all:knowledge graph retrieval LLM HippoRAG
all:context window compression long context LLM
all:prospective memory intention agent
all:forgetting catastrophic interference neural networks
all:Mem0 memory layer AI agents
all:A-MEM agentic memory
all:agent tool use benchmark MCP
```

### 1.2 Round 2 — targeted works and concepts (28 queries, 126 unique records)

Raw: `evidence/raw/queries3.json`, `evidence/raw/arxiv3.json`

```
ti:"Self-Controlled Memory"      ti:ConflictBank      ti:MemGPT      ti:A-MEM agentic memory
all:memory management survey large language model agents
all:temporal knowledge graph agent memory Zep
all:knowledge conflict benchmark large language models
all:sleep-time compute memory consolidation agent
all:prompt injection attack model context protocol
all:long-term dialogue memory benchmark
all:retrieval augmented generation survey
all:lost in the middle long context
all:cognitive architecture memory SOAR ACT-R
all:truth maintenance system belief revision non-monotonic
all:metacognition large language models self-knowledge
all:abstention knowing what you don't know language models
all:memory operating system for AI agents
all:agentic memory survey personalization
all:knowledge graph memory conversational agent
all:context compression summarization agent memory
all:evaluation framework memory systems agents benchmark
all:probabilistic confidence elicitation verbalized uncertainty
all:tool retrieval MCP server selection agent
all:episodic memory consolidation hippocampal replay model
all:semantic memory personalization assistant long-term
all:analogical reasoning memory generalization agent
all:database transaction consistency memory store agent
all:human-like memory forgetting curve language model
```

### 1.3 Round 3 — the novelty probe itself (10 queries, 48 unique records)

Raw: `evidence/raw/queries4.json`, `evidence/raw/arxiv4.json`. These are the queries that would
have found a prior operation-level protocol if one existed.

```
all:conformance testing protocol agent memory server
all:interoperability standard memory format AI agents
all:comparing memory systems protocol reproducible
all:benchmark suite memory operations write read update delete
all:MCP server evaluation protocol comparative study memory
all:agent memory write cost latency tokens metric
all:memory poisoning defense write path
all:belief revision agent memory contradiction detection
all:memory versioning provenance audit trail agent
all:forgetting retrieval failure metric memory agent
```

### 1.4 Direct identifier verification (not a search)

Raw: `evidence/raw/arxiv_ids.json`, `evidence/raw/dois.json`, `evidence/raw/dois2.json`.
65 requested candidate IDs were resolved one by one through the arXiv `id_list` endpoint and 25
DOIs through `api.crossref.org/works/`. Purpose: to make sure that a known-by-name work is
cited with the identifier that actually exists. One candidate was **rejected** by this step: I
expected `2308.13566` to be the Self-Controlled Memory framework and the API returned a
different paper (`MLLM-DataEngine`), so the citation was dropped and the correct record
(`2304.13343`) was found by a title search instead. This is the failure mode the pipeline exists
to catch.

### 1.5 Web-source probing

Raw: `evidence/raw/urls.json`. 25 URLs fetched with a browser user agent; 23 returned HTTP 200
with a readable title. Two returned HTTP 403 to the plain fetch (W3C PROV-O and the OpenAI
memory announcement) and were verified through a page-text extractor instead, which returned
their content; both are cited with that method noted in the verification table.

Two Crossref *bibliographic* searches (10 queries) were used to attach correct DOIs to classic
cognitive-psychology references, because those titles are frequently cited wrongly:
`Encoding specificity...`, `Levels of processing...`, `Human memory: A proposed system...`,
`Source monitoring.`, `Long short-term memory`, `Retrieval practice...`, `Spacing effect...`,
`Calibrating...`, `Model checking...`, `Calibration of probabilities...`. Two of the returned
top matches were rejected for being a different paper than the intended work, and the surviving
matches were re-verified by direct DOI lookup.

---

## 2. Screening result (what the search did NOT find)

Screened set: 320 unique arXiv records across three rounds, read by title and abstract.

- **Found (closest prior art), 5 works:** `\cite{luo2025mcpbench}` (implementation-agnostic
  evaluation framework for MCP servers, tool-use scope); `\cite{zhou2026agentnative}`
  (four-module evaluation of 12 memory systems, internal module boundaries);
  `\cite{wang2026rightmemory}` (verification of retrieval admissibility — one property);
  `\cite{liu2026pmbench}` (prospective memory benchmark — one operation);
  `\cite{huan2026secondhalf}` (survey with a type-level taxonomy and a review of metrics).
- **Found (adjacent, reinforces the need):** `\cite{afsar2026registry}` (sampling bias in MCP
  ecosystem studies), `\cite{moghadasi2026disclose}` (benchmark papers omit reproducibility
  fields), `\cite{song2026eviction}` (retention failure before retrieval),
  `\cite{mullick2026store}` (store must describe itself), `\cite{souza2025provagent}` (provenance
  for agent workflows).
- **Not found:** a published protocol that (a) defines memory operations by postconditions,
  (b) is runnable against an arbitrary memory server over MCP, and (c) prescribes a
  machine-validated report format covering more than one operation.

**Wording constraint that follows:** the paper may write "to the best of our knowledge, no
published protocol combines operation-level definitions, cross-vendor runnability and a
validated report format for agent memory; our search (queries above, 2026-10-16, 320 records
screened) returned the five closest works listed in Section~2". It may **not** write "the first"
or "no prior work exists" without requalification.

## 3. Limits of this search (stated, not hidden)

1. arXiv only, plus web probing of documentation and repositories. Journals, theses and
   non-indexed conference proceedings were not swept; a true systematic review needs at least
   ACM/IEEE/DBLP coverage.
2. Screening was title-and-abstract level, single screener, no second rater, no κ — so the
   screened set is not a systematic-review grade exclusion process.
3. English-language bias.
4. The field moves fast: the collection date is part of the claim. A publication more than a
   few months after 2026-10-16 requires a re-run and an updated log, and the novelty sentence
   must be requalified with the new date.
5. One round-1 query (`all:agent memory consolidation`) returned a record whose author list was
   later re-fetched; metadata in the snapshot is the id_list version, not the search version,
   precisely to avoid drift.
