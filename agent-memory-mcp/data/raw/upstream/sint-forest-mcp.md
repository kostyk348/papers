# sint-forest-mcp

[![Release](https://img.shields.io/github/v/release/kostyk348/sint-forest-mcp?sort=semver&color=blue)](https://github.com/kostyk348/sint-forest-mcp/releases)
[![License](https://img.shields.io/badge/license-Apache-2.0-green)](https://github.com/kostyk348/sint-forest-mcp/blob/main/LICENSE)
[![Last commit](https://img.shields.io/github/last-commit/kostyk348/sint-forest-mcp?branch=main)](https://github.com/kostyk348/sint-forest-mcp/commits/main)

Experience Ensemble / Random Forest for Agents — an MCP server that stores lightweight "experience trees" from past decisions, samples random subsets (bagging), and votes on approaches. Prevents overfitting to any single experience path.

## Tools

| Tool | Description |
|---|---|
| `forest_seed` | Initialize/reset forest for a domain |
| `forest_learn` | Add an experience tree (problem → approach → outcome) |
| `forest_sample_trees` | Randomly sample K trees from domain + M from related domains |
| `forest_vote_on_approaches` | Each tree scores candidate approaches by similarity + outcome weight |
| `forest_forget_weak` | Prune trees below weight threshold |
| `forest_consolidate_similar` | Merge similar trees by approach text similarity |
| `forest_stats` | Forest statistics (tree count, diversity, top approaches) |

## Persistence

Data stored as JSON in `~/.config/opencode/forest/{domain}.json`.
