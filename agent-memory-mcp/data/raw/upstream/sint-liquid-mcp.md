# sint-liquid-mcp

[![Release](https://img.shields.io/github/v/release/kostyk348/sint-liquid-mcp?sort=semver&color=blue)](https://github.com/kostyk348/sint-liquid-mcp/releases)
[![License](https://img.shields.io/badge/license-Apache-2.0-green)](https://github.com/kostyk348/sint-liquid-mcp/blob/main/LICENSE)
[![Last commit](https://img.shields.io/github/last-commit/kostyk348/sint-liquid-mcp?branch=main)](https://github.com/kostyk348/sint-liquid-mcp/commits/main)

**Liquid Memory** — adaptive belief revision MCP server.

Instead of append-only memory, Liquid Memory detects contradictions between old and new information, automatically revises beliefs, and maintains a revision chain.

## Tools

| Tool | Description |
|------|-------------|
| `liquid_belief` | Register/update a belief (upsert by claim text) |
| `liquid_query` | Look up belief by ID or claim text with full revision history |
| `liquid_contradict` | Register that two beliefs contradict |
| `liquid_resolve` | Resolve a contradiction: mark which belief is correct |
| `liquid_revise` | Manually revise a belief with a reason |
| `liquid_history` | Full revision chain of a belief |
| `liquid_domains` | List all domains with registered beliefs |
| `liquid_stats` | Summary: total beliefs, contradictions, revisions |

## Persistence

Data is stored at `~/.config/opencode/liquid/`.

## Run

```bash
pip install mcp
python server.py
```
