# sint-dso-router

[![Release](https://img.shields.io/github/v/release/kostyk348/sint-dso-router?sort=semver&color=blue)](https://github.com/kostyk348/sint-dso-router/releases)
[![License](https://img.shields.io/badge/license-Apache-2.0-green)](https://github.com/kostyk348/sint-dso-router/blob/main/LICENSE)
[![Last commit](https://img.shields.io/github/last-commit/kostyk348/sint-dso-router?branch=main)](https://github.com/kostyk348/sint-dso-router/commits/main)

**DSO Deterministic Router** — classifies tasks by semantic register and domain using hash-based lookup + decision tree. Zero LLM calls, zero API calls, pure deterministic logic.

## Tools

| Tool | Description |
|------|-------------|
| `dso_route(task)` | Classify a task → `{register, domain, complexity, suggested_agent, confidence, reasoning}` |
| `dso_register_stats()` | Show register classification rules |
| `dso_domain_stats()` | Show domain classification rules |
| `dso_learn_rule(register, keywords, domain)` | Add a new classification rule |
| `dso_test(task, expected_register, expected_domain)` | Test classification accuracy |

## Usage

```python
from dso.classifier import route

result = route("implement AES encryption in C")
# {"register": "action", "domain": "crypto", "complexity": "simple", ...}

```

## Architecture

- **tokenizer.py** — lowercase, split on non-alpha, stopword removal, suffix stemming
- **rules.py** — rule registry with persistent storage to `~/.config/opencode/dso/router_rules.json`
- **classifier.py** — hash-based register + domain classification
- **complexity.py** — complexity estimator by keyword match count
- **server.py** — FastMCP server + CLI entry point
