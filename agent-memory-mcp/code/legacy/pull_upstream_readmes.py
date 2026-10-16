#!/usr/bin/env python3
"""Cache the READMEs of the SINT repositories this paper draws its evidence base from.

The paper cites its own implementation as an artifact.  To keep every claim in
``taxonomy.md`` and ``claims.md`` traceable, the raw upstream READMEs are mirrored
here at a pinned commit.  Repositories are read-only: this script only fetches.

Usage:  python code/pull_upstream_readmes.py
"""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEST = ROOT / "data" / "raw" / "upstream"

REPOS = [
    "sint",
    "sint-liquid-mcp",
    "sint-epistemic-mcp",
    "sint-dream-mcp",
    "sint-prospective-mcp",
    "sint-working-mcp",
    "sint-forest-mcp",
    "sint-historian-mcp",
    "sint-resonance-mcp",
    "sint-ua-v2.1",
    "sint-portfolio-mcp",
    "sint-learner-mcp",
    "sint-self-mcp",
    "sint-llm-swarm",
    "sint-dso-swarm",
    "sint-dso-router",
    "sint-dso-validator",
    "sint-dso-matcher",
    "sint-config",
    "sint-manifesto",
]


def gh(*args: str) -> str:
    out = subprocess.run(["gh", *args], capture_output=True, text=True, encoding="utf-8")
    if out.returncode != 0:
        raise RuntimeError(f"gh {' '.join(args)}: {out.stderr.strip()}")
    return out.stdout


def main() -> int:
    DEST.mkdir(parents=True, exist_ok=True)
    manifest, errors = [], []
    for repo in REPOS:
        slug = f"kostyk348/{repo}"
        try:
            meta = json.loads(
                gh("repo", "view", slug, "--json",
                   "name,description,isPrivate,defaultBranchRef,pushedAt,createdAt,licenseInfo,url")
            )
            readme = gh("api", f"repos/{slug}/readme", "--jq", ".content") 
            import base64

            text = base64.b64decode(readme).decode("utf-8", "replace")
            (DEST / f"{repo}.md").write_text(text, encoding="utf-8")
            head = json.loads(
                gh("api", f"repos/{slug}/commits", "--jq", ".[0] | {sha: .sha, date: .commit.author.date}")
            )
            manifest.append(
                {
                    "repo": slug,
                    "url": meta["url"],
                    "description": meta["description"],
                    "private": meta["isPrivate"],
                    "default_branch": (meta.get("defaultBranchRef") or {}).get("name"),
                    "pushed_at": meta["pushedAt"],
                    "created_at": meta["createdAt"],
                    "license": (meta.get("licenseInfo") or {}).get("spdxId"),
                    "head_commit": head["sha"],
                    "head_commit_date": head["date"],
                    "readme_bytes": len(text),
                }
            )
            print(f"ok   {slug:32s} head={head['sha'][:8]} readme={len(text)}B")
        except Exception as exc:  # noqa: BLE001
            errors.append({"repo": slug, "error": str(exc)})
            print(f"FAIL {slug}: {exc}")

    (DEST / "manifest.json").write_text(
        json.dumps({"repos": manifest, "errors": errors}, indent=2, ensure_ascii=False), encoding="utf-8"
    )
    print(f"\nreadmes={len(manifest)} errors={len(errors)} -> {DEST}")
    return 0 if not errors else 1


if __name__ == "__main__":
    sys.exit(main())
