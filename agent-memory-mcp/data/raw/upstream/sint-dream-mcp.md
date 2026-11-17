# sint-dream-mcp

[![Release](https://img.shields.io/github/v/release/kostyk348/sint-dream-mcp?sort=semver&color=blue)](https://github.com/kostyk348/sint-dream-mcp/releases)
[![License](https://img.shields.io/badge/license-Apache-2.0-green)](https://github.com/kostyk348/sint-dream-mcp/blob/main/LICENSE)
[![Last commit](https://img.shields.io/github/last-commit/kostyk348/sint-dream-mcp?branch=main)](https://github.com/kostyk348/sint-dream-mcp/commits/main)

**Agent Dreaming** — offline consolidation between MCP sessions.

Compresses similar memory blocks into generalized knowledge, finds error patterns, and suggests new skills.

## Tools

| Tool | Description |
|------|-------------|
| `dream_consolidate(blocks_json)` | Group blocks by domain+tags, merge into generalized FACT claims |
| `dream_error_patterns(blocks_json)` | Find recurring failure patterns → anti-patterns |
| `dream_skill_suggestions(blocks_json, usage_stats)` | Suggest skills when a tool is used 5+ times in a domain |
| `dream_generalize(domain)` | Generalize across all stored dreams for a domain |
| `dream_report()` | Full dream report combining all analyses |
| `dream_store(domain, content, kind)` | Persist a dream result to `~/.config/opencode/dreams/` |
