# sint-dso-validator

[![Release](https://img.shields.io/github/v/release/kostyk348/sint-dso-validator?sort=semver&color=blue)](https://github.com/kostyk348/sint-dso-validator/releases)
[![License](https://img.shields.io/badge/license-Apache-2.0-green)](https://github.com/kostyk348/sint-dso-validator/blob/master/LICENSE)
[![Last commit](https://img.shields.io/github/last-commit/kostyk348/sint-dso-validator?branch=master)](https://github.com/kostyk348/sint-dso-validator/commits/master)

DSO Deterministic Validator — checks agent outputs for register compliance, schema correctness, and quality criteria. Zero LLM calls, pure functions.

## Tools

| Tool | Description |
|---|---|
| `dso_validate_register` | Check output matches register rules (SENSE/FACT/LOGIC/OPINION/ACTION/CAUSALITY) |
| `dso_validate_schema` | Validate JSON against a named schema (block/session/chain_entry) |
| `dso_validate_quality` | Check quality: completeness, clarity, conciseness |
| `dso_register_requirements` | Get requirements for a register |
| `dso_suggest_fix` | Suggest how to fix register violations |
| `dso_validator_stats` | Summary of all validations run |

## Usage

```python
from dso.registers import validate_register
result = validate_register("Source: observed X", "SENSE")
print(result)
```

Run as FastMCP server:

```bash
python server.py
```

## Registers

- **SENSE**: source + observation, no interpretation
- **FACT**: verifiable claim, source recommended
- **LOGIC**: explicit premises + conclusion
- **OPINION**: declared actor ("I think")
- **ACTION**: actionable steps
- **CAUSALITY**: cause → effect chain

## Stats

Persisted to `~/.config/opencode/dso/validator_stats.json`.
