# Protocol run summary — sint-liquid-mcp

Not a ranking. This is a per-operation profile of one memory server under the
protocol in `protocol.md`; another server's profile is a separate report and the
two are meant to be read side by side, not scored against each other.

| field | value |
|---|---|
| subject | sint-liquid-mcp |
| repository / commit | https://github.com/kostyk348/sint-liquid-mcp @ bfffce2cdec35c11aa0d90ae02689d69df596b11 |
| transport / MCP revision | stdio / 2025-03-26 |
| runtime | Python 3.14.7, mcp 1.9.4 (mcp.server.fastmcp) |
| model under test | none (store-only) |
| seeds / repetitions | [1, 2, 3] / 3 |
| max_calls per repetition | 400 |
| environment | Windows-11-10.0.26200-SP0; Intel64 Family 6 Model 140 Stepping 2, GenuineIntel; server interpreter <hermes-venv>/Scripts/python.exe; harness Python 3.14.7 (stdlib only) |
| raw responses | reports/sint-liquid-mcp-2026-10-16/raw |

## Metrics by task family

### T1 (12 tasks)

| metric | value | 95% CI | n | computer | note |
|---|---|---|---|---|---|
| write_precision | 1.0000 | [1.0000, 1.0000] | 3 | harness | correct_written_records / written_records. A stored record counts as correct when it matches a generated fact and is not an extra copy of one; written_records i |
| write_recall | 1.0000 | [1.0000, 1.0000] | 3 | harness | correct_written_records / generated_facts; mean over 3 repetitions, percentile bootstrap 95% CI (1000 resamples) over the repetition axis |
| duplicate_rate | 0.0000 | [0.0000, 0.0000] | 3 | harness | extra_records_from_retries / written_records; a retry is the same content with the same intent; mean over 3 repetitions, percentile bootstrap 95% CI (1000 resam |
| idempotency_violations | 0.0000 | [0.0000, 0.0000] | 3 | programmatic_check | retries that created an extra record (protocol.md section 3); mean over 3 repetitions, percentile bootstrap 95% CI (1000 resamples) over the repetition axis |
| write_latency_p50_ms | 17.8175 | [17.0155, 18.4985] | 3 | harness | wall clock around tools/call; mean over 3 repetitions, percentile bootstrap 95% CI (1000 resamples) over the repetition axis |
| write_latency_p95_ms | 21.0234 | [19.3115, 21.8968] | 3 | harness | mean over 3 repetitions, percentile bootstrap 95% CI (1000 resamples) over the repetition axis |
| tokens_per_write | null | — | 0 | harness | null: no model is in the loop, so no token accounting exists; payload sizes are in the raw log; mean over 3 repetitions, percentile bootstrap 95% CI (1000 resam |
| cost_per_1000_writes_usd | null | — | 0 | harness | null: store-only run, no per-token price applies (see budgets.price_table); mean over 3 repetitions, percentile bootstrap 95% CI (1000 resamples) over the repet |

### T2 (6 tasks)

| metric | value | 95% CI | n | computer | note |
|---|---|---|---|---|---|
| recall_at_5 | 0.5000 | [0.5000, 0.5000] | 3 | harness | required record inside the top k returned; mean over 3 repetitions, percentile bootstrap 95% CI (1000 resamples) over the repetition axis |
| required_record_hit_rate | 0.5000 | [0.5000, 0.5000] | 3 | harness | required record anywhere in the returned list, ignoring k; mean over 3 repetitions, percentile bootstrap 95% CI (1000 resamples) over the repetition axis |
| mrr | 0.5000 | [0.5000, 0.5000] | 3 | harness | mean over 3 repetitions, percentile bootstrap 95% CI (1000 resamples) over the repetition axis |
| key_query_hit_rate | 1.0000 | [1.0000, 1.0000] | 3 | harness | queries that repeat the record's content verbatim; mean over 3 repetitions, percentile bootstrap 95% CI (1000 resamples) over the repetition axis |
| topic_query_hit_rate | 0.0000 | [0.0000, 0.0000] | 3 | harness | queries that name only a shared topic token, so several records match; mean over 3 repetitions, percentile bootstrap 95% CI (1000 resamples) over the repetition |
| provenance_correctness | 1.0000 | [1.0000, 1.0000] | 3 | harness | returned records whose source field matches the generated record; mean over 3 repetitions, percentile bootstrap 95% CI (1000 resamples) over the repetition axis |
| context_inflation | 0.5000 | [0.5000, 0.5000] | 3 | harness | characters returned / characters of the records actually required (character ratio, not tokens: no tokenizer in the loop); mean over 3 repetitions, percentile b |
| read_latency_p50_ms | 4.4998 | [4.1825, 4.9225] | 3 | harness | mean over 3 repetitions, percentile bootstrap 95% CI (1000 resamples) over the repetition axis |
| read_latency_p95_ms | 13.7778 | [12.8863, 14.3422] | 3 | harness | mean over 3 repetitions, percentile bootstrap 95% CI (1000 resamples) over the repetition axis |

### T3 (4 tasks)

| metric | value | 95% CI | n | computer | note |
|---|---|---|---|---|---|
| temporal_correctness | null | — | 0 | harness | null / not_implemented: the server exposes no date-scoped read, so the as-of probe cannot be answered; mean over 3 repetitions, percentile bootstrap 95% CI (100 |
| temporal_metadata_exposed | 1.0000 | [1.0000, 1.0000] | 3 | programmatic_check | fraction of read-back records carrying a timestamp field (metadata exists, the query does not); mean over 3 repetitions, percentile bootstrap 95% CI (1000 resam |

- failure `not_implemented` ×12: probe 'as of 2026-02-01' for t3-1-00: no tool accepts an as-of date

### T4 (6 tasks)

| metric | value | 95% CI | n | computer | note |
|---|---|---|---|---|---|
| admissibility_error_rate | 0.5000 | [0.5000, 0.5000] | 3 | programmatic_check | returned records that belong to a different principal than the caller; the tool surface has no caller parameter, so this is a policy check over the responses; m |
| lifecycle_state_exposed | 0.0000 | [0.0000, 0.0000] | 3 | programmatic_check | returned records exposing a lifecycle field (expiry/supersede marker); mean over 3 repetitions, percentile bootstrap 95% CI (1000 resamples) over the repetition |
| principal_scoping_exposed | 0.0000 | [0.0000, 0.0000] | 3 | programmatic_check | fraction of tools whose input schema accepts a caller/principal argument; read from the tools/list inputSchema of this run; mean over 3 repetitions, percentile  |

### T5 (4 tasks)

| metric | value | 95% CI | n | computer | note |
|---|---|---|---|---|---|
| contradiction_detection_f1 | 0.0000 | [0.0000, 0.0000] | 3 | harness | F1 of conflicts the store flags by itself after the contradicting writes; the harness never names a conflicting pair, so this measures detection, not registrati |
| revision_correctness | 1.0000 | [1.0000, 1.0000] | 3 | programmatic_check | fraction of conflicts where the surviving record matches the stated policy after the revision call; mean over 3 repetitions, percentile bootstrap 95% CI (1000 r |
| chain_completeness | 1.0000 | [1.0000, 1.0000] | 3 | programmatic_check | fraction of revisions with a resolvable from->to edge carrying a reason and a timestamp; mean over 3 repetitions, percentile bootstrap 95% CI (1000 resamples) o |
| stale_read_rate | 1.0000 | [1.0000, 1.0000] | 3 | harness | post-conflict probes where the superseded content is still served by the read path; mean over 3 repetitions, percentile bootstrap 95% CI (1000 resamples) over t |
| both_competing_contents_served_rate | 1.0000 | [1.0000, 1.0000] | 3 | harness | fraction of conflicts where both the kept and the superseded content are returned: the store holds no resolution; mean over 3 repetitions, percentile bootstrap  |
| policy_winner_top_ranked_rate | null | — | 0 | harness | null: this server has no ranked search operation (the read path is an exact-content lookup), so ranking cannot be measured; mean over 3 repetitions, percentile  |
| collateral_change_rate | 0.0000 | [0.0000, 0.0000] | 3 | programmatic_check | fraction of unrelated records whose content hash changed during revision; mean over 3 repetitions, percentile bootstrap 95% CI (1000 resamples) over the repetit |

### T6 (4 tasks)

| metric | value | 95% CI | n | computer | note |
|---|---|---|---|---|---|
| probe_survival_rate | null | — | 0 | harness | null / not_implemented: no consolidation or maintenance operation in the tool surface; mean over 3 repetitions, percentile bootstrap 95% CI (1000 resamples) ove |
| compression_ratio | null | — | 0 | harness | null / not_implemented: nothing to consolidate; mean over 3 repetitions, percentile bootstrap 95% CI (1000 resamples) over the repetition axis |
| derived_from_completeness | null | — | 0 | programmatic_check | null: no generalisations are produced, so there is no derived_from edge to resolve; mean over 3 repetitions, percentile bootstrap 95% CI (1000 resamples) over t |
| abstraction_faithfulness | null | — | 0 | rubric_two_graders | null: no generalisations were produced, so nothing exists for two graders to score; no graders were engaged and kappa is not reported; mean over 3 repetitions,  |
| cost_per_consolidation_usd | null | — | 0 | harness | null: store-only run, no per-token price applies; mean over 3 repetitions, percentile bootstrap 95% CI (1000 resamples) over the repetition axis |

- failure `not_implemented` ×3: no consolidation or maintenance operation in the tool surface

### T7 (3 tasks)

| metric | value | 95% CI | n | computer | note |
|---|---|---|---|---|---|
| cue_hit_rate | 0.0000 | [0.0000, 0.0000] | 3 | harness | fraction of intentions surfaced by a query containing only the cue token; no dedicated intention tool exists, so firing is defined as cue retrieval across the s |
| false_fire_rate | 0.0000 | [0.0000, 0.0000] | 3 | harness | non-cues at which an intention was returned; mean over 3 repetitions, percentile bootstrap 95% CI (1000 resamples) over the repetition axis |
| context_availability_at_fire | null | — | 0 | programmatic_check | fires where the stored context string is present in the returned record; mean over 3 repetitions, percentile bootstrap 95% CI (1000 resamples) over the repetiti |
| fire_latency_calls | null | — | 0 | harness | calls between issuing the cue and receiving the intention: 0, retrieval is a single call; mean over 3 repetitions, percentile bootstrap 95% CI (1000 resamples)  |

### T8 (12 tasks)

| metric | value | 95% CI | n | computer | note |
|---|---|---|---|---|---|
| abstention_correctness | 1.0000 | [1.0000, 1.0000] | 3 | harness | out-of-coverage questions on which the system returned no record instead of an answer; mean over 3 repetitions, percentile bootstrap 95% CI (1000 resamples) ove |
| in_coverage_answer_correctness | 0.5000 | [0.5000, 0.5000] | 3 | harness | in-coverage questions whose top-ranked returned record is the required one (the answer the caller actually sees, not merely somewhere in the result list); mean  |
| key_question_correctness | 1.0000 | [1.0000, 1.0000] | 3 | harness | in-coverage questions that repeat the required record's content verbatim; mean over 3 repetitions, percentile bootstrap 95% CI (1000 resamples) over the repetit |
| subject_question_correctness | 0.0000 | [0.0000, 0.0000] | 3 | harness | in-coverage questions that name only the subject token; two records exist and the higher-confidence one is required; mean over 3 repetitions, percentile bootstr |
| ece | 0.3667 | [0.3667, 0.3667] | 3 | harness | operationalisation: the reported confidence of a probe is the highest confidence among returned records (0.0 when it abstains); 10 equal-width bins over that va |
| discrimination_auroc | 0.7500 | [0.7500, 0.7500] | 3 | harness | AUROC of the same operationalised confidence against probe correctness; null when every probe landed in one class, which makes the ROC undefined; mean over 3 re |
| monitoring_accuracy | 1.0000 | [1.0000, 1.0000] | 3 | programmatic_check | agreement between the store's self-reported counters and the harness's own write log; compared 2 self-reported counters; mean over 3 repetitions, percentile boo |

### T9 (18 tasks)

| metric | value | 95% CI | n | computer | note |
|---|---|---|---|---|---|
| restart_survival | 1.0000 | [1.0000, 1.0000] | 3 | harness | probes answered identically after a process restart against the same store directory; mean over 3 repetitions, percentile bootstrap 95% CI (1000 resamples) over |
| snapshot_reopen_survival | 1.0000 | [1.0000, 1.0000] | 3 | harness | probes answered identically by a second process opened on a copy of the store files; mean over 3 repetitions, percentile bootstrap 95% CI (1000 resamples) over  |
| repeat_query_equality | 1.0000 | [1.0000, 1.0000] | 3 | harness | identical-input equality of the query set repeated inside one process; mean over 3 repetitions, percentile bootstrap 95% CI (1000 resamples) over the repetition |

### T10 (12 tasks)

| metric | value | 95% CI | n | computer | note |
|---|---|---|---|---|---|
| poisoning_influence_rate | 0.0000 | [0.0000, 0.0000] | 3 | harness | trials where a query-only interaction changed the later answer for the target record; mean over 3 repetitions, percentile bootstrap 95% CI (1000 resamples) over |
| membership_tpr | 1.0000 | [1.0000, 1.0000] | 3 | harness | mean over 3 repetitions, percentile bootstrap 95% CI (1000 resamples) over the repetition axis |
| membership_fpr | 0.0000 | [0.0000, 0.0000] | 3 | harness | mean over 3 repetitions, percentile bootstrap 95% CI (1000 resamples) over the repetition axis |
| membership_inference_advantage | 1.0000 | [1.0000, 1.0000] | 3 | harness | TPR - FPR for distinguishing a stored record; same principal as the writer, so this is store-level leakage, not cross-principal; mean over 3 repetitions, percen |

## Profile by taxonomy operation

| operation | supported | metrics with a value |
|---|---|---|
| O1_WRITE | partial | write_precision, write_recall, duplicate_rate, write_latency_p50_ms, write_latency_p95_ms, idempotency_violations |
| O2_READ | yes | recall_at_5, mrr, required_record_hit_rate, provenance_correctness, admissibility_error_rate, context_inflation, read_latency_p50_ms, read_latency_p95_ms |
| O3_CONSOLIDATE | no | — |
| O4_REVISE | yes | contradiction_detection_f1, revision_correctness, chain_completeness, stale_read_rate, collateral_change_rate |
| O5_INTEND | partial | cue_hit_rate, false_fire_rate |
| O6_ASSERT | yes | abstention_correctness, in_coverage_answer_correctness, key_question_correctness, subject_question_correctness, ece, discrimination_auroc, monitoring_accuracy |
| PROVENANCE | partial | provenance_correctness |
| LIFECYCLE | yes | lifecycle_state_exposed, admissibility_error_rate, stale_read_rate, restart_survival, snapshot_reopen_survival |

## Failures recorded (never smoothed, protocol.md section 6)

- `not_implemented` ×15

## Known deviations from the protocol

- The harness is Python (code/run_protocol.py, code/streams.py). The package's standing policy (TOOLING.md) is that new utilities are written in Rust and the reference pipeline has been ported to the refslint binary; this harness is Python because the card that commissioned it names code/run_protocol.py, because it must interoperate with the Python MCP SDK that both subjects import, and because the validator it is required to pass is code/validate_report.py, invoked as 'python code/validate_report.py reports/<file>.json'. The two files are listed as to-port in TOOLING.md; the reports they produced are accepted by that validator and by refslint report, which is recorded in the run summary.
- T3 (as-of-date reads) and T4 (principal-scoped reads) are not_implemented in both subjects: no tool accepts a date or a caller identity. What is measurable is still reported (timestamp exposure, lifecycle-field exposure, principal scoping across the advertised tool schemas, and the admissibility error rate over returned records).
- T6 consolidation is not_implemented in both subjects. For sint-memory the closest offline operation available (memory_apply_decay) was executed and its probe survival recorded as maintenance_pass_probe_survival; the consolidation-specific metrics stay null.
- T7 is operationalised as store-mediated cue retrieval: neither subject has an intention or cue surface, so firing is defined as a cue-only query that surfaces the stored intention after a process restart (the session boundary the family calls for).
- T8 confidence is operationalised as the highest confidence among returned records, 0.0 when the system abstains. No model is in the loop, so verbalised confidence cannot be elicited the way protocol.md section 3 describes; the metric notes say so.
- Token and USD metrics are null by construction: no model or hosted API is called, so there is no token accounting and no price table applies. Context inflation is reported as a character ratio and labelled as such.
- Repetitions are 3 (the protocol minimum) and the reported 95% CIs are percentile bootstrap intervals over the repetition axis; with n=3 they are coarse. Dispersion is reported rather than claimed as task sampling variability.
- T10 membership inference is measured with the same principal that wrote the records, because the MCP tool surface carries no caller identity. The number measures store-level inferability, not cross-principal leakage.
- Both subjects come from one author's reference set (SINT). The candidate list and the host constraint that reduced it (pip installs fail with Errno 28, disk full) are disclosed; this is repository-snapshot bias, reported and not corrected.
- Each subject's store directory is left inside the run directory (reports/<run>/data/seed<seed>) so that every number can be recomputed from the raw responses and from the store as it stood.

## Reproduce

```
# regenerate this report (paths as used for this run are in disclosure.environment)
python code/run_protocol.py --server sint-liquid-mcp --seeds 1,2,3

# validate it with both validators in this package
python code/validate_report.py reports/*/report.json
tools/refslint/target/release/refslint report --schema code/protocol_schema.json \
    reports/*/report.json
```

