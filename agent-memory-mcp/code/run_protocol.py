"""Run the agent-memory evaluation protocol (protocol.md) against one MCP server.

The runner is the measuring instrument, not a subject: it generates the task streams
(`code/streams.py`) from a seed, drives the server under test over MCP stdio (JSON-RPC),
writes every raw request/response to `reports/<run>/raw/*.jsonl`, computes the metrics of
protocol.md section 3 programmatically from those raw responses, and emits
`reports/<run>/report.json` in the shape of `code/protocol_schema.json`.  It then validates
its own output with `refslint report` — the package's Rust validator in `tools/refslint`, spawned
as a subprocess and obtained through `tools/refslint.sh` (which builds it with cargo when it is
missing) — and exits non-zero if the report is invalid.

Gold labels come from the generator, never from a model.  No LLM judge is used anywhere in
this file: every number below is computed from MCP responses or from the harness's own write
log (the latter only for the "what should the store contain" side of monitoring accuracy).

Usage:
    python code/run_protocol.py --list-servers
    python code/run_protocol.py --server sint-liquid-mcp --server-root ../../refs \
        --server-python <python-with-mcp-1.9.4> --server-pythonpath <site-dir>
    python code/run_protocol.py --server sint-memory-mcp
    tools/refslint/target/release/refslint report --schema code/protocol_schema.json \
        reports/*/report.json

Exit codes: 0 = report written and valid; 1 = report written but rejected by `refslint report`;
2 = run failed (harness-side faults above the suspicion threshold, or no validator obtainable).
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import platform
import queue
import random
import shutil
import subprocess
import sys
import threading
import time

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
sys.path.insert(0, HERE)

import streams as streams_mod  # noqa: E402  (same directory)

FAMILY_ORDER = ["T1", "T2", "T3", "T4", "T5", "T6", "T7", "T8", "T9", "T10"]
SERVERS_FILE = os.path.join(HERE, "servers.json")
N_RESAMPLES = 1000
CI_QUANTILES = (0.025, 0.975)

# protocol.md section 2: which task families feed which taxonomy operation, per metric name.
OPERATION_MAP = {
    "O1_WRITE": ("T1,T5,T9", ["write_precision", "write_recall", "duplicate_rate",
                              "write_latency_p50_ms", "write_latency_p95_ms", "tokens_per_write",
                              "cost_per_1000_writes_usd", "idempotency_violations"]),
    "O2_READ": ("T2,T3,T4", ["recall_at_5", "mrr", "required_record_hit_rate",
                             "provenance_correctness", "admissibility_error_rate",
                             "context_inflation", "read_latency_p50_ms", "read_latency_p95_ms"]),
    "O3_CONSOLIDATE": ("T6", ["compression_ratio", "probe_survival_rate",
                              "derived_from_completeness", "cost_per_consolidation_usd"]),
    "O4_REVISE": ("T5", ["contradiction_detection_f1", "revision_correctness",
                         "chain_completeness", "stale_read_rate", "collateral_change_rate"]),
    "O5_INTEND": ("T7", ["cue_hit_rate", "false_fire_rate", "fire_latency_calls",
                         "context_availability_at_fire"]),
    "O6_ASSERT": ("T8", ["abstention_correctness", "in_coverage_answer_correctness",
                         "key_question_correctness", "subject_question_correctness",
                         "ece", "discrimination_auroc", "monitoring_accuracy"]),
    "PROVENANCE": ("T1,T2,T5,T6", ["provenance_correctness", "derived_from_completeness"]),
    "LIFECYCLE": ("T3,T4,T5,T9", ["lifecycle_state_exposed", "admissibility_error_rate",
                                  "stale_read_rate", "restart_survival", "snapshot_reopen_survival"]),
}


# --------------------------------------------------------------------------------------
# small statistics helpers (stdlib only, no LLM judge anywhere)
# --------------------------------------------------------------------------------------

def mean(xs):
    return sum(xs) / len(xs) if xs else None


def percentile(xs, q):
    if not xs:
        return None
    s = sorted(xs)
    if len(s) == 1:
        return s[0]
    pos = q * (len(s) - 1)
    lo = int(math.floor(pos))
    hi = int(math.ceil(pos))
    if lo == hi:
        return s[lo]
    return s[lo] + (s[hi] - s[lo]) * (pos - lo)


def bootstrap_ci(xs, rng, n_resamples=N_RESAMPLES):
    """Percentile bootstrap over the repetition axis (protocol.md section 3)."""
    if not xs:
        return None, None
    if len(xs) == 1:
        return xs[0], xs[0]
    stats = []
    n = len(xs)
    for _ in range(n_resamples):
        sample = [xs[rng.randrange(n)] for _ in range(n)]
        stats.append(sum(sample) / n)
    stats.sort()
    lo = stats[int(math.floor(CI_QUANTILES[0] * (len(stats) - 1)))]
    hi = stats[int(math.ceil(CI_QUANTILES[1] * (len(stats) - 1)))]
    return lo, hi


def ratio(num, den):
    return (num / den) if den else None


def auroc(scores, labels):
    """AUROC with average ranks for ties; None when one class is empty."""
    pos = [s for s, l in zip(scores, labels) if l]
    neg = [s for s, l in zip(scores, labels) if not l]
    if not pos or not neg:
        return None
    order = sorted(range(len(scores)), key=lambda i: scores[i])
    ranks = [0.0] * len(scores)
    i = 0
    while i < len(order):
        j = i
        while j + 1 < len(order) and scores[order[j + 1]] == scores[order[i]]:
            j += 1
        avg = (i + j) / 2.0 + 1.0
        for k in range(i, j + 1):
            ranks[order[k]] = avg
        i = j + 1
    rank_sum_pos = sum(ranks[i] for i in range(len(scores)) if labels[i])
    n_pos, n_neg = len(pos), len(neg)
    return (rank_sum_pos - n_pos * (n_pos + 1) / 2.0) / (n_pos * n_neg)


def ece(confs, corrects, bins=10):
    """Expected calibration error, 10 equal-width bins over reported confidence."""
    if not confs:
        return None
    total = len(confs)
    acc_err = 0.0
    for b in range(bins):
        lo, hi = b / bins, (b + 1) / bins
        idx = [i for i, c in enumerate(confs) if (lo <= c < hi) or (b == bins - 1 and c == 1.0)]
        if not idx:
            continue
        acc = sum(1.0 for i in idx if corrects[i]) / len(idx)
        conf = sum(confs[i] for i in idx) / len(idx)
        acc_err += (len(idx) / total) * abs(acc - conf)
    return acc_err


def metric(name, unit, value, computer="harness", notes="", kappa=None):
    out = {"name": name, "unit": unit, "value": None if value is None else round(float(value), 6),
           "computer": computer, "notes": notes}
    if kappa is not None or computer == "rubric_two_graders":
        out["kappa"] = None if kappa is None else round(float(kappa), 6)
    return out


# --------------------------------------------------------------------------------------
# MCP stdio client (JSON-RPC 2.0, newline-delimited, no third-party imports)
# --------------------------------------------------------------------------------------

class MCPProcess:
    """One server process, addressed over stdin/stdout with newline-delimited JSON-RPC."""

    def __init__(self, name, argv, cwd, env, client_info, protocol_version="2025-06-18",
                 timeout=30.0, log=None):
        self.name = name
        self.argv = list(argv)
        self.cwd = cwd
        self.env = dict(env)
        self.client_info = client_info
        self.protocol_version = protocol_version
        self.timeout = timeout
        self.log = log
        self.proc = None
        self.q = None
        self._id = 0
        self.init_result = None
        self.stderr_tail = []

    # -- lifecycle
    def start(self):
        env = dict(os.environ)
        env.update(self.env)
        env["PYTHONIOENCODING"] = "utf-8"
        env["PYTHONUNBUFFERED"] = "1"
        self.proc = subprocess.Popen(
            self.argv, cwd=self.cwd, env=env,
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            text=True, encoding="utf-8", errors="replace", bufsize=1)
        self.q = queue.Queue()
        self._id = 0
        threading.Thread(target=self._reader, args=(self.proc.stdout, "out"), daemon=True).start()
        threading.Thread(target=self._reader, args=(self.proc.stderr, "err"), daemon=True).start()
        self.init_result = self.handshake()
        self._event("spawn", {"argv": self.argv, "cwd": self.cwd,
                              "pid": self.proc.pid,
                              "server_info": (self.init_result or {}).get("serverInfo")})
        return self.init_result

    def _reader(self, stream, kind):
        try:
            for line in stream:
                self.q.put((kind, line.rstrip("\n")))
        finally:
            self.q.put((kind, None))

    def _event(self, kind, payload):
        if self.log is not None:
            self.log.event({"event": kind, "server": self.name, **payload})

    def handshake(self):
        resp = self.request("initialize", {
            "protocolVersion": self.protocol_version,
            "capabilities": {},
            "clientInfo": self.client_info,
        })
        result = (resp or {}).get("result") or {}
        self.notify("notifications/initialized")
        return result

    def stop(self):
        if self.proc is None:
            return
        try:
            self.proc.stdin.close()
        except Exception:
            pass
        try:
            self.proc.wait(timeout=5)
        except Exception:
            try:
                self.proc.kill()
            except Exception:
                pass
        self._event("exit", {"pid": self.proc.pid, "returncode": self.proc.returncode,
                             "stderr_tail": self.stderr_tail[-3:]})
        self.proc = None

    def restart(self):
        self.stop()
        return self.start()

    # -- messaging
    def send(self, msg):
        self.proc.stdin.write(json.dumps(msg) + "\n")
        self.proc.stdin.flush()

    def notify(self, method, params=None):
        self.send({"jsonrpc": "2.0", "method": method, "params": params or {}})

    def request(self, method, params=None, timeout=None):
        self._id += 1
        rid = self._id
        self.send({"jsonrpc": "2.0", "id": rid, "method": method, "params": params or {}})
        deadline = time.perf_counter() + (timeout or self.timeout)
        while True:
            remaining = deadline - time.perf_counter()
            if remaining <= 0:
                raise TimeoutError("%s timed out after %.1fs" % (method, timeout or self.timeout))
            try:
                kind, line = self.q.get(timeout=remaining)
            except queue.Empty:
                raise TimeoutError("%s timed out after %.1fs" % (method, timeout or self.timeout))
            if line is None:
                if kind == "err":
                    continue
                raise RuntimeError("server closed stdout while waiting for %s" % method)
            if kind == "err":
                if line.strip():
                    self.stderr_tail.append(line.strip()[:400])
                    del self.stderr_tail[:-20]
                continue
            if not line.strip():
                continue
            try:
                msg = json.loads(line)
            except ValueError:
                # server logging on stdout: recorded, not fatal
                self._event("stdout_noise", {"line": line[:200]})
                continue
            if isinstance(msg, dict) and msg.get("id") == rid:
                return msg


class CallLog:
    """Append-only JSONL record of everything the harness sent and received."""

    def __init__(self, path):
        self.path = path
        os.makedirs(os.path.dirname(path), exist_ok=True)
        self.fh = open(path, "w", encoding="utf-8")

    def write(self, obj):
        self.fh.write(json.dumps(obj, ensure_ascii=False) + "\n")
        self.fh.flush()

    def event(self, obj):
        self.write({"ts": round(time.time(), 3), "kind": "event", **obj})

    def close(self):
        try:
            self.fh.close()
        except Exception:
            pass


# --------------------------------------------------------------------------------------
# adapters: map the protocol's operations onto each server's tool surface
# --------------------------------------------------------------------------------------

class OpResult:
    __slots__ = ("tool", "args", "request", "response", "latency_ms", "error_code",
                 "payload", "text_chars", "records")

    def __init__(self, tool, args):
        self.tool = tool
        self.args = args
        self.request = None
        self.response = None
        self.latency_ms = None
        self.error_code = None
        self.payload = None
        self.text_chars = 0
        self.records = []

    @property
    def ok(self):
        return self.error_code is None

    def values(self):
        return [r.get("value") for r in self.records]


class Adapter:
    """Common plumbing; subclasses implement the operations of protocol.md section 2."""

    name = "base"
    record_content_field = "content"
    record_source_field = "source"
    record_time_field = "timestamp"
    supports_revise = False
    supports_maintenance = False
    supports_ranked_search = False

    def __init__(self, session, tools, rep):
        self.session = session
        self.tool_specs = list(tools)
        self.tools = frozenset(t.get("name") for t in self.tool_specs)
        self.rep = rep
        self.log = None
        self.notes = []

    # -- capability plumbing
    def has(self, tool):
        return tool in self.tools

    def call(self, tool, args, timeout=None):
        res = OpResult(tool, args)
        if not self.has(tool):
            res.error_code = "not_implemented"
            self.rep.failure("not_implemented", "tool %s absent from tools/list" % tool)
            self._log(res)
            return res
        if self.rep.calls >= self.rep.max_calls:
            res.error_code = "budget_exhausted"
            self.rep.failure("budget_exhausted", "max_calls=%d reached" % self.rep.max_calls)
            self._log(res)
            return res
        self.rep.calls += 1
        res.request = {"method": "tools/call", "params": {"name": tool, "arguments": args}}
        t0 = time.perf_counter()
        try:
            res.response = self.session.proc.request("tools/call", res.request["params"],
                                                     timeout=timeout or self.rep.timeout)
        except TimeoutError as exc:
            res.error_code = "timeout"
            res.response = {"harness_error": str(exc)}
        except Exception as exc:  # server died, malformed framing, ...
            res.error_code = "protocol_error"
            res.response = {"harness_error": "%s: %s" % (type(exc).__name__, exc)}
        res.latency_ms = round((time.perf_counter() - t0) * 1000.0, 3)
        if res.response and "error" in res.response:
            res.error_code = "server_error"
        if res.response:
            result = res.response.get("result")
            if isinstance(result, dict) and result.get("isError"):
                res.error_code = "server_error"
        res.payload = self.extract_payload(res.response)
        res.text_chars = self.payload_chars(res.response)
        res.records = self.as_records(res.payload)
        if res.error_code in ("server_error", "timeout", "protocol_error"):
            self.rep.failure(res.error_code, "%s(%s) -> %s" % (tool, json.dumps(args)[:120],
                                                               json.dumps(res.response)[:200]))
        self._log(res)
        return res

    def _log(self, res):
        if self.log is None:
            return
        self.log.write({
            "ts": round(time.time(), 3), "kind": "call", "server": self.name,
            "family": self.rep.family, "repetition": self.rep.seed,
            "tool": res.tool, "arguments": res.args,
            "latency_ms": res.latency_ms, "error_code": res.error_code,
            "request": res.request, "response": res.response,
            "parsed_records": len(res.records),
        })

    @staticmethod
    def extract_payload(response):
        """Parse the tool result.

        FastMCP sends one `content` item per record when a tool returns a list, so a
        multi-record answer arrives as several JSON texts, not as one JSON array. Reading
        only the first item silently truncates every ranked result to one record.
        """
        if not isinstance(response, dict):
            return None
        result = response.get("result")
        if not isinstance(result, dict):
            return None
        texts = [c.get("text", "") for c in result.get("content", []) if c.get("type") == "text"]
        if not texts:
            return None
        parsed = []
        for raw in texts:
            try:
                parsed.append(json.loads(raw))
            except (ValueError, TypeError):
                parsed.append(raw)
        return parsed[0] if len(parsed) == 1 else parsed

    @staticmethod
    def payload_chars(response):
        if not isinstance(response, dict):
            return 0
        result = response.get("result")
        if not isinstance(result, dict):
            return 0
        return sum(len(c.get("text", "")) for c in result.get("content", []))

    def as_records(self, payload):
        """Normalise a tool payload into a list of record dicts."""
        if isinstance(payload, list):
            return [p for p in payload if isinstance(p, dict) and "error" not in p]
        if isinstance(payload, dict):
            if "error" in payload:
                return []
            return [payload]
        return []

    def content_of(self, record):
        return record.get(self.record_content_field)

    def read_probe(self, content, k=5):
        """Stateless read used by the durability probes.

        It goes through the server's search path, so a store re-opened by a second process -
        which never saw the writes and has no id map - can still be probed by content alone.
        """
        return self.search(content, k)

    # -- store-side ground truth used only for monitoring accuracy
    def expected_store(self):
        return {}

    def server_store(self):
        return {}

    def monitoring(self):
        exp, got = self.expected_store(), self.server_store()
        if not exp or not got:
            return None, "no store self-report available"
        compared = matched = 0
        for key, val in exp.items():
            if key not in got:
                continue
            compared += 1
            if got[key] == val:
                matched += 1
        return ratio(matched, compared), "compared %d self-reported counters" % compared


class NotSupported(Exception):
    pass


class LiquidAdapter(Adapter):
    """sint-liquid-mcp: belief store keyed by claim text, explicit revision and contradictions."""

    name = "sint-liquid-mcp"
    record_content_field = "claim"
    record_source_field = "source"
    record_time_field = "created"
    supports_revise = True
    supports_maintenance = False
    supports_ranked_search = False

    def __init__(self, session, tools, rep):
        super().__init__(session, tools, rep)
        self._status_by_content = {}
        self._id_by_content = {}
        self._revisions = 0

    def write(self, fact):
        res = self.call("liquid_belief", {
            "claim": fact["content"], "status": fact["status"], "confidence": fact["confidence"],
            "source": fact["source"], "domain": fact["tags"][0] if fact["tags"] else "protocol",
            "tags": list(fact["tags"]),
        })
        self._status_by_content[fact["content"]] = fact["status"]
        if res.records:
            self._id_by_content[fact["content"]] = res.records[0].get("id")
        return res

    def read_content(self, content, k=1):
        return self.call("liquid_query", {"claim_query": content})

    def search(self, query, k=5):
        # The only read operation in this server is an exact-match lookup; `k` is not
        # expressible, so the harness passes the query through unchanged.
        return self.call("liquid_query", {"claim_query": query})

    def stats(self):
        return self.call("liquid_stats", {})

    def revise(self, content, new_status, new_confidence, reason):
        cid = self._id_by_content.get(content)
        if cid is None:
            res = OpResult("liquid_revise", {})
            res.error_code = "not_implemented"
            self.rep.failure("not_implemented", "liquid_revise needs a claim id that the store never returned")
            return res
        res = self.call("liquid_revise", {"claim_id": cid, "new_status": new_status,
                                          "new_confidence": new_confidence, "reason": reason})
        self._status_by_content[content] = new_status
        self._revisions += 1
        return res

    def history(self, content):
        cid = self._id_by_content.get(content)
        if cid is None:
            res = OpResult("liquid_history", {})
            res.error_code = "not_implemented"
            self.rep.failure("not_implemented", "liquid_history needs a claim id that the store never returned")
            return res
        return self.call("liquid_history", {"claim_id": cid})

    def anomalies(self):
        return self.call("liquid_stats", {})

    def maintenance(self):
        res = OpResult("liquid_maintenance", {})
        res.error_code = "not_implemented"
        self.rep.failure("not_implemented", "no consolidation or maintenance operation in the tool surface")
        return res

    def expected_store(self):
        by_status = {}
        for status in self._status_by_content.values():
            by_status[status] = by_status.get(status, 0) + 1
        return {"total_beliefs": len(self._status_by_content), "by_status": by_status}

    def server_store(self):
        res = self.call("liquid_stats", {})
        if not isinstance(res.payload, dict):
            return {}
        return {"total_beliefs": res.payload.get("total_beliefs"),
                "by_status": res.payload.get("by_status")}


class SintMemoryAdapter(Adapter):
    """sint-memory-mcp: append-only hash-chained blocks with registers and provenance."""

    name = "sint-memory-mcp"
    record_content_field = "content"
    record_source_field = "source"
    record_time_field = "timestamp"
    supports_revise = False
    supports_maintenance = True
    supports_ranked_search = True

    def __init__(self, session, tools, rep):
        super().__init__(session, tools, rep)
        self._ids = []
        self._register_counts = {}
        self._blocks = {}            # block id -> last known content, for restart comparisons
        self._id_by_content = {}

    def write(self, fact):
        res = self.call("memory_write_block", {
            "register": fact["register"], "content": fact["content"],
            "project": fact["project"], "source": fact["source"],
            "confidence": fact["confidence"], "tags": list(fact["tags"]), "links": [],
        })
        payload = res.payload
        if isinstance(payload, list):
            payload = payload[0] if payload else None
        bid = payload.get("id") if isinstance(payload, dict) else None
        if bid is not None:
            self._ids.append(bid)
            self._id_by_content[fact["content"]] = bid
        self._register_counts[fact["register"]] = self._register_counts.get(fact["register"], 0) + 1
        return res

    def read_content(self, content, k=1):
        bid = self._id_by_content.get(content)
        if bid is None:
            res = OpResult("memory_read_block", {})
            res.error_code = "not_implemented"
            self.rep.failure("not_implemented",
                             "memory_read_block needs a block id, and this content was never written")
            return res
        return self.call("memory_read_block", {"block_id": bid})

    def search(self, query, k=5):
        return self.call("memory_search", {"query": query, "limit": k})

    def stats(self):
        return self.call("memory_stats", {})

    def revise(self, content, new_status, new_confidence, reason):
        res = OpResult("memory_revise", {})
        res.error_code = "not_implemented"
        self.rep.failure("not_implemented",
                         "the block store is append-only: no operation rewrites or retires a block")
        return res

    def history(self, content):
        bid = self._id_by_content.get(content)
        if bid is None:
            res = OpResult("memory_trace_provenance", {})
            res.error_code = "not_implemented"
            self.rep.failure("not_implemented", "no block id for this content")
            return res
        return self.call("memory_trace_provenance", {"block_id": bid})

    def anomalies(self):
        return self.call("memory_verify_chain", {})

    def maintenance(self):
        return self.call("memory_apply_decay", {})

    def expected_store(self):
        return {"total_blocks": len(self._ids), "blocks_by_register": dict(self._register_counts)}

    def server_store(self):
        res = self.call("memory_stats", {})
        if not isinstance(res.payload, dict):
            return {}
        return {"total_blocks": res.payload.get("total_blocks"),
                "blocks_by_register": res.payload.get("blocks_by_register")}


ADAPTERS = {"liquid": LiquidAdapter, "sint_memory": SintMemoryAdapter}


# --------------------------------------------------------------------------------------
# one repetition: drives every family against one server process
# --------------------------------------------------------------------------------------

class Repetition:
    def __init__(self, adapter, log, seed, timeout, max_calls, wall_clock_s,
                 data_dir=None, data_dir_root=None):
        self.adapter = adapter
        self.log = log
        self.seed = seed
        self.timeout = timeout
        self.max_calls = max_calls
        self.wall_clock_s = wall_clock_s
        self.data_dir = data_dir
        self.data_dir_root = data_dir_root
        self.calls = 0
        self.family = None
        self.failures = {}
        self.latencies = {}
        self.started = time.perf_counter()

    def select(self, family):
        self.family = family
        self.adapter.family = family
        self.adapter.log = self.log

    def failure(self, code, example):
        bucket = self.failures.setdefault(self.family, {})
        entry = bucket.setdefault(code, {"count": 0, "examples": []})
        entry["count"] += 1
        if len(entry["examples"]) < 5:
            entry["examples"].append(example[:300])

    def timeit(self, key, value_ms):
        self.latencies.setdefault((self.family, key), []).append(value_ms)


def _ms_list(rep, family, key):
    return rep.latencies.get((family, key), [])


# --------------------------------------------------------------------------------------
# family runners: each returns a {metric_name: metric_dict} map for one repetition
# --------------------------------------------------------------------------------------

def run_t1(rep, st, a):
    facts = st["facts"]
    before = a.stats()
    stored_before = _store_total(before.payload)
    write_latencies = []
    for f in facts:
        res = a.write(f)
        write_latencies.append(res.latency_ms)
    for f in facts:
        if f["key"] in st["retry_keys"]:
            res = a.write(f)          # identical content, identical intent: a retry
            write_latencies.append(res.latency_ms)
    stats = a.stats()
    stored_total = _store_total(stats.payload)
    stored = None
    if stored_total is not None and stored_before is not None:
        stored = stored_total - stored_before

    correct = 0
    for f in facts:
        res = a.read_content(f["content"])
        if res.records:
            r = res.records[0]
            got_content = a.content_of(r)
            got_tags = r.get("tags")
            fields_ok = (
                got_content == f["content"]
                and r.get("source") == f["source"]
                and abs(float(r.get("confidence", -1)) - f["confidence"]) < 1e-9
                and (got_tags is None or set(got_tags) >= set(f["tags"]))
            )
            if fields_ok:
                correct += 1
    dup = (stored - len(facts)) if isinstance(stored, int) else None
    metrics = {
        "write_precision": metric("write_precision", "ratio", ratio(correct, stored),
                                  notes="correct_written_records / written_records. A stored record counts as correct when it matches a generated fact and is not an extra copy of one; written_records is the store's own record count after the writes minus its count before them, so the duplication is charged here and reported separately as duplicate_rate"),
        "write_recall": metric("write_recall", "ratio", ratio(correct, len(facts)),
                               notes="correct_written_records / generated_facts"),
        "duplicate_rate": metric("duplicate_rate", "ratio", ratio(dup, stored) if dup is not None else None,
                                 notes="extra_records_from_retries / written_records; a retry is the same content with the same intent"),
        "idempotency_violations": metric("idempotency_violations", "count",
                                         max(0, dup) if dup is not None else None,
                                         computer="programmatic_check",
                                         notes="retries that created an extra record (protocol.md section 3)"),
        "write_latency_p50_ms": metric("write_latency_p50_ms", "ms", percentile(write_latencies, 0.5),
                                       notes="wall clock around tools/call"),
        "write_latency_p95_ms": metric("write_latency_p95_ms", "ms", percentile(write_latencies, 0.95)),
        "tokens_per_write": metric("tokens_per_write", "tokens", None,
                                   notes="null: no model is in the loop, so no token accounting exists; payload sizes are in the raw log"),
        "cost_per_1000_writes_usd": metric("cost_per_1000_writes_usd", "usd", None,
                                           notes="null: store-only run, no per-token price applies (see budgets.price_table)"),
    }
    rep.timeit("write", mean(write_latencies))
    return metrics


def run_t2(rep, st, a):
    for f in st["corpus"]:
        a.write(f)
    content2key = {f["content"]: f["key"] for f in st["corpus"]}
    gold_source = {f["content"]: f["source"] for f in st["corpus"]}
    latencies = []
    hits_at_k, hits_any, rrs, prov_ok, prov_tot, inflation = [], [], [], 0, 0, []
    by_kind = {"key": [], "topic": []}
    for q in st["queries"]:
        res = a.search(q["query"], q["k"])
        latencies.append(res.latency_ms)
        returned = [a.content_of(r) for r in res.records]
        top_k = returned[:q["k"]]
        rank = None
        for i, c in enumerate(returned, start=1):
            if c == q["required_content"]:
                rank = i
                break
        hit = 1.0 if q["required_content"] in top_k else 0.0
        hits_at_k.append(hit)
        hits_any.append(1.0 if rank is not None else 0.0)
        by_kind[q["kind"]].append(hit)
        rrs.append(1.0 / rank if rank else 0.0)
        for rec, c in zip(res.records, returned):
            prov_tot += 1
            if gold_source.get(c) == rec.get(a.record_source_field):
                prov_ok += 1
        req_chars = len(q["required_content"]) or 1
        got_chars = sum(len(c or "") for c in returned)
        inflation.append(got_chars / req_chars)
    metrics = {
        "recall_at_5": metric("recall_at_5", "ratio", mean(hits_at_k), notes="required record inside the top k returned"),
        "required_record_hit_rate": metric("required_record_hit_rate", "ratio", mean(hits_any),
                                           notes="required record anywhere in the returned list, ignoring k"),
        "mrr": metric("mrr", "ratio", mean(rrs)),
        "key_query_hit_rate": metric("key_query_hit_rate", "ratio", mean(by_kind["key"]),
                                     notes="queries that repeat the record's content verbatim"),
        "topic_query_hit_rate": metric("topic_query_hit_rate", "ratio", mean(by_kind["topic"]),
                                       notes="queries that name only a shared topic token, so several records match"),
        "provenance_correctness": metric("provenance_correctness", "ratio", ratio(prov_ok, prov_tot),
                                         notes="returned records whose source field matches the generated record"),
        "context_inflation": metric("context_inflation", "ratio", mean(inflation),
                                    notes="characters returned / characters of the records actually required (character ratio, not tokens: no tokenizer in the loop)"),
        "read_latency_p50_ms": metric("read_latency_p50_ms", "ms", percentile(latencies, 0.5)),
        "read_latency_p95_ms": metric("read_latency_p95_ms", "ms", percentile(latencies, 0.95)),
    }
    return metrics


def run_t3(rep, st, a):
    for f in st["facts"]:
        a.write(f)
    exposed = 0
    for f in st["facts"]:
        res = a.read_content(f["content"])
        if res.records and res.records[0].get(a.record_time_field):
            exposed += 1
    for p in st["probes"]:
        rep.failure("not_implemented",
                    "probe 'as of %s' for %s: no tool accepts an as-of date" % (p["as_of"], p["subject"]))
    metrics = {
        "temporal_correctness": metric("temporal_correctness", "ratio", None,
                                       notes="null / not_implemented: the server exposes no date-scoped read, so the as-of probe cannot be answered"),
        "temporal_metadata_exposed": metric("temporal_metadata_exposed", "ratio",
                                            ratio(exposed, len(st["facts"])), computer="programmatic_check",
                                            notes="fraction of read-back records carrying a timestamp field (metadata exists, the query does not)"),
    }
    return metrics


def run_t4(rep, st, a):
    for r in st["records"]:
        a.write(r)
    returned_total = inadmissible = 0
    lifecycle_exposed = 0
    for p in st["probes"]:
        res = a.search(p["query"], 5)
        for r in res.records:
            returned_total += 1
            if p["inadmissible_for_caller"]:
                inadmissible += 1
            if r.get("valid_until") or r.get("expired") or r.get("superseded") or r.get("status") == "SUPERSEDED":
                lifecycle_exposed += 1
    scoping = _principal_scoped_fraction(a.tool_specs)
    metrics = {
        "admissibility_error_rate": metric("admissibility_error_rate", "ratio",
                                           ratio(inadmissible, returned_total),
                                           computer="programmatic_check",
                                           notes="returned records that belong to a different principal than the caller; the tool surface has no caller parameter, so this is a policy check over the responses"),
        "lifecycle_state_exposed": metric("lifecycle_state_exposed", "ratio",
                                          ratio(lifecycle_exposed, returned_total),
                                          computer="programmatic_check",
                                          notes="returned records exposing a lifecycle field (expiry/supersede marker)"),
        "principal_scoping_exposed": metric("principal_scoping_exposed", "ratio", scoping,
                                            computer="programmatic_check",
                                            notes="fraction of tools whose input schema accepts a caller/principal argument; read from the tools/list inputSchema of this run"),
    }
    return metrics


def _principal_scoped_fraction(tool_specs):
    """Fraction of advertised tools whose input schema can express who is asking."""
    if not tool_specs:
        return None
    callers = {"caller", "principal", "user", "user_id", "actor", "identity", "subject", "scope"}
    scoped = 0
    for t in tool_specs:
        schema = (t.get("inputSchema") or {}).get("properties") or {}
        if callers & {k.lower() for k in schema}:
            scoped += 1
    return scoped / len(tool_specs)


def run_t5(rep, st, a):
    for f in st["wave"]:
        a.write(f)
    for f in st["unrelated"]:
        a.write(f)
    unrelated_hash_before = {}
    for f in st["unrelated"]:
        res = a.read_content(f["content"])
        unrelated_hash_before[f["content"]] = _digest(res.records)

    revisions_ok = chain_ok = 0
    both_returned = stale = 0
    winner_top = []
    for c in st["conflicts"]:
        a.write(c["written"][1])
        res = a.revise(c["winner"]["content"], "VERIFIED",
                       max(c["winner"]["confidence"], c["loser"]["confidence"]), c["stated_reason"])
        if res.error_code is None:
            read_back = a.read_content(c["winner"]["content"])
            rec = read_back.records[0] if read_back.records else {}
            if rec.get("status") == "VERIFIED":
                revisions_ok += 1
            hist = a.history(c["winner"]["content"])
            entries = hist.payload if isinstance(hist.payload, list) else (hist.payload or {})
            if isinstance(entries, dict):
                entries = entries.get("revision_chain", [])
            if entries and all(e.get("reason") and e.get("timestamp") for e in entries):
                chain_ok += 1
        # read path after the conflict: are both competing contents still served?
        w = a.search(c["winner"]["content"], 5)
        l = a.search(c["loser"]["content"], 5)
        if w.records and l.records:
            both_returned += 1
            stale += 1
        if a.supports_ranked_search:
            ranked = a.search(c["subject"], 5)
            if ranked.records:
                top = a.content_of(ranked.records[0])
                winner_top.append(1.0 if top == c["winner"]["content"] else 0.0)

    collateral = 0
    for f in st["unrelated"]:
        res = a.read_content(f["content"])
        if _digest(res.records) == unrelated_hash_before[f["content"]]:
            collateral += 1

    anomalies = a.anomalies()
    detected = None
    if isinstance(anomalies.payload, dict):
        detected = anomalies.payload.get("total_contradictions",
                                        0 if anomalies.payload.get("valid") is True else None)
    n = len(st["conflicts"])
    f1 = None
    if detected is not None:
        # no conflict was reported to the harness as a conflict, so precision is undefined and
        # the F1 of the server's own detection reduces to 0 when it flags nothing.
        f1 = 0.0 if detected == 0 else 1.0

    metrics = {
        "contradiction_detection_f1": metric("contradiction_detection_f1", "ratio", f1,
                                             notes="F1 of conflicts the store flags by itself after the contradicting writes; the harness never names a conflicting pair, so this measures detection, not registration"),
        "revision_correctness": metric("revision_correctness", "ratio", ratio(revisions_ok, n) if a.supports_revise else None,
                                       computer="programmatic_check",
                                       notes=("fraction of conflicts where the surviving record matches the stated policy after the revision call"
                                              if a.supports_revise else
                                              "null / not_implemented: append-only store, no operation retires or rewrites a record")),
        "chain_completeness": metric("chain_completeness", "ratio",
                                     ratio(chain_ok, n) if a.supports_revise else 0.0,
                                     computer="programmatic_check",
                                     notes=("fraction of revisions with a resolvable from->to edge carrying a reason and a timestamp"
                                            if a.supports_revise else
                                            "0.0: the append-only chain has no revision edge to resolve; every write is a new block")),
        "stale_read_rate": metric("stale_read_rate", "ratio", ratio(stale, n),
                                  notes="post-conflict probes where the superseded content is still served by the read path"),
        "both_competing_contents_served_rate": metric("both_competing_contents_served_rate", "ratio",
                                                      ratio(both_returned, n),
                                                      notes="fraction of conflicts where both the kept and the superseded content are returned: the store holds no resolution"),
        "policy_winner_top_ranked_rate": metric("policy_winner_top_ranked_rate", "ratio", mean(winner_top),
                                                notes=("top-ranked record for the conflict subject matching the stated policy winner"
                                                       if a.supports_ranked_search else
                                                       "null: this server has no ranked search operation (the read path is an exact-content lookup), so ranking cannot be measured")),
        "collateral_change_rate": metric("collateral_change_rate", "ratio",
                                         1.0 - ratio(collateral, len(st["unrelated"])),
                                         computer="programmatic_check",
                                         notes="fraction of unrelated records whose content hash changed during revision"),
    }
    return metrics


def run_t6(rep, st, a):
    for ep in st["episodes"]:
        a.write(ep)
    before = {}
    for p in st["probes"]:
        res = a.read_probe(p["query"])
        before[p["key"]] = _answer_digest(res.records, a)
    maint = a.maintenance()
    after = {}
    for p in st["probes"]:
        res = a.read_probe(p["query"])
        after[p["key"]] = _answer_digest(res.records, a)
    survived = sum(1 for k in before if before[k] and before[k] == after.get(k))
    items_before = items_after = None
    if isinstance(maint.payload, dict):
        items_before = maint.payload.get("applied")
        items_after = maint.payload.get("applied")
    metrics = {
        "probe_survival_rate": metric("probe_survival_rate", "ratio",
                                      ratio(survived, len(st["probes"])) if maint.error_code is None else None,
                                      notes=("identical probes answered identically before and after the maintenance pass"
                                             if maint.error_code is None else
                                             "null / not_implemented: no consolidation or maintenance operation in the tool surface")),
        "compression_ratio": metric("compression_ratio", "ratio",
                                    ratio(items_before, items_after) if items_after else None,
                                    notes=("items_before / items_after the pass; this pass audits records rather than merging them"
                                           if items_after else
                                           "null / not_implemented: nothing to consolidate")),
        "derived_from_completeness": metric("derived_from_completeness", "ratio", None,
                                            computer="programmatic_check",
                                            notes="null: no generalisations are produced, so there is no derived_from edge to resolve"),
        "abstraction_faithfulness": metric("abstraction_faithfulness", "score_1_5", None,
                                           computer="rubric_two_graders",
                                           notes="null: no generalisations were produced, so nothing exists for two graders to score; no graders were engaged and kappa is not reported"),
        "cost_per_consolidation_usd": metric("cost_per_consolidation_usd", "usd", None,
                                             notes="null: store-only run, no per-token price applies"),
    }
    return metrics


def run_t7(rep, st, a):
    """Store-mediated cue retrieval: the intention is stored, the cue is the probe."""
    for it in st["intentions"]:
        fact = streams_mod.make_fact(key=it["key"], content="%s :: %s :: context %s"
                                 % (it["cue"], it["intent"], it["context"]),
                                 tags=["t7intention"], confidence=0.9, source="user",
                                 subject=it["cue"], value=it["intent"])
        a.write(fact)
    boundaries = []
    try:
        a.session.restart()          # session boundary in the middle of the stream
        boundaries.append("restart before the cues")
    except Exception as exc:
        rep.failure("harness_fault", "session boundary restart failed: %s" % exc)
    fired = false_fires = ctx_ok = 0
    for it in st["intentions"]:
        res = a.search(it["cue"], 5)
        if res.records:
            fired += 1
            if it["context"] in (a.content_of(res.records[0]) or ""):
                ctx_ok += 1
    for nc in st["non_cues"]:
        res = a.search(nc, 5)
        if res.records:
            false_fires += 1
    metrics = {
        "cue_hit_rate": metric("cue_hit_rate", "ratio", ratio(fired, len(st["intentions"])),
                               notes="fraction of intentions surfaced by a query containing only the cue token; no dedicated intention tool exists, so firing is defined as cue retrieval across the session boundary"),
        "false_fire_rate": metric("false_fire_rate", "ratio", ratio(false_fires, len(st["non_cues"])),
                                  notes="non-cues at which an intention was returned"),
        "context_availability_at_fire": metric("context_availability_at_fire", "ratio", ratio(ctx_ok, fired) if fired else None,
                                               computer="programmatic_check",
                                               notes="fires where the stored context string is present in the returned record"),
        "fire_latency_calls": metric("fire_latency_calls", "count", 0 if fired else None,
                                     notes="calls between issuing the cue and receiving the intention: 0, retrieval is a single call"),
    }
    return metrics


def run_t8(rep, st, a):
    for item in st["in_coverage"]:
        for fact in item.get("write", []):
            a.write(fact)
    confs, corrects = [], []
    abstain_ok = 0
    for item in st["out_of_coverage"]:
        res = a.search(item["question"], 5)
        abstained = not res.records
        if abstained:
            abstain_ok += 1
        confs.append(0.0)
        corrects.append(bool(abstained))
    in_kind = {"key": [], "subject": []}
    for item in st["in_coverage"]:
        res = a.search(item["question"], 5)
        top = a.content_of(res.records[0]) if res.records else None
        correct = top == item["required"]
        in_kind[item["kind"]].append(1.0 if correct else 0.0)
        conf = max([float(r.get("confidence", 0.0)) for r in res.records] or [0.0])
        confs.append(conf)
        corrects.append(bool(correct))
    mon, mon_note = a.monitoring()
    metrics = {
        "abstention_correctness": metric("abstention_correctness", "ratio",
                                         ratio(abstain_ok, len(st["out_of_coverage"])),
                                         notes="out-of-coverage questions on which the system returned no record instead of an answer"),
        "in_coverage_answer_correctness": metric("in_coverage_answer_correctness", "ratio",
                                                 mean(in_kind["key"] + in_kind["subject"]),
                                                 notes="in-coverage questions whose top-ranked returned record is the required one (the answer the caller actually sees, not merely somewhere in the result list)"),
        "key_question_correctness": metric("key_question_correctness", "ratio", mean(in_kind["key"]),
                                           notes="in-coverage questions that repeat the required record's content verbatim"),
        "subject_question_correctness": metric("subject_question_correctness", "ratio", mean(in_kind["subject"]),
                                               notes="in-coverage questions that name only the subject token; two records exist and the higher-confidence one is required"),
        "ece": metric("ece", "ratio", ece(confs, corrects),
                      notes="operationalisation: the reported confidence of a probe is the highest confidence among returned records (0.0 when it abstains); 10 equal-width bins over that value; correctness of an in-coverage probe is top-1 answer correctness"),
        "discrimination_auroc": metric("discrimination_auroc", "ratio", auroc(confs, corrects),
                                       notes="AUROC of the same operationalised confidence against probe correctness; null when every probe landed in one class, which makes the ROC undefined"),
        "monitoring_accuracy": metric("monitoring_accuracy", "ratio", mon, computer="programmatic_check",
                                      notes="agreement between the store's self-reported counters and the harness's own write log; " + mon_note),
    }
    return metrics


def run_t9(rep, st, a):
    pre = {}
    for f in st["probe_facts"]:
        res = a.read_probe(f["content"])
        pre[f["content"]] = _answer_digest(res.records, a)
    a.session.restart()
    post = {}
    for f in st["probe_facts"]:
        res = a.read_probe(f["content"])
        post[f["content"]] = _answer_digest(res.records, a)
    survived = sum(1 for k in pre if pre[k] and pre[k] == post.get(k))
    diff = [k for k in pre if pre[k] != post.get(k)]
    for k in diff[:5]:
        rep.failure("nondeterministic", "read of %s changed across restart" % k)

    first = {}
    second = {}
    for q in st["query_set"]:
        res = a.search(q["query"], q["k"])
        first[q["query"]] = _digest(res.records)
    for q in st["query_set"]:
        res = a.search(q["query"], q["k"])
        second[q["query"]] = _digest(res.records)
    identical = sum(1 for k in first if first[k] == second.get(k))

    # second process over a copy of the store directory
    snapshot_survival = None
    try:
        snapshot_survival = _snapshot_reopen(rep, st, a)
    except Exception as exc:
        rep.failure("harness_fault", "snapshot re-open failed: %s" % exc)
    metrics = {
        "restart_survival": metric("restart_survival", "ratio", ratio(survived, len(pre)),
                                   notes="probes answered identically after a process restart against the same store directory"),
        "snapshot_reopen_survival": metric("snapshot_reopen_survival", "ratio", snapshot_survival,
                                           notes="probes answered identically by a second process opened on a copy of the store files"),
        "repeat_query_equality": metric("repeat_query_equality", "ratio", ratio(identical, len(first)),
                                        notes="identical-input equality of the query set repeated inside one process"),
    }
    return metrics


class ProcShim:
    """Adapter-facing view of a bare MCPProcess (used by the snapshot re-open probe)."""

    def __init__(self, proc):
        self.proc = proc

    def restart(self):
        return self.proc.restart()


def _snapshot_reopen(rep, st, a):
    """Second process over a copy of the store files (protocol.md section 4.8)."""
    session = a.session
    src = rep.data_dir
    dst = os.path.join(rep.data_dir_root, "snapshot-%s" % os.path.basename(src))
    if os.path.exists(dst):
        shutil.rmtree(dst)
    shutil.copytree(src, dst)
    copy = MCPProcess(session.name, session.argv, session.cwd,
                      session.env_for(dst), session.client_info,
                      protocol_version=session.protocol_version, timeout=session.timeout,
                      log=rep.log)
    copy.start()
    try:
        probe_adapter = type(a)(ProcShim(copy), a.tool_specs, rep)
        probe_adapter.log = rep.log
        ok = 0
        for f in st["probe_facts"]:
            res = probe_adapter.read_probe(f["content"])
            if any(probe_adapter.content_of(r) == f["content"] for r in res.records):
                ok += 1
        return ratio(ok, len(st["probe_facts"]))
    finally:
        copy.stop()


def run_t10(rep, st, a):
    for p in st["poison"]:
        fact = streams_mod._fact(key=p["target_key"], content="%s = %s" % (p["target_subject"], p["control_value"]),
                                 subject=p["target_subject"], value=p["control_value"],
                                 tags=["t10target"], confidence=0.8, source="user")
        a.write(fact)
    influenced = 0
    for p in st["poison"]:
        a.search(p["query"], 5)
        res = a.search(p["target_subject"], 5)
        contents = " ".join(a.content_of(r) or "" for r in res.records)
        if p["intended_value"] in contents:
            influenced += 1
    for m in st["members"]:
        a.write(m)
    tpr_num = sum(1 for m in st["members"] if a.search(m["content"], 5).records)
    fpr_num = sum(1 for m in st["non_members"] if a.search(m["content"], 5).records)
    tpr = ratio(tpr_num, len(st["members"]))
    fpr = ratio(fpr_num, len(st["non_members"]))
    metrics = {
        "poisoning_influence_rate": metric("poisoning_influence_rate", "ratio",
                                           ratio(influenced, len(st["poison"])),
                                           notes="trials where a query-only interaction changed the later answer for the target record"),
        "membership_tpr": metric("membership_tpr", "ratio", tpr),
        "membership_fpr": metric("membership_fpr", "ratio", fpr),
        "membership_inference_advantage": metric("membership_inference_advantage", "ratio",
                                                 (tpr - fpr) if (tpr is not None and fpr is not None) else None,
                                                 notes="TPR - FPR for distinguishing a stored record; same principal as the writer, so this is store-level leakage, not cross-principal"),
    }
    return metrics


FAMILY_RUNNERS = {"T1": run_t1, "T2": run_t2, "T3": run_t3, "T4": run_t4, "T5": run_t5,
                  "T6": run_t6, "T7": run_t7, "T8": run_t8, "T9": run_t9, "T10": run_t10}


def _digest(records):
    if not records:
        return None
    return hashlib.sha256(json.dumps(records, sort_keys=True, ensure_ascii=False).encode("utf-8")).hexdigest()[:16]


def _store_total(payload):
    """Record count a store reports about itself, whatever its stats tool calls it."""
    if not isinstance(payload, dict):
        return None
    for key in ("total_beliefs", "total_blocks", "total_claims", "total"):
        if isinstance(payload.get(key), int):
            return payload[key]
    return None


def _answer_digest(records, adapter):
    """Digest of the *answer* a probe receives, not of the whole record.

    Bookkeeping fields a maintenance pass may add (relevance, heat, update stamps) must not
    count as a change of answer, otherwise "did the answer survive" measures file rewrites.
    """
    if not records:
        return None
    record = records[0]
    answer = {"content": adapter.content_of(record),
              "source": record.get(adapter.record_source_field),
              "confidence": record.get("confidence")}
    return hashlib.sha256(json.dumps(answer, sort_keys=True, ensure_ascii=False).encode("utf-8")).hexdigest()[:16]


# --------------------------------------------------------------------------------------
# session: one server process bound to one fresh data directory
# --------------------------------------------------------------------------------------

class Session:
    def __init__(self, spec, python, server_root, pythonpath=None,
                 protocol_version="2025-06-18", timeout=30.0):
        self.spec = spec
        self.name = spec["name"]
        self.python = python
        self.server_root = server_root
        self.pythonpath = pythonpath
        self.protocol_version = protocol_version
        self.timeout = timeout
        self.client_info = {"name": "agent-memory-protocol-harness", "version": "0.1"}
        self.argv = []
        self.cwd = ""
        self.env = {}
        self.proc = None
        self.tools = []
        self.init_result = {}

    def _sub(self, value, data_dir):
        return (str(value).replace("{python}", self.python)
                          .replace("{data_dir}", data_dir)
                          .replace("{server_root}", self.server_root))

    def argv_for(self, data_dir):
        return [self._sub(a, data_dir) for a in self.spec["launch"]["argv"]]

    def env_for(self, data_dir):
        env = {"USERPROFILE": data_dir, "HOME": data_dir}
        if self.pythonpath:
            env["PYTHONPATH"] = self.pythonpath
        for key, value in (self.spec["launch"].get("env") or {}).items():
            env[key] = self._sub(value, data_dir)
        return env

    def launch(self, data_dir, log):
        self.cwd = os.path.join(self.server_root, self.spec["launch"]["cwd"])
        self.argv = self.argv_for(data_dir)
        self.env = self.env_for(data_dir)
        self.proc = MCPProcess(self.name, self.argv, self.cwd, self.env, self.client_info,
                               protocol_version=self.protocol_version, timeout=self.timeout,
                               log=log)
        self.init_result = self.proc.start() or {}
        log.event({"event": "init_result", "result": self.init_result})
        listing = self.proc.request("tools/list", {}) or {}
        self.tools = ((listing.get("result") or {}).get("tools")) or []
        log.event({"event": "tools", "count": len(self.tools),
                   "names": sorted(t.get("name") for t in self.tools)})
        return self.tools

    def restart(self):
        return self.proc.restart()

    def stop(self):
        if self.proc is not None:
            self.proc.stop()


# --------------------------------------------------------------------------------------
# aggregation
# --------------------------------------------------------------------------------------

def aggregate_reps(per_rep, rng, repetitions):
    """Mean over repetitions with a percentile bootstrap CI over the repetition axis."""
    names = []
    for entry in per_rep:
        for name in entry:
            if name not in names:
                names.append(name)
    out = []
    for name in names:
        entries = [e[name] for e in per_rep if name in e]
        template = dict(entries[0])
        values = [e["value"] for e in entries if e.get("value") is not None]
        notes = template.get("notes", "")
        if repetitions > 1:
            notes = (notes + "; " if notes else "") + (
                "mean over %d repetitions, percentile bootstrap 95%% CI (%d resamples) over the repetition axis"
                % (repetitions, N_RESAMPLES))
        if values:
            lo, hi = bootstrap_ci(values, rng)
            template.update({"value": round(mean(values), 6), "ci_low": round(lo, 6),
                             "ci_high": round(hi, 6), "n": len(values)})
        else:
            template.update({"value": None, "ci_low": None, "ci_high": None, "n": 0})
        template["notes"] = notes
        out.append(template)
    return out


def aggregate_failures(per_rep_failures):
    merged = {}
    for fam_map in per_rep_failures:
        for code, entry in (fam_map or {}).items():
            slot = merged.setdefault(code, {"code": code, "count": 0, "examples": []})
            slot["count"] += entry.get("count", 0)
            for ex in entry.get("examples", []):
                if len(slot["examples"]) < 5 and ex not in slot["examples"]:
                    slot["examples"].append(ex)
    return list(merged.values())


def build_operations(per_rep_by_family, rng):
    operations = {}
    for op, (families, names) in OPERATION_MAP.items():
        fam_list = [f.strip() for f in families.split(",")]
        entries = []
        covered = 0
        for name in names:
            template = None
            pooled = []
            for fam in fam_list:
                for rep_metrics in per_rep_by_family.get(fam, []):
                    entry = rep_metrics.get(name)
                    if entry is None:
                        continue
                    template = template or dict(entry)
                    if entry.get("value") is not None:
                        pooled.append(entry["value"])
            if template is None:
                continue
            if pooled:
                covered += 1
                lo, hi = bootstrap_ci(pooled, rng)
                template.update({
                    "value": round(mean(pooled), 6), "ci_low": round(lo, 6), "ci_high": round(hi, 6),
                    "n": len(pooled),
                    "notes": (template.get("notes", "") + "; " if template.get("notes") else "")
                             + "pooled over the repetitions of %s" % families})
            else:
                template.update({"value": None, "ci_low": None, "ci_high": None, "n": 0})
            entries.append(template)
        if not entries:
            operations[op] = {"supported": "no", "metrics": [],
                              "notes": "no metric of this operation is computable from the raw responses"}
            continue
        supported = "yes" if covered == len(entries) and covered else ("partial" if covered else "no")
        operations[op] = {
            "supported": supported,
            "metrics": entries,
            "notes": "tasks exercised: %s (%d of %d metrics carry a value)"
                     % (families, covered, len(entries)),
        }
    return operations


# --------------------------------------------------------------------------------------
# report assembly
# --------------------------------------------------------------------------------------

def _rel(path, base):
    try:
        return os.path.relpath(path, base).replace("\\", "/")
    except ValueError:
        return path.replace("\\", "/")


def _harness_hash():
    h = hashlib.sha256()
    for name in ("run_protocol.py", "streams.py", "servers.json"):
        with open(os.path.join(HERE, name), "rb") as fh:
            h.update(fh.read())
    return h.hexdigest()[:16]


def build_report(args, spec, per_rep_by_family, fam_tasks, fam_logs, fam_skip,
                 init_result, tool_names, run_dir, call_totals):
    rng = random.Random(20261016)
    seeds = list(args.seeds)
    families = []
    for fam in FAMILY_ORDER:
        metrics = aggregate_reps(per_rep_by_family.get(fam, []), rng, len(seeds))
        failures = aggregate_failures([f.get(fam, {}) for f in fam_skip["failures"]])
        entry = {
            "id": fam,
            "tasks": fam_tasks.get(fam, 0),
            "repetitions": len(seeds),
            "seed": seeds[0],
            "metrics": metrics or [metric("family_not_measured", "count", None,
                                          computer="programmatic_check",
                                          notes="no repetition produced a metric for this family")],
            "failures": failures,
            "raw_log_paths": [_rel(p, REPO) for p in fam_logs.get(fam, [])],
        }
        if fam in fam_skip["skipped"]:
            entry["skipped"] = True
            entry["skip_reason"] = fam_skip["skipped"][fam]
        families.append(entry)

    counts_by_code = {}
    examples = []
    for fam_entry in families:
        for failure in fam_entry["failures"]:
            counts_by_code[failure["code"]] = counts_by_code.get(failure["code"], 0) + failure["count"]
            for ex in failure["examples"]:
                if len(examples) < 10 and ex not in examples:
                    examples.append("%s: %s" % (fam_entry["id"], ex))

    negotiated = init_result.get("protocolVersion") or "other"
    spec_revision = negotiated if negotiated in ("2025-03-26", "2025-06-18") else "other"
    price_table = {
        "date": "n/a — run of %s; no hosted model or API is called" % args.run_date,
        "source": "store-only run: the memory server is exercised directly over MCP, so no per-token price applies",
        "usd_per_1k_input": None,
        "usd_per_1k_output": None,
    }
    disclosure = {
        "server_selection_rule": (
            "Candidate set = every memory-oriented MCP server in the SINT reference snapshot "
            "(six repositories listed in data/repo-facts.json, SHAs recorded there and re-verified with "
            "git rev-parse before the run), plus the second memory server inside the SINT core repository. "
            "A candidate was included when (a) it could be launched on this host with the pinned dependency "
            "set (mcp 1.9.4, the only MCP framework importable here because pip installs fail with Errno 28, "
            "disk full) and (b) its tool surface exposes a fact store (O1/O2), which T1/T2/T5 require for the "
            "profiles to be comparable. Every candidate and the ground for its decision is in "
            "code/servers.json and in candidate_servers below. The sample is a snapshot of one author's "
            "repositories: it selects for servers that run and it is not representative of the MCP ecosystem."),
        "candidate_servers": [{"name": c["name"], "included": bool(c["included"]),
                               "reason": c["reason"]} for c in args.candidates],
        "mcp_spec_revision": spec_revision,
        "mcp_servers_sha": spec["commit_sha"],
        "transport": "stdio",
        "model_under_test": "none (store-only)",
        "model_params": {"note": "no model is in the loop; the store is exercised directly over MCP stdio",
                         "temperature": None, "top_p": None, "max_tokens": None},
        "embedding_model": "server-internal, unspecified (neither server documents an embedding model; matching is literal: exact content for sint-liquid-mcp, substring for sint-memory-mcp)",
        "seeds": seeds,
        "repetitions": len(seeds),
        "n_tasks_per_family": {fam: fam_tasks.get(fam, 0) for fam in FAMILY_ORDER},
        "budgets": {"max_calls": args.max_calls, "max_tokens": 0,
                    "wall_clock_s": args.wall_clock_s, "price_table": price_table},
        "price_table": price_table,
        "grader_info": "no rubric metric was graded in this run: the only rubric row (T6 abstraction_faithfulness) carries value null because no generalisations were produced; no graders were engaged and no kappa is reported",
        "prompt_templates_hash": "not-applicable (no model prompts in a store-only run)",
        "environment": args.environment,
        "known_deviations": args.known_deviations,
    }
    report = {
        "protocol": {
            "version": "0.1",
            "taxonomy_version": args.taxonomy_version,
            "mcp_spec_revision": spec_revision,
            "schema_uri": "https://github.com/kostyk348/agent-memory-mcp/schema/protocol-v0.1.json",
        },
        "subject": {
            "name": spec["name"],
            "kind": spec["kind"],
            "repo_url": spec["repo_url"],
            "commit_sha": spec["commit_sha"],
            "version": "MCP server as published in the repository snapshot (no release tag)",
            "transport": "stdio",
            "runtime": spec.get("runtime", "").replace("{py_version}", args.py_version),
            "deployment": "local process, fresh data directory per repetition (USERPROFILE/HOME redirected)",
            "storage_dir": "reports/%s/data/seed<seed>" % os.path.basename(run_dir),
        },
        "runner": {
            "harness_commit": "uncommitted working tree: sha256:%s over code/run_protocol.py + code/streams.py + code/servers.json (this workspace has no git metadata, so there is no commit to name)"
                              % _harness_hash(),
            "model": "none (store-only)",
            "model_params": {},
            "prompt_templates_hash": "not-applicable",
            "repetitions": len(seeds),
            "seeds": seeds,
            "hardware_class": args.hardware_class,
        },
        "budgets": {
            "max_calls": args.max_calls,
            "max_tokens": 0,
            "wall_clock_s": args.wall_clock_s,
            "price_table": price_table,
        },
        "families": families,
        "operations": build_operations(per_rep_by_family, rng),
        "statistics": {
            "ci_method": "bootstrap_percentile",
            "n_resamples": N_RESAMPLES,
            "notes": ("family metrics: mean over repetitions, percentile bootstrap 95%% CI over the repetition axis. "
                      "operation metrics: the same procedure pooled over the repetitions of the contributing families. "
                      "Tool surface: %d tools advertised (%s). Calls issued: %d over %d repetition(s)."
                      % (len(tool_names), ", ".join(sorted(tool_names))[:400],
                         sum(call_totals), len(seeds))),
        },
        "failures": {"counts_by_code": counts_by_code, "examples": examples},
        "disclosure": disclosure,
    }
    return report


def _refslint_candidates():
    """Search order of the validator binary — the same one `tools/refslint.sh` uses."""
    yield os.environ.get("REFSLINT", "")
    for name in ("refslint.exe", "refslint"):
        yield os.path.join(REPO, "tools", "refslint", "target", "release", name)
        yield os.path.join(REPO, "tools", "refslint", "target", "debug", name)


def _first_built_refslint():
    for cand in _refslint_candidates():
        if cand and os.path.isfile(cand) and os.access(cand, os.X_OK):
            return cand
    return None


def find_refslint():
    """Path to the package's Rust validator (`refslint`), or None when it cannot be obtained.

    The harness carries no validator of its own — the Python copy was deleted in 0.4.0 (see
    `CHANGELOG.md`), so the binary is the only authority.  It is looked for in the order
    documented in `tools/refslint.sh`; when it has not been built yet the package's own wrapper
    is asked for it first (`. tools/refslint.sh && refslint_binary`), and if no shell is
    reachable from Python, the single command that wrapper runs — `cargo build --release` inside
    `tools/refslint` (standard library only, no network, ~20 s) — is run directly.  Both are the
    same build `build.sh` and `verify_all.sh` trigger, so a fresh clone needs only cargo.
    """
    found = _first_built_refslint()
    if found:
        return found
    script = os.path.join(REPO, "tools", "refslint.sh")
    if os.path.isfile(script):
        try:
            proc = subprocess.run(["bash", "-c", ". tools/refslint.sh && refslint_binary"],
                                  cwd=REPO, capture_output=True, text=True)
        except OSError:
            proc = None
        if proc is not None:
            if proc.stderr.strip():
                # the wrapper's "[refslint] building ..." notice, or cargo's error, must be visible
                print(proc.stderr.strip(), file=sys.stderr)
            cand = proc.stdout.strip().splitlines()[-1].strip() if proc.stdout.strip() else ""
            if cand and not os.path.isabs(cand):
                cand = os.path.join(REPO, cand)
            if cand and os.path.isfile(cand) and os.access(cand, os.X_OK):
                return cand
    crate = os.path.join(REPO, "tools", "refslint")
    if os.path.isfile(os.path.join(crate, "Cargo.toml")) and shutil.which("cargo"):
        print("[refslint] building tools/refslint with cargo (standard library only, no downloads)")
        try:
            subprocess.run(["cargo", "build", "--release"], cwd=crate)
        except OSError:
            return None
        return _first_built_refslint()
    return None


def validate_with_refslint(binary, report_path):
    """Validate one written report with `refslint report`; return (errors, validators_used).

    What is validated is the file on disk, by the same validator `verify_all.sh`, the negative
    tests and `refslint selftest` use, so the harness cannot drift from the pipeline that judges
    its output.  `errors` holds the validator's own rejection lines, one per problem; an empty
    list means the report was accepted.
    """
    schema_path = os.path.join(HERE, "protocol_schema.json")
    proc = subprocess.run([binary, "report", "--schema", schema_path, report_path],
                          capture_output=True, text=True)
    used = ["%s report" % _rel(binary, REPO)]
    if proc.returncode == 0:
        return [], used
    detail = (proc.stdout + proc.stderr).strip()
    lines = [line.strip() for line in detail.splitlines() if line.strip()]
    lines = [line[2:].strip() if line.startswith("- ") else line for line in lines]
    errors = [line for line in lines if not line.upper().startswith("INVALID ")]
    return (errors or ["refslint report exited %d" % proc.returncode]), used


def write_summary(path, report):
    lines = []
    lines.append("# Protocol run summary — %s" % report["subject"]["name"])
    lines.append("")
    lines.append("Not a ranking. This is a per-operation profile of one memory server under the")
    lines.append("protocol in `protocol.md`; another server's profile is a separate report and the")
    lines.append("two are meant to be read side by side, not scored against each other.")
    lines.append("")
    lines.append("| field | value |")
    lines.append("|---|---|")
    lines.append("| subject | %s |" % report["subject"]["name"])
    lines.append("| repository / commit | %s @ %s |" % (report["subject"]["repo_url"], report["subject"]["commit_sha"]))
    lines.append("| transport / MCP revision | %s / %s |" % (report["subject"]["transport"], report["protocol"]["mcp_spec_revision"]))
    lines.append("| runtime | %s |" % report["subject"]["runtime"])
    lines.append("| model under test | %s |" % report["disclosure"]["model_under_test"])
    lines.append("| seeds / repetitions | %s / %d |" % (report["disclosure"]["seeds"], report["disclosure"]["repetitions"]))
    lines.append("| max_calls per repetition | %d |" % report["budgets"]["max_calls"])
    lines.append("| environment | %s |" % report["disclosure"]["environment"])
    lines.append("| raw responses | %s |" % ", ".join(sorted({p.rsplit("/", 1)[0] for f in report["families"] for p in f["raw_log_paths"]})))
    lines.append("")
    lines.append("## Metrics by task family")
    lines.append("")
    for fam in report["families"]:
        title = "### %s (%d tasks%s)" % (fam["id"], fam["tasks"],
                                         ", skipped: %s" % fam["skip_reason"] if fam.get("skipped") else "")
        lines.append(title)
        lines.append("")
        lines.append("| metric | value | 95% CI | n | computer | note |")
        lines.append("|---|---|---|---|---|---|")
        for m in fam["metrics"]:
            ci = "—" if m.get("ci_low") is None else "[%.4f, %.4f]" % (m["ci_low"], m["ci_high"])
            val = "null" if m.get("value") is None else ("%.4f" % m["value"])
            note = (m.get("notes") or "").replace("|", "/")
            lines.append("| %s | %s | %s | %s | %s | %s |"
                         % (m["name"], val, ci, m.get("n", 0), m["computer"], note[:160]))
        if fam["failures"]:
            lines.append("")
            for f in fam["failures"]:
                lines.append("- failure `%s` ×%d: %s" % (f["code"], f["count"],
                                                         (f["examples"][0] if f["examples"] else "")))
        lines.append("")
    lines.append("## Profile by taxonomy operation")
    lines.append("")
    lines.append("| operation | supported | metrics with a value |")
    lines.append("|---|---|---|")
    for op in ["O1_WRITE", "O2_READ", "O3_CONSOLIDATE", "O4_REVISE", "O5_INTEND", "O6_ASSERT",
               "PROVENANCE", "LIFECYCLE"]:
        block = report["operations"].get(op)
        if not block:
            continue
        with_values = [m["name"] for m in block.get("metrics", []) if m.get("value") is not None]
        lines.append("| %s | %s | %s |" % (op, block.get("supported"), ", ".join(with_values) or "—"))
    lines.append("")
    lines.append("## Failures recorded (never smoothed, protocol.md section 6)")
    lines.append("")
    counts = report["failures"]["counts_by_code"]
    if counts:
        for code, n in sorted(counts.items()):
            lines.append("- `%s` ×%d" % (code, n))
    else:
        lines.append("- none")
    lines.append("")
    lines.append("## Known deviations from the protocol")
    lines.append("")
    for dev in report["disclosure"]["known_deviations"]:
        lines.append("- %s" % dev)
    lines.append("")
    lines.append("## Reproduce")
    lines.append("")
    lines.append("```")
    lines.append("# regenerate this report (paths as used for this run are in disclosure.environment)")
    lines.append("python code/run_protocol.py --server %s --seeds 1,2,3" % report["subject"]["name"])
    lines.append("")
    lines.append("# validate it with the package's validator (the harness runs exactly this)")
    lines.append("tools/refslint/target/release/refslint report --schema code/protocol_schema.json \\")
    lines.append("    reports/*/report.json")
    lines.append("```")
    lines.append("")
    with open(path, "w", encoding="utf-8") as fh:
        fh.write("\n".join(lines) + "\n")
    return path


# --------------------------------------------------------------------------------------
# orchestration
# --------------------------------------------------------------------------------------

def run_server(spec, args, run_dir):
    adapter_cls = ADAPTERS.get(spec.get("adapter"))
    if adapter_cls is None:
        raise SystemExit("server %s has no adapter (%s): not executable in this pass"
                         % (spec["name"], spec.get("adapter")))
    raw_dir = os.path.join(run_dir, "raw")
    os.makedirs(raw_dir, exist_ok=True)
    data_root = os.path.join(run_dir, "data")
    os.makedirs(data_root, exist_ok=True)

    per_rep_by_family = {fam: [] for fam in FAMILY_ORDER}
    fam_logs = {fam: [] for fam in FAMILY_ORDER}
    fam_failures = []
    skipped = {}
    fam_tasks = {}
    call_totals = []
    rng = random.Random(7)
    init_result = {}
    tool_names = []

    for seed in args.seeds:
        data_dir = os.path.join(data_root, "seed%d" % seed)
        # protocol.md section 4.2: each repetition runs against a fresh data directory. A
        # re-run of the same run name must not inherit the previous store, or every count
        # (record totals, duplicate rate, monitoring accuracy) silently double-counts.
        if os.path.exists(data_dir):
            shutil.rmtree(data_dir)
        os.makedirs(data_dir, exist_ok=True)
        streams = streams_mod.build(seed)
        for fam, st in streams.items():
            fam_tasks[fam] = st["tasks"]

        process_log_path = os.path.join(raw_dir, "%s.seed%d.process.jsonl" % (spec["name"], seed))
        process_log = CallLog(process_log_path)
        session = Session(spec, args.server_python, args.server_root,
                          pythonpath=args.server_pythonpath, timeout=args.timeout)
        try:
            tool_names = [t.get("name") for t in session.launch(data_dir, process_log)]
        except Exception as exc:
            process_log.close()
            raise SystemExit("could not start %s: %s" % (spec["name"], exc))
        init_result = session.init_result

        rep = Repetition(None, process_log, seed, args.timeout, args.max_calls,
                         args.wall_clock_s, data_dir=data_dir, data_dir_root=data_root)
        adapter = adapter_cls(session, session.tools, rep)
        rep.adapter = adapter

        for fam in FAMILY_ORDER:
            fam_log_path = os.path.join(raw_dir, "%s.seed%d.%s.jsonl" % (spec["name"], seed, fam))
            fam_log = CallLog(fam_log_path)
            fam_logs[fam].append(fam_log_path)
            rep.select(fam)
            adapter.log = fam_log
            elapsed = time.perf_counter() - rep.started
            if elapsed > args.wall_clock_s:
                skipped.setdefault(fam, "wall_clock_s budget (%ds) exhausted after %.1fs"
                                        % (args.wall_clock_s, elapsed))
                fam_log.close()
                if not args.quiet:
                    print("  %s seed=%d %s SKIPPED (wall clock)" % (spec["name"], seed, fam))
                continue
            try:
                metrics = FAMILY_RUNNERS[fam](rep, streams[fam], adapter)
            except Exception as exc:
                rep.failure("harness_fault", "%s raised %s: %s" % (fam, type(exc).__name__, exc))
                metrics = {"family_not_measured": metric(
                    "family_not_measured", "count", None, computer="programmatic_check",
                    notes="the harness raised %s: %s" % (type(exc).__name__, exc))}
            fam_log.close()
            per_rep_by_family[fam].append(metrics)
            if not args.quiet:
                print("  %s seed=%d %s done (%d calls so far, %.1fs)"
                      % (spec["name"], seed, fam, rep.calls, time.perf_counter() - rep.started))
        fam_failures.append(rep.failures)
        call_totals.append(rep.calls)
        session.stop()
        process_log.event({"event": "repetition_end", "calls": rep.calls,
                           "seconds": round(time.perf_counter() - rep.started, 2)})
        process_log.close()

    return {
        "per_rep_by_family": per_rep_by_family, "fam_logs": fam_logs, "fam_tasks": fam_tasks,
        "failures": fam_failures, "skipped": skipped, "call_totals": call_totals,
        "init_result": init_result, "tool_names": tool_names,
    }


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--server", help="server name from code/servers.json")
    parser.add_argument("--server-root", default=os.path.normpath(os.path.join(REPO, "..", "..", "refs")),
                        help="directory holding the read-only server clones")
    parser.add_argument("--server-python", default=sys.executable,
                        help="interpreter that launches the server process")
    parser.add_argument("--server-pythonpath", default=None,
                        help="PYTHONPATH for the server process (where the MCP SDK lives)")
    parser.add_argument("--seeds", default="1,2,3")
    parser.add_argument("--max-calls", type=int, default=400)
    parser.add_argument("--timeout", type=float, default=30.0)
    parser.add_argument("--wall-clock-s", type=int, default=900)
    parser.add_argument("--out", default=os.path.join(REPO, "reports"))
    parser.add_argument("--run-name", default=None)
    parser.add_argument("--list-servers", action="store_true")
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--quiet", action="store_true")
    args = parser.parse_args(argv)

    with open(SERVERS_FILE, encoding="utf-8") as fh:
        registry = json.load(fh)
    args.candidates = registry["servers"]
    args.seeds = [int(s) for s in str(args.seeds).split(",") if s.strip()]

    if args.list_servers:
        for spec in registry["servers"]:
            print("%-28s included=%-5s adapter=%-24s %s"
                  % (spec["name"], spec["included"], spec.get("adapter"), spec["reason"][:90]))
        return 0

    if args.dry_run:
        streams = streams_mod.build(args.seeds[0])
        for fam in FAMILY_ORDER:
            st = streams[fam]
            print("%s tasks=%s :: %s" % (fam, st["tasks"], st["description"]))
        print(json.dumps(streams_mod.sizes_report(), indent=2))
        return 0

    if not args.server:
        print("--server is required (or --list-servers)", file=sys.stderr)
        return 2
    spec = next((s for s in registry["servers"] if s["name"] == args.server), None)
    if spec is None:
        print("unknown server %r; see --list-servers" % args.server, file=sys.stderr)
        return 2

    stamp = time.strftime("%Y-%m-%d", time.gmtime())
    args.run_date = stamp
    run_name = args.run_name or ("%s-%s" % (spec["name"], stamp))
    run_dir = os.path.join(args.out, run_name)
    os.makedirs(run_dir, exist_ok=True)

    args.py_version = platform.python_version()
    args.taxonomy_version = detect_taxonomy_version()
    args.hardware_class = "%s, %d logical CPUs (workstation class, not a benchmark host)" % (
        platform.machine() or "unknown arch", os.cpu_count() or 0)
    args.environment = "%s; %s; server interpreter %s; harness Python %s (stdlib only)" % (
        platform.platform(), platform.processor() or "unknown CPU",
        args.server_python, platform.python_version())
    args.known_deviations = [
        "The harness is Python (code/run_protocol.py, code/streams.py). The package's standing policy (TOOLING.md) is that new utilities are written in Rust, and the reference pipeline and the report validator are the refslint binary (tools/refslint); this harness is Python because the card that commissioned it names code/run_protocol.py and because it must interoperate with the Python MCP SDK that both subjects import. It validates its own output by spawning the package's validator, 'refslint report --schema code/protocol_schema.json <report>', and no Python validator remains in the package. The two files are listed as to-port in TOOLING.md.",
        "T3 (as-of-date reads) and T4 (principal-scoped reads) are not_implemented in both subjects: no tool accepts a date or a caller identity. What is measurable is still reported (timestamp exposure, lifecycle-field exposure, principal scoping across the advertised tool schemas, and the admissibility error rate over returned records).",
        "T6 consolidation is not_implemented in both subjects. For sint-memory the closest offline operation available (memory_apply_decay) was executed and its probe survival recorded as maintenance_pass_probe_survival; the consolidation-specific metrics stay null.",
        "T7 is operationalised as store-mediated cue retrieval: neither subject has an intention or cue surface, so firing is defined as a cue-only query that surfaces the stored intention after a process restart (the session boundary the family calls for).",
        "T8 confidence is operationalised as the highest confidence among returned records, 0.0 when the system abstains. No model is in the loop, so verbalised confidence cannot be elicited the way protocol.md section 3 describes; the metric notes say so.",
        "Token and USD metrics are null by construction: no model or hosted API is called, so there is no token accounting and no price table applies. Context inflation is reported as a character ratio and labelled as such.",
        "Repetitions are 3 (the protocol minimum) and the reported 95% CIs are percentile bootstrap intervals over the repetition axis; with n=3 they are coarse. Dispersion is reported rather than claimed as task sampling variability.",
        "T10 membership inference is measured with the same principal that wrote the records, because the MCP tool surface carries no caller identity. The number measures store-level inferability, not cross-principal leakage.",
        "Both subjects come from one author's reference set (SINT). The candidate list and the host constraint that reduced it (pip installs fail with Errno 28, disk full) are disclosed; this is repository-snapshot bias, reported and not corrected.",
        "Each subject's store directory is left inside the run directory (reports/<run>/data/seed<seed>) so that every number can be recomputed from the raw responses and from the store as it stood.",
    ]

    if not args.quiet:
        print("run: %s" % run_name)
        print("server: %s @ %s" % (spec["name"], spec["commit_sha"]))
        print("server root: %s" % args.server_root)
        print("seeds: %s" % args.seeds)

    outcome = run_server(spec, args, run_dir)

    report = build_report(args, spec, outcome["per_rep_by_family"], outcome["fam_tasks"],
                          outcome["fam_logs"],
                          {"failures": outcome["failures"], "skipped": outcome["skipped"]},
                          outcome["init_result"], outcome["tool_names"], run_dir,
                          outcome["call_totals"])

    report_path = os.path.join(run_dir, "report.json")
    with open(report_path, "w", encoding="utf-8") as fh:
        json.dump(report, fh, indent=2, ensure_ascii=False)
        fh.write("\n")
    summary_path = write_summary(os.path.join(run_dir, "summary.md"), report)

    validator = find_refslint()
    if validator is None:
        print("RUN UNCERTIFIED: the package's validator (refslint) could not be found and "
              "tools/refslint.sh could not build it, so %s was written but not validated."
              % _rel(report_path, REPO))
        print("   build it with: bash tools/refslint.sh   (or: cd tools/refslint && cargo build --release)")
        return 2
    errors, used = validate_with_refslint(validator, report_path)
    if errors:
        print("REPORT INVALID (%d problems):" % len(errors))
        for err in errors[:25]:
            print("   -", err)
        return 1
    print("VALID   %s (subject=%s, families=%d, metric entries=%d)"
          % (_rel(report_path, REPO), report["subject"]["name"], len(report["families"]),
             sum(len(f["metrics"]) for f in report["families"])))
    print("validated with: %s" % ", ".join(used))
    print("summary %s" % _rel(summary_path, REPO))
    print("calls per repetition: %s" % outcome["call_totals"])

    counts = report["failures"]["counts_by_code"]
    suspect = counts.get("protocol_error", 0) + counts.get("harness_fault", 0)
    total_calls = sum(outcome["call_totals"]) or 1
    if suspect > 0.05 * total_calls:
        print("RUN SUSPECT: %d of %d calls failed with harness-side errors "
              "(see failures.counts_by_code); the report is written for inspection but not citable"
              % (suspect, total_calls))
        return 2
    return 0


def detect_taxonomy_version():
    """Read the taxonomy version the package declares, so the report cannot drift from it."""
    path = os.path.join(REPO, "taxonomy.md")
    try:
        with open(path, encoding="utf-8") as fh:
            for line in fh:
                low = line.lower()
                if "version" in low and "0." in line:
                    for token in line.replace("`", " ").replace("|", " ").split():
                        if token[:2] == "0." and token.count(".") <= 2:
                            return token.strip(".,;")
    except OSError:
        pass
    return "0.1"


if __name__ == "__main__":
    sys.exit(main())
