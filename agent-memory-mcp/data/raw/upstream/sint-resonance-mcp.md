# sint-resonance-mcp

[![Release](https://img.shields.io/github/v/release/kostyk348/sint-resonance-mcp?sort=semver&color=blue)](https://github.com/kostyk348/sint-resonance-mcp/releases)
[![License](https://img.shields.io/badge/license-Apache-2.0-green)](https://github.com/kostyk348/sint-resonance-mcp/blob/main/LICENSE)
[![Last commit](https://img.shields.io/github/last-commit/kostyk348/sint-resonance-mcp?branch=main)](https://github.com/kostyk348/sint-resonance-mcp/commits/main)

Resonance Detection MCP server — proactively finds cross-domain pattern matches between the current problem and past experiences.

## Tools

| Tool | Description |
|------|-------------|
| `resonance_index(domain, problem, solution, tags)` | Index a problem-solution pair |
| `resonance_find(query, k, domains)` | Find top K resonating past problems |
| `resonance_structural(problem_structure, k)` | Match by structural pattern (not keywords) |
| `resonance_proactive(context_snapshot)` | Auto-suggest resonances from current context |
| `resonance_domains()` | List all domains in index |
| `resonance_stats()` | Index statistics |

## Architecture

```
sint-resonance-mcp/
├── server.py          — FastMCP server
├── config.py          — NAME, VERSION
├── resonance/
│   ├── index.py       — Problem-solution index (persisted to ~/.config/opencode/resonance/)
│   ├── embeddings.py  — TF-IDF + keyword overlap similarity (zero external deps)
│   ├── structural.py  — Structural pattern matching (AST-like fingerprints)
│   └── proactive.py   — Proactive suggestion engine
└── pyproject.toml
```

## Persistence

Data is stored at `~/.config/opencode/resonance/index.json` — survives server restart.
