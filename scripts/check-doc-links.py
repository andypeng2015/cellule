#!/usr/bin/env python3
"""Reject local Markdown links that no longer resolve after source/doc moves."""

from __future__ import annotations

import re
from pathlib import Path
from urllib.parse import unquote

ROOT = Path(__file__).resolve().parents[1]
LINK = re.compile(r"\]\(([^)]+)\)")


def main() -> int:
    missing = []
    checked = 0
    for doc in sorted([ROOT / "README.md", *ROOT.glob("docs/**/*.md"), *ROOT.glob("crates/**/*.md")]):
        for raw in LINK.findall(doc.read_text()):
            target = raw.split("#", 1)[0].split(" ", 1)[0].strip("<>")
            if not target or target.startswith(("http:", "https:", "mailto:", "/")):
                continue
            checked += 1
            if not (doc.parent / unquote(target)).exists():
                missing.append(f"{doc.relative_to(ROOT)}: {target}")
    for item in missing:
        print(f"error: missing local link: {item}")
    if missing:
        return 1
    print(f"ok: {checked} local Markdown links resolve")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
