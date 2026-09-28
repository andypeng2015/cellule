# cellule-ltx contributor guide

Root `AGENTS.md` applies. Read [the LTX guide](docs/README.md) and
[provenance](UPSTREAM.md) before changing formats or adapted code.

## Boundary

- Own managed SQLite capture, LTX validation, exact restore, immutable Cell
  objects, compaction, and sparse page access.
- Leave authority, leases, retention decisions, HTTP, credentials, and node
  scheduling outside this crate.
- `PreparedRoot` is a proposal. Only the runtime's fenced authority CAS can
  select it for an acknowledged command.

## Module map

| Concern | Entry | Evidence |
| --- | --- | --- |
| Managed database | `src/db/mod.rs` | `src/db/tests/mod.rs`, `tests/ltx.rs`. |
| WAL capture | `src/capture/mod.rs` | `src/capture/wal.rs`, `tests/ltx/crash.rs`. |
| Exact recovery | `src/recovery.rs` | `tests/cell/restore.rs`. |
| Cell roots and compaction | `src/replica/mod.rs` | `tests/cell.rs`, `src/replica/directory/tests.rs`. |
| Sparse writable VFS | `src/writable_vfs/mod.rs` | `tests/cell/roots.rs`. |
| Host resources | `src/environment/mod.rs` | `src/environment/tests.rs`, `tests/host.rs`. |
| Format decoding | `src/codec.rs`, `src/internal.rs` | `tests/ltx/vectors.rs`, `fuzz/`. |

Module entries live at `module/mod.rs`; child logic and unit tests sit in the
same directory. Public scenarios use `tests/cell.rs`, `tests/ltx.rs`, and
`tests/host.rs`. `scripts/check-module-layout.py` rejects orphaned modules.

## Invariants

- Checksummed LTX only; verify complete ordered page coverage and rolling
  database checksums.
- The committed WAL boundary is captured even when an incremental cut is too
  large; use a bounded full image instead of stranding a commit.
- Restore and compaction install fresh destinations at the exact endpoint.
- Cancellation does not roll back dispatched work. Retain admission and scratch
  until completion or reconciliation.
- Callers branch on `LtxError::classify()`, never on error text.

## Features and tests

`replica` adds object transport, Cell roots, compaction, and sparse SQL. Local
capture needs no network. Stable tests replay external vectors; nightly fuzzing
and provider qualification are separate gates.

```sh
CARGO_TARGET_DIR=$HOME/Workspace/crabbuild-target/<checkout> \
  cargo test -p cellule-ltx --features replica --locked
CARGO_TARGET_DIR=$HOME/Workspace/crabbuild-target/<checkout> \
  cargo test -p cellule-ltx --no-default-features --locked
python3 scripts/check-module-layout.py
```

[External vector provenance](tests/vectors/README.md) and
[example setup](examples/README.md) remain part of format reviews.
