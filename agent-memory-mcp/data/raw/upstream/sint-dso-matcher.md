# sint-dso-matcher

[![Release](https://img.shields.io/github/v/release/kostyk348/sint-dso-matcher?sort=semver&color=blue)](https://github.com/kostyk348/sint-dso-matcher/releases)
[![License](https://img.shields.io/badge/license-Apache-2.0-green)](https://github.com/kostyk348/sint-dso-matcher/blob/main/LICENSE)
[![Last commit](https://img.shields.io/github/last-commit/kostyk348/sint-dso-matcher?branch=main)](https://github.com/kostyk348/sint-dso-matcher/commits/main)

DSO Deterministic Matcher — fast pattern matching between current problems and past experience forest. Bagging + weighted voting. **Zero LLM calls.**

## Tools (all return JSON)

| Tool | Description |
|------|-------------|
| `dso_match(query, domain, k)` | Find top K similar past problems |
| `dso_vote(query, candidates_json)` | Weighted voting on best approach |
| `dso_fingerprint(text)` | Generate structural fingerprint hash |
| `dso_similarity(fp1, fp2)` | Compute similarity (0.0–1.0) |
| `dso_domains()` | List domains with indexed problems |
| `dso_matcher_stats()` | Match statistics |

## Fingerprint Algorithm

1. Tokenize: lowercase, split, remove stopwords
2. Extract key terms (nouns + verbs, skip common words)
3. Create overlapping n-gram sets (bigrams, trigrams)
4. Jaccard similarity on n-gram sets
5. TF-IDF weighted term overlap
6. Combined: 0.6 × Jaccard + 0.4 × TF-IDF

## Usage

```bash
pip install .
python server.py
```

## Structure

```
sint-dso-matcher/
├── server.py          # FastMCP server
├── config.py          # Configuration
├── dso/
│   ├── __init__.py
│   ├── fingerprint.py # Structural fingerprint generation
│   ├── similarity.py  # Jaccard + TF-IDF similarity
│   ├── matcher.py     # Full matching engine
│   └── index.py       # Persistent problem index
├── README.md
├── pyproject.toml
└── .gitignore
```
