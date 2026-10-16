#!/usr/bin/env python3
"""Verify every candidate reference for the paper "Agent Memory in the MCP Ecosystem".

Design rule of this repository: no reference enters the bibliography unless this
script has fetched it from a primary metadata API and the returned record matches
the expected title.  Nothing is typed from memory.

Usage:
    python code/verify_refs.py            # verify all candidates, write data/refs_verified.json
    python code/verify_refs.py --limit 3  # smoke test on the first 3 candidates

Sources of truth:
    * arXiv Atom API   https://export.arxiv.org/api/query   (rate limit ~1 req / 3 s)
    * Crossref REST    https://api.crossref.org/works/<doi>

Raw responses are stored under data/raw/ so a reviewer can audit the fetch.
"""

from __future__ import annotations

import argparse
import html
import json
import os
import re
import sys
import time
import unicodedata
import urllib.error
import urllib.parse
import urllib.request
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RAW = ROOT / "data" / "raw"
OUT = ROOT / "data" / "refs_verified.json"

ARXIV_API = "https://export.arxiv.org/api/query"
CROSSREF_API = "https://api.crossref.org/works/"
ATOM = {"a": "http://www.w3.org/2005/Atom", "arxiv": "http://arxiv.org/schemas/atom"}
UA = "agent-memory-mcp-reference-verifier/1.0 (mailto:parashchuk.k@gmail.com)"

# --------------------------------------------------------------------------
# Candidate list.  ``arxiv`` candidates are searched by title and accepted only
# when every string in ``expect`` occurs in the returned title.
# ``doi`` candidates are resolved through Crossref and accepted only when every
# string in ``expect`` occurs in the registered title.
# --------------------------------------------------------------------------
CANDIDATES: list[dict] = [
    # --- agent memory systems -------------------------------------------------
    {"key": "packer2023memgpt", "src": "arxiv", "q": 'ti:"MemGPT"',
     "expect": ["memgpt"], "section": "systems"},
    {"key": "park2023generative", "src": "arxiv", "q": 'ti:"Generative Agents"',
     "expect": ["generative agents"], "section": "systems"},
    {"key": "shinn2023reflexion", "src": "arxiv", "q": 'ti:"Reflexion" AND abs:"verbal reinforcement"',
     "expect": ["reflexion"], "section": "systems"},
    {"key": "wang2023voyager", "src": "arxiv", "q": 'ti:"Voyager" AND abs:"Minecraft"',
     "expect": ["voyager"], "section": "systems"},
    {"key": "xu2025amem", "src": "arxiv", "q": 'ti:"A-MEM"',
     "expect": ["a-mem"], "section": "systems"},
    {"key": "chhikara2025mem0", "src": "arxiv", "q": 'ti:"Mem0"',
     "expect": ["mem0"], "section": "systems"},
    {"key": "zhong2023memorybank", "src": "arxiv", "q": 'ti:"MemoryBank"',
     "expect": ["memorybank"], "section": "systems"},
    {"key": "gutierrez2024hipporag", "src": "arxiv", "q": 'ti:"HippoRAG"',
     "expect": ["hipporag"], "section": "systems"},
    {"key": "gutierrez2025hipporag2", "src": "arxiv", "q": 'ti:"From RAG to Memory"',
     "expect": ["rag to memory"], "section": "systems"},
    {"key": "sumers2023coala", "src": "arxiv", "q": 'ti:"Cognitive Architectures for Language Agents"',
     "expect": ["cognitive architectures"], "section": "systems"},
    {"key": "wang2024memorysurvey", "src": "arxiv", "q": 'ti:"Survey on the Memory Mechanism of Large Language Model based Agents"',
     "expect": ["memory mechanism"], "section": "surveys"},
    {"key": "wu2025humanmemory", "src": "arxiv", "q": 'ti:"From Human Memory to AI Memory"',
     "expect": ["human memory"], "section": "surveys"},

    # --- retrieval / long context --------------------------------------------
    {"key": "lewis2020rag", "src": "arxiv", "q": 'ti:"Retrieval-Augmented Generation for Knowledge-Intensive NLP Tasks"',
     "expect": ["retrieval-augmented generation"], "section": "retrieval"},
    {"key": "liu2023lostmiddle", "src": "arxiv", "q": 'ti:"Lost in the Middle"',
     "expect": ["lost in the middle"], "section": "retrieval"},
    {"key": "hsieh2024ruler", "src": "arxiv", "q": 'ti:"RULER" AND abs:"long-context"',
     "expect": ["ruler"], "section": "retrieval"},
    {"key": "edge2024graphrag", "src": "arxiv", "q": 'ti:"From Local to Global" AND abs:"query-focused summarization"',
     "expect": ["local to global"], "section": "retrieval"},
    {"key": "wu2022memorizing", "src": "arxiv", "q": 'ti:"Memorizing Transformers"',
     "expect": ["memorizing transformers"], "section": "forgetting"},
    {"key": "lin2025forgetting", "src": "arxiv", "q": 'ti:"Forgetting Transformer"',
     "expect": ["forgetting transformer"], "section": "forgetting"},

    # --- consolidation / offline processing ----------------------------------
    {"key": "snell2025sleeptime", "src": "arxiv", "q": 'ti:"Sleep-time Compute"',
     "expect": ["sleep-time compute"], "section": "consolidation"},
    {"key": "madaan2023selfrefine", "src": "arxiv", "q": 'ti:"Self-Refine"',
     "expect": ["self-refine"], "section": "consolidation"},
    {"key": "mcClelland1995clc", "src": "doi", "doi": "10.1037/0033-295X.102.3.419",
     "expect": ["complementary learning systems"], "section": "consolidation-neuro"},
    {"key": "wilson1994replay", "src": "doi", "doi": "10.1126/science.8036517",
     "expect": ["reactivation of hippocampal"], "section": "consolidation-neuro"},

    # --- epistemic status / belief revision ----------------------------------
    {"key": "kadavath2022know", "src": "arxiv", "q": 'ti:"Language Models (Mostly) Know What They Know"',
     "expect": ["know what they know"], "section": "epistemics"},
    {"key": "lin2022uncertainty", "src": "arxiv", "q": 'ti:"Teaching Models to Express Their Uncertainty in Words"',
     "expect": ["uncertainty in words"], "section": "epistemics"},
    {"key": "farquhar2024semantic", "src": "doi", "doi": "10.1038/s41586-024-07421-0",
     "expect": ["semantic entropy"], "section": "epistemics"},
    {"key": "alchourron1985agm", "src": "doi", "doi": "10.2307/2214737",
     "expect": ["theory change"], "section": "belief-revision"},
    {"key": "doyle1979tms", "src": "doi", "doi": "10.1016/0004-3702(79)90008-0",
     "expect": ["truth maintenance"], "section": "belief-revision"},
    {"key": "reiter1980default", "src": "doi", "doi": "10.1016/0004-3702(80)90014-4",
     "expect": ["default reasoning"], "section": "belief-revision"},
    {"key": "xie2023conflict", "src": "arxiv", "q": 'ti:"Adaptive Chameleon or Stubborn Sloth"',
     "expect": ["chameleon"], "section": "conflict"},
    {"key": "xu2024conflictsurvey", "src": "arxiv", "q": 'ti:"Knowledge Conflicts for LLMs"',
     "expect": ["knowledge conflicts"], "section": "conflict"},
    {"key": "su2024conflictbank", "src": "arxiv", "q": 'ti:"ConflictBank"',
     "expect": ["conflictbank"], "section": "conflict"},

    # --- provenance / documentation ------------------------------------------
    {"key": "gebru2021datasheets", "src": "doi", "doi": "10.1145/3458723",
     "expect": ["datasheets"], "section": "provenance"},
    {"key": "mitchell2019modelcards", "src": "doi", "doi": "10.1145/3287560.3287596",
     "expect": ["model cards"], "section": "provenance"},
    {"key": "erman1980hearsay", "src": "doi", "doi": "10.1145/356810.356816",
     "expect": ["hearsay"], "section": "provenance"},

    # --- MCP ------------------------------------------------------------------
    {"key": "hou2025mcpsurvey", "src": "arxiv", "q": 'ti:"Model Context Protocol" AND abs:"security"',
     "expect": ["model context protocol"], "section": "mcp"},
    {"key": "mcpbench2025", "src": "arxiv", "q": 'ti:"MCP-Bench"',
     "expect": ["mcp-bench"], "section": "mcp"},
    {"key": "mcpeval2025", "src": "arxiv", "q": 'ti:"MCPEval"',
     "expect": ["mcpeval"], "section": "mcp"},
    {"key": "mcpsafetyaudit2025", "src": "arxiv", "q": 'ti:"MCP" AND ti:"Safety Audit"',
     "expect": ["safety audit"], "section": "mcp"},
    {"key": "agentprotocols2025", "src": "arxiv", "q": 'ti:"AI Agent Protocols"',
     "expect": ["agent protocols"], "section": "mcp"},

    # --- security of memory ---------------------------------------------------
    {"key": "zou2024poisonedrag", "src": "arxiv", "q": 'ti:"PoisonedRAG"',
     "expect": ["poisonedrag"], "section": "security"},
    {"key": "chen2024agentpoison", "src": "arxiv", "q": 'ti:"AgentPoison"',
     "expect": ["agentpoison"], "section": "security"},
    {"key": "greshake2023injection", "src": "arxiv", "q": 'ti:"Not what you\'ve signed up for"',
     "expect": ["indirect prompt injection"], "section": "security"},

    # --- evaluation of agents -------------------------------------------------
    {"key": "liu2023agentbench", "src": "arxiv", "q": 'ti:"AgentBench"',
     "expect": ["agentbench"], "section": "evaluation"},
    {"key": "mialon2023gaia", "src": "arxiv", "q": 'ti:"GAIA" AND abs:"General AI Assistants"',
     "expect": ["gaia"], "section": "evaluation"},
    {"key": "yao2024taubench", "src": "arxiv", "q": 'ti:"tau-bench" OR ti:"$\\tau$-bench" OR abs:"tau-bench"',
     "expect": ["bench"], "section": "evaluation"},
    {"key": "zheng2023mtbench", "src": "arxiv", "q": 'ti:"Judging LLM-as-a-Judge"',
     "expect": ["judge"], "section": "evaluation"},
    {"key": "mohammadi2025agentsurvey", "src": "arxiv", "q": 'ti:"Survey on Evaluation of LLM-based Agents"',
     "expect": ["evaluation"], "section": "evaluation"},
    {"key": "wu2024longmemeval", "src": "arxiv", "q": 'ti:"LongMemEval"',
     "expect": ["longmemeval"], "section": "evaluation"},
    {"key": "maharana2024locomo", "src": "arxiv", "q": 'ti:"Evaluating Very Long-Term Conversational Memory"',
     "expect": ["long-term conversational memory"], "section": "evaluation"},
    {"key": "memoryagentbench2025", "src": "arxiv", "q": 'ti:"MemoryAgentBench"',
     "expect": ["memoryagentbench"], "section": "evaluation"},

    # --- multi-agent shared memory -------------------------------------------
    {"key": "hong2023metagpt", "src": "arxiv", "q": 'ti:"MetaGPT"',
     "expect": ["metagpt"], "section": "multiagent"},
    {"key": "wu2023autogen", "src": "arxiv", "q": 'ti:"AutoGen"',
     "expect": ["autogen"], "section": "multiagent"},
    {"key": "yao2022react", "src": "arxiv", "q": 'ti:"ReAct" AND abs:"reasoning" AND abs:"acting"',
     "expect": ["react"], "section": "multiagent"},
]


def fetch(url: str, tries: int = 4, pause: float = 4.0) -> bytes:
    last = None
    for attempt in range(tries):
        req = urllib.request.Request(url, headers={"User-Agent": UA})
        try:
            with urllib.request.urlopen(req, timeout=45) as resp:
                return resp.read()
        except urllib.error.HTTPError as exc:  # retry on 429 / 5xx
            last = exc
            if exc.code in (429, 500, 502, 503) and attempt < tries - 1:
                time.sleep(pause * (attempt + 1))
                continue
            raise
        except Exception as exc:  # noqa: BLE001 - network flakiness
            last = exc
            if attempt < tries - 1:
                time.sleep(pause * (attempt + 1))
                continue
            raise
    raise RuntimeError(f"unreachable: {last}")


def norm(text: str) -> str:
    text = unicodedata.normalize("NFKD", html.unescape(text or ""))
    text = text.replace("$\\tau$", "tau").replace("τ", "tau")
    text = re.sub(r"[^a-z0-9]+", " ", text.lower())
    return re.sub(r"\s+", " ", text).strip()


def fetch_arxiv(cand: dict) -> dict:
    query = urllib.parse.urlencode(
        {"search_query": cand["q"], "max_results": 6, "sortBy": "relevance"}
    )
    raw = fetch(f"{ARXIV_API}?{query}")
    (RAW / f"arxiv_{cand['key']}.xml").write_bytes(raw)
    root = ET.fromstring(raw)
    entries = []
    for entry in root.findall("a:entry", ATOM):
        ident = entry.find("a:id", ATOM).text.strip()
        title = " ".join(entry.find("a:title", ATOM).text.split())
        entries.append(
            {
                "id": ident.split("/abs/")[-1],
                "title": title,
                "authors": [a.find("a:name", ATOM).text for a in entry.findall("a:author", ATOM)],
                "published": entry.find("a:published", ATOM).text[:10],
                "updated": entry.find("a:updated", ATOM).text[:10],
                "summary": " ".join(entry.find("a:summary", ATOM).text.split()),
                "url": ident,
                "pdf": ident.replace("/abs/", "/pdf/"),
                "primary_category": (
                    entry.find("arxiv:primary_category", ATOM).get("term")
                    if entry.find("arxiv:primary_category", ATOM) is not None
                    else None
                ),
                "comment": (
                    " ".join(entry.find("arxiv:comment", ATOM).text.split())
                    if entry.find("arxiv:comment", ATOM) is not None
                    else None
                ),
                "journal_ref": (
                    " ".join(entry.find("arxiv:journal_ref", ATOM).text.split())
                    if entry.find("arxiv:journal_ref", ATOM) is not None
                    else None
                ),
                "doi": (
                    entry.find("arxiv:doi", ATOM).text.strip()
                    if entry.find("arxiv:doi", ATOM) is not None
                    else None
                ),
            }
        )
    return {"source": "arxiv", "query": cand["q"], "n_returned": len(entries), "entries": entries}


def pick_arxiv(cand: dict, payload: dict) -> dict | None:
    wanted = [norm(e) for e in cand["expect"]]
    for entry in payload["entries"]:
        t = norm(entry["title"])
        if all(w in t for w in wanted):
            return entry
    return None


def fetch_crossref(cand: dict) -> dict:
    raw = fetch(CROSSREF_API + urllib.parse.quote(cand["doi"]))
    (RAW / f"crossref_{cand['key']}.json").write_bytes(raw)
    message = json.loads(raw)["message"]
    return {"source": "crossref", "doi": cand["doi"], "message": message}


def pick_crossref(cand: dict, payload: dict) -> dict | None:
    msg = payload["message"]
    titles = msg.get("title") or []
    title = " ".join(titles[0].split()) if titles else ""
    wanted = [norm(e) for e in cand["expect"]]
    if not all(w in norm(title) for w in wanted):
        return None
    year = None
    for field in ("published-print", "published-online", "issued", "created"):
        parts = msg.get(field, {}).get("date-parts") if msg.get(field) else None
        if parts and parts[0] and parts[0][0]:
            year = str(parts[0][0])
            break
    return {
        "title": title,
        "authors": [
            " ".join(filter(None, [a.get("given"), a.get("family")])) or a.get("name", "")
            for a in msg.get("author", [])
        ],
        "year": year,
        "container": (msg.get("container-title") or [None])[0],
        "volume": msg.get("volume"),
        "issue": msg.get("issue"),
        "page": msg.get("page"),
        "type": msg.get("type"),
        "publisher": msg.get("publisher"),
        "url": msg.get("URL") or f"https://doi.org/{cand['doi']}",
        "doi": cand["doi"],
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--limit", type=int, default=0)
    parser.add_argument("--out", default=str(OUT))
    args = parser.parse_args()

    RAW.mkdir(parents=True, exist_ok=True)
    cands = CANDIDATES[: args.limit] if args.limit else CANDIDATES
    verified, failed = [], []

    for i, cand in enumerate(cands, 1):
        try:
            if cand["src"] == "arxiv":
                payload = fetch_arxiv(cand)
                hit = pick_arxiv(cand, payload)
                if hit is None:
                    failed.append({"key": cand["key"], "why": "no title match",
                                   "got": [e["title"] for e in payload["entries"][:3]],
                                   "query": cand["q"]})
                    print(f"[{i}/{len(cands)}] FAIL {cand['key']}: no match")
                else:
                    record = {"key": cand["key"], "section": cand["section"],
                              "src": "arxiv", **hit}
                    verified.append(record)
                    print(f"[{i}/{len(cands)}] ok   {cand['key']}  arXiv:{hit['id']}  {hit['title'][:70]}")
                time.sleep(3.2)  # arXiv asks for >= 3 s between requests
            else:
                payload = fetch_crossref(cand)
                hit = pick_crossref(cand, payload)
                if hit is None:
                    failed.append({"key": cand["key"], "why": "no title match",
                                   "got": payload["message"].get("title"), "doi": cand["doi"]})
                    print(f"[{i}/{len(cands)}] FAIL {cand['key']}: doi title mismatch")
                else:
                    record = {"key": cand["key"], "section": cand["section"],
                              "src": "doi", **hit}
                    verified.append(record)
                    print(f"[{i}/{len(cands)}] ok   {cand['key']}  DOI:{cand['doi']}  {hit['title'][:70]}")
                time.sleep(1.0)
        except Exception as exc:  # noqa: BLE001
            failed.append({"key": cand["key"], "why": f"error: {exc}"})
            print(f"[{i}/{len(cands)}] ERR  {cand['key']}: {exc}")
            time.sleep(2.0)

    out = {
        "generated_by": "code/verify_refs.py",
        "generated_at_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "n_verified": len(verified),
        "n_failed": len(failed),
        "verified": verified,
        "failed": failed,
    }
    Path(args.out).write_text(json.dumps(out, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"\nverified={len(verified)} failed={len(failed)} -> {args.out}")
    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main())
