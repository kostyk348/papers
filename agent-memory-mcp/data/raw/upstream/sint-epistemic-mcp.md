# sint-epistemic-mcp

[![Release](https://img.shields.io/github/v/release/kostyk348/sint-epistemic-mcp?sort=semver&color=blue)](https://github.com/kostyk348/sint-epistemic-mcp/releases)
[![License](https://img.shields.io/badge/license-Apache-2.0-green)](https://github.com/kostyk348/sint-epistemic-mcp/blob/main/LICENSE)
[![Last commit](https://img.shields.io/github/last-commit/kostyk348/sint-epistemic-mcp?branch=main)](https://github.com/kostyk348/sint-epistemic-mcp/commits/main)

Epistemic Status Tracking MCP server — tracks what an agent KNOWS vs INFERS vs ASSUMES with calibrated confidence.

## Tools

| Tool | Description |
|------|-------------|
| `epistemic_register(claim, status, confidence, source, domain)` | Register a knowledge claim with epistemic status |
| `epistemic_status(claim_query, domain)` | Look up claims by text or domain |
| `epistemic_calibrate(domain)` | Get calibration accuracy stats for a domain |
| `epistemic_gaps(domain)` | Identify domains with high ASSUMED/UNCERTAIN ratio |
| `epistemic_graph(domain)` | Export knowledge status as a graph (nodes + edges) |
| `epistemic_stats()` | Summary statistics across all claims |

## Status Types

- **VERIFIED** — confirmed by external validation
- **INFERRED** — logically derived from verified claims
- **ASSUMED** — taken as true without verification
- **UNCERTAIN** — insufficient evidence either way

## Usage

Add to your MCP config:
```json
{
  "mcpServers": {
    "sint-epistemic-mcp": {
      "command": "python",
      "args": ["/path/to/sint-epistemic-mcp/server.py"]
    }
  }
}
```
