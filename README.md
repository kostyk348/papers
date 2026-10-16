# papers

Paper and preprint packages by Konstantin Parashchuk (GitHub [@kostyk348](https://github.com/kostyk348)).
Each directory is a self-contained package: manuscript sources, the artefacts the manuscript claims, and the
tools that regenerate every number in it.

| Package | Version | What it contains | Status |
|---|---|---|---|
| [Memory Operations in MCP Agent Ecosystems](agent-memory-mcp/) | 1.0 | operation taxonomy (6 operations, 2 record axes), evaluation protocol (10 task families, machine-validated report format), verified reference base (135 sources), two completed protocol runs on two memory servers | [released v1.0](https://github.com/kostyk348/papers/releases/tag/agent-memory-mcp-v1.0) — DOI pending (Zenodo deposit not yet made) |

## Principles these packages follow

- **No number is typed by hand.** Every table and every value in the text is generated from the report files that
  the protocol produced, by the tooling in the package (`refslint tables`, `refslint bib`). A digit in the manuscript
  exists only as a macro resolved from those files.
- **A claim is published only with a machine-checkable support.** Each package carries a claim ledger with an
  epistemic status per assertion (verified artefact / verified literature / position / not confirmed) and commands
  that reproduce it.
- **Tooling is compiled, not scripted.** The pipelines are single Rust binaries with subcommands, built with no
  external crates where the job allows it, and they ship with self-tests (including negative tests: a validator that
  accepts a valid input proves nothing, so each package includes deliberately broken inputs that must be rejected).
- **Failures are published too.** Runs that expose a defect in a measured system, and limitations of the measurement
  itself, are stated in the manuscript rather than smoothed over.

## Building a package

```sh
cd agent-memory-mcp
bash build.sh          # builds tex/main.pdf from sources + generated tables
```

PDF builds use `tectonic` or a TeX Live installation (the engine is not vendored in this repository).

## Note on anonymised paths

Before publication, local absolute paths in the run logs, reports and evidence files were replaced:
the user home prefix became `{{HOME}}` and the interpreter location became `<hermes-venv>`. Nothing else was
changed — the executed code, the raw protocol exchanges and every reported value are as produced.
Hashes recorded in the port/parity evidence files were computed before this substitution, so re-running the tools
reproduces the same content except for those two placeholders.
