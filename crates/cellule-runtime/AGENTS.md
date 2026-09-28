# cellule-runtime contributor guide

Root `AGENTS.md` applies. Cellule owns this implementation and its contracts.
Read [the runtime guide](docs/README.md) before changing behavior.

## Boundary

- Own Cell identity, authority CAS, actor and SQL execution, request outcomes,
  follower durability, placement, primitives, and qualification mechanics.
- Leave HTTP ingress, product authorization, credentials, and provider
  construction with the embedding service.
- Keep one canonical response gate: a successful command has an exact published
  root or recoverable follower proof.

## Module map

| Concern | Start at | Adjacent tests |
| --- | --- | --- |
| Actor and worker | `src/cell/actor/mod.rs`, `src/cell/worker/mod.rs` | `src/cell/actor/tests.rs`, `src/cell/worker/tests/mod.rs` |
| Client routing | `src/client/mod.rs` | `src/client/tests/mod.rs` |
| Authority and publication | `src/control/mod.rs`, `src/publication/mod.rs` | Their `tests.rs` siblings. |
| Node and follower log | `src/node/mod.rs`, `src/follower/mod.rs` | Co-located unit tests and `tests/fleet.rs`. |
| Primitives and registry | `src/primitives/mod.rs`, `src/registry/mod.rs` | `tests/primitives.rs`, `tests/contracts.rs`. |
| Recovery and fleet | `src/recovery/mod.rs`, `src/fleet/mod.rs` | `tests/runtime.rs`, `tests/fleet.rs`. |
| Qualification | `src/qualification/mod.rs` | `tests/qualification.rs`. |
| Pure coordination | `src/coordination/mod.rs` | `src/coordination/tests/mod.rs` and `model/`. |

A module with children uses `module/mod.rs`; focused production and unit-test
files sit beside it. Integration suites test the public path under `tests/`.
No `#[path]` indirection. `scripts/check-module-layout.py` catches orphaned
modules. The root API inventory is `api-prelude.txt`.

## Invariants

- One fenced writer per Cell. Stale owners lose admission before further SQL.
- Outcome and mutation commit together; response waits for durable proof.
- Exact recovery verifies the authority-pinned root and every dependency.
- Staged objects flush before bundle publication; locks and admissions release
  on success, error, cancellation, and timeout.
- Pressure shedding requires a full dwell window; a sampled tier drives bounded
  eviction and reported telemetry.
- The coordination kernel has no I/O, clock, or async. Gather observations in
  adapters, decide in the kernel, then act.

## Verification

```sh
CARGO_TARGET_DIR=$HOME/Workspace/crabbuild-target/<checkout> \
  cargo test -p cellule-runtime --features test-support --locked
CARGO_TARGET_DIR=$HOME/Workspace/crabbuild-target/<checkout> \
  cargo clippy -p cellule-runtime --all-targets --features test-support --locked -- -D warnings
python3 scripts/check-module-layout.py
python3 scripts/check-boundaries.py
```

Use a target directory unique to the checkout. Provider and process
qualification requires separate controlled environments; see
[delivery](docs/delivery.md) and [the qualification harness](qualification/README.md).
