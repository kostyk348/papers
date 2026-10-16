# pre-port-runner: the Python harness, frozen

`run_protocol.py` and `streams.py` are the protocol harness of `protocol.md` **as it stood before
the port to `tools/protorun`** (Rust, standard library only). They were copied here byte for byte —
`SHA256SUMS.txt` carries the digests, and `cmp` against `code/` is clean — so that the port can
always be checked against the thing it replaced instead of against a description of it.

| File | sha256 | What it is |
|---|---|---|
| `run_protocol.py` | `fb051d36281f1094…` | the harness: streams → MCP stdio JSON-RPC → raw logs → metrics → `report.json` + `summary.md` (self-validated with `refslint report`) |
| `streams.py` | `d7975f0266587f3c…` | deterministic T1–T10 stream generator with programmatic gold labels |

Python command line, as the pre-port copy documents it:

```bash
python code/run_protocol.py --list-servers
python code/run_protocol.py --server sint-liquid-mcp --seeds 1,2,3
python code/validate_report.py reports/*/report.json        # removed in 0.4.0; refslint report is the validator
```

The Rust port is invoked as:

```bash
tools/protorun/target/release/protorun servers
tools/protorun/target/release/protorun streams --seed 1
tools/protorun/target/release/protorun run --server sint-liquid-mcp --seeds 1,2,3
tools/protorun/target/release/protorun parity streams       --reference reports/<python run> --server <name>
tools/protorun/target/release/protorun parity run-vs-report --reference reports/<python run> --run reports/parity-rust/<run>
```

The parity record is `evidence/protorun-port-2026-10-16.txt`; `TOOLING.md` and `CHANGELOG.md`
(0.5.0) name this directory as the pre-port copy.

**These files are a frozen reference, not a pipeline.** They are not wired into `build.sh`,
`verify_all.sh` or any card. `code/run_protocol.py` and `code/streams.py` still exist for now
because other documents in the package (`protocol.md`, `README.md` §4b, the run summaries) name
them; removing them is a follow-up that also has to rewrite those references.
