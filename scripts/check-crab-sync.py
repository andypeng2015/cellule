#!/usr/bin/env python3
"""Compare Cellule's extracted capability code with a local Crab checkout.

No checkout is modified. Mechanical framework naming is applied first, then the
reviewable framework-only patch. Any upstream patch conflict or unexplained
source/test drift fails. Documentation is maintained separately.
"""
from __future__ import annotations

import argparse
import subprocess
import tempfile
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MAPPING = {
    "crab-cell-runtime": "cellule-runtime",
    "crab-cell-app": "cellule-app",
    "crab-cell-host": "cellule-host",
    "crab-cell-peer-http": "cellule-peer-http",
    "crab-ltx": "cellule-ltx",
}
NAMES = {**MAPPING, "crab-storage": "cellule-store", "crab-types": "cellule-types"}
# These additions have their own tests; they do not replace upstream behavior.
LOCAL_FILES = {"crates/cellule-peer-http/src/tests.rs"}


def capability_file(path: str) -> bool:
    if "__pycache__" in Path(path).parts or Path(path).suffix in {".md", ".png", ".svg"}:
        return False
    return (
        path.startswith(("src/", "tests/", "docs/contracts/", "qualification/", "model/", "fuzz/", "perf/"))
        or path in {"Cargo.toml", "build.rs", "api-prelude.txt", "tests-allow-list.txt", "LICENSE", "LICENSE.pierrec-lz4"}
    )


def adapt(data: bytes, relative: str) -> bytes:
    try:
        text = data.decode()
    except UnicodeDecodeError:
        return data
    for source, target in NAMES.items():
        text = text.replace(source, target).replace(source.replace("-", "_"), target.replace("-", "_"))
    text = text.replace("CrabError", "LtxError")
    text = text.replace("CRAB_CELL_", "CELLULE_").replace("CRAB_LTX_", "CELLULE_LTX_")
    text = text.replace("CRAB_COORDINATION_", "CELLULE_COORDINATION_")
    text = text.replace("CRAB_REFERENCE_", "CELLULE_REFERENCE_")
    text = text.replace("crab.application.v1", "cellule.application.v1")
    text = text.replace("crab.cell.peer.v1", "cellule.peer.v1").replace("crab.peer.v1", "cellule.peer.v1")
    text = text.replace("CatalogRole::Repository", "CatalogRole::Application").replace("Self::Repository", "Self::Application")
    text = text.replace('runtime: "crab-http-server"', 'runtime: "cellule"')
    text = text.replace('release.runtime != "crab-http-server"', 'release.runtime != "cellule"')
    text = text.replace('"runtime": "crab-http-server"', '"runtime": "cellule"')
    text = text.replace('Application => "repository"', 'Application => "application"')
    text = text.replace('".crab/blob-parts', '".cellule/blob-parts')
    if relative.endswith("src/cell/catalog.rs"):
        text = text.replace("    Repository,", "    Application,")
        text = text.replace("The namespace serves repository content.", "The namespace serves application-defined content.")
    if relative.count("/") == 2 and relative.endswith("Cargo.toml"):
        text = text.replace("publish = false\n", "")
    text = text.replace("for Crab services", "for embedding services")
    text = text.replace("concerns of `crab-http-server`", "concerns of the embedding service")
    text = text.replace('for Crab"', 'for Cellule"').replace("Crab Cell applications", "Cellule applications")
    text = text.replace(
        "// A panic in a filter process or FUSE path corrupts a worktree, so production\n"
        "// builds deny unwrap, expect, panic, todo, and unimplemented; test builds keep\n"
        "// them available.",
        "// Production panics can abandon accepted work and persistence resources; tests\n"
        "// retain assertions while runtime paths propagate typed errors.",
    )
    return text.encode()


def materialize(source: Path, destination: Path) -> set[str]:
    files = subprocess.check_output(
        ["git", "ls-files", "-z", "--", *(f"crates/{crate}" for crate in MAPPING)],
        cwd=source,
    ).decode().split("\0")
    paths = set()
    rust = []
    for name in filter(None, files):
        _, crate, relative = name.split("/", 2)
        if not capability_file(relative):
            continue
        output = f"crates/{MAPPING[crate]}/{relative.replace('perf/crab/', 'perf/cellule/')}"
        path = destination / output
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(adapt((source / name).read_bytes(), output))
        paths.add(output)
        if path.suffix == ".rs" and not relative.startswith(("tests/vectors/generate/", "perf/", "fuzz/")):
            rust.append(str(path))
    subprocess.run(
        ["rustfmt", "--edition", "2024", "--config", "skip_children=true", *rust],
        check=True, capture_output=True,
    )
    return paths


def workspace_drift(source: Path) -> list[str]:
    upstream = tomllib.loads((source / "Cargo.toml").read_text())["workspace"]
    local = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]
    inverse = {target: name for name, target in NAMES.items()}
    problems = []
    for name, declaration in local["dependencies"].items():
        expected = upstream["dependencies"].get(inverse.get(name, name))
        if isinstance(expected, dict):
            expected = expected.copy()
            if "path" in expected:
                expected["path"] = adapt(expected["path"].encode(), "").decode()
        if declaration != expected:
            problems.append(f"workspace dependency contract differs: {name}")
    for field in ("edition", "rust-version", "license"):
        if local["package"][field] != upstream["package"][field]:
            problems.append(f"workspace package contract differs: {field}")
    return problems


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--crab-source", type=Path, required=True)
    args = parser.parse_args()
    source = args.crab_source.resolve()
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=source, text=True).strip()
    dirty = subprocess.check_output(
        ["git", "status", "--porcelain", "--", "Cargo.toml", *(f"crates/{name}" for name in MAPPING)],
        cwd=source, text=True,
    )
    if dirty:
        parser.error("the Crab capability inputs have uncommitted changes; use a stable source checkout")
    problems = workspace_drift(source)
    if problems:
        for problem in problems:
            print(f"error: {problem}")
        return 1
    with tempfile.TemporaryDirectory(prefix="cellule-crab-sync-") as temporary:
        expected = Path(temporary)
        paths = materialize(source, expected)
        patch = ROOT / "scripts/crab-adaptations.patch"
        applied = subprocess.run(["git", "apply", str(patch)], cwd=expected, capture_output=True, text=True)
        if applied.returncode:
            print("error: Crab changed a framework adaptation; review the upstream change and patch")
            print(applied.stderr.strip())
            return 1
        mismatches = []
        for relative in sorted(paths):
            actual = ROOT / relative
            if not actual.is_file() or actual.read_bytes() != (expected / relative).read_bytes():
                mismatches.append(relative)
        for crate in MAPPING.values():
            for path in (ROOT / "crates" / crate).rglob("*"):
                relative = path.relative_to(ROOT).as_posix()
                inner = path.relative_to(ROOT / "crates" / crate).as_posix()
                if path.is_file() and capability_file(inner) and relative not in paths | LOCAL_FILES:
                    mismatches.append(relative + " (not in Crab or the explicit extension set)")
        if mismatches:
            for path in mismatches:
                print(f"error: capability drift: {path}")
            return 1
    print(f"ok: {len(paths)} capability files match Crab {revision} with reviewed Cellule adaptations")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
