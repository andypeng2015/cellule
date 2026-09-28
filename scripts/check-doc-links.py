#!/usr/bin/env python3
"""Reject local Markdown links that no longer resolve after source/doc moves."""

from __future__ import annotations

import re
from functools import lru_cache
from pathlib import Path
from urllib.parse import unquote

ROOT = Path(__file__).resolve().parents[1]
LINK = re.compile(r"\]\(([^)]+)\)")
HEADING = re.compile(r"^#+\s+(.+)$", re.M)
EXPLICIT_ANCHOR = re.compile(r'<a id="([^"]+)"></a>')


@lru_cache(maxsize=None)
def anchors(path: Path) -> set[str]:
    text = path.read_text()
    headings = {
        re.sub(r"[^\w\-\s]", "", match.group(1).lower()).replace(" ", "-")
        for match in HEADING.finditer(text)
    }
    return headings | set(EXPLICIT_ANCHOR.findall(text))


def main() -> int:
    missing = []
    checked = 0
    for doc in sorted([ROOT / "README.md", *ROOT.glob("docs/**/*.md"), *ROOT.glob("crates/**/*.md")]):
        for raw in LINK.findall(doc.read_text()):
            link = raw.split(" ", 1)[0].strip("<>")
            if link.startswith(("http:", "https:", "mailto:", "/")):
                continue
            target, _, anchor = link.partition("#")
            checked += 1
            destination = doc.parent / unquote(target) if target else doc
            if not destination.exists():
                missing.append(f"{doc.relative_to(ROOT)}: {target}")
            elif anchor and destination.suffix == ".md" and unquote(anchor) not in anchors(destination):
                missing.append(f"{doc.relative_to(ROOT)}: {link} (anchor)")
    for item in missing:
        print(f"error: missing local link: {item}")
    if missing:
        return 1
    print(f"ok: {checked} local Markdown links and anchors resolve")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
