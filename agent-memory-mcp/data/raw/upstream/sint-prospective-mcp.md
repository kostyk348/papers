# sint-prospective-mcp

[![Release](https://img.shields.io/github/v/release/kostyk348/sint-prospective-mcp?sort=semver&color=blue)](https://github.com/kostyk348/sint-prospective-mcp/releases)
[![License](https://img.shields.io/badge/license-Apache-2.0-green)](https://github.com/kostyk348/sint-prospective-mcp/blob/main/LICENSE)
[![Last commit](https://img.shields.io/github/last-commit/kostyk348/sint-prospective-mcp?branch=main)](https://github.com/kostyk348/sint-prospective-mcp/commits/main)

**Prospective Memory** MCP server — tracks the agent's intentions, suspended tasks, deferred decisions, and context markers across context windows and sessions.

## Tools

| Tool | Description |
|------|-------------|
| `prospective_push` | Push new intention onto stack (LIFO with nesting) |
| `prospective_pop` | Mark intention as completed, pop from stack |
| `prospective_suspend` | Suspend intention (interrupted by higher priority) |
| `prospective_resume` | Get top active intention + context to resume |
| `prospective_list` | List intentions by status filter |
| `prospective_marker` | Create context marker at a position |
| `prospective_snapshot` | Full state snapshot for session end |
| `prospective_restore` | Restore state at session start |

## Architecture

```
Intention Stack (LIFO tree)
  ├── depth tracking
  ├── parent/children links
  └── status: active | suspended | completed | abandoned

Marker Store
  └── ordered position-indexed context markers

Persistence
  └── ~/.config/opencode/prospective/
```

## Usage

```python
from mcp.client import stdio_client

# or run directly:

python server.py
```
