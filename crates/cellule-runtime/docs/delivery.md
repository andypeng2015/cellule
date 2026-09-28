# Verification and qualification

A feature is useful when a typed action creates a real side effect and a
subsequent read observes it. Mocked unit checks alone do not prove wiring.

| Level | Evidence | Primary suite |
| --- | --- | --- |
| Contracts | Stable IDs, schemas, codecs, and transition invariants. | `tests/contracts.rs`, `docs/validate.mjs`. |
| Runtime | Actor, publication, recovery, and primitive behavior. | `tests/runtime.rs`, `tests/primitives.rs`. |
| Fleet | Enrollment, follower proof, owner loss, and placement. | `tests/fleet.rs`, `tests/protocol.rs`. |
| Application | Typed call → durable result → read-back. | `cellule-app/tests/reference_application.rs`. |
| Process | Separate nodes with an object store. | Ignored three-process reference smoke. |
| Production | Fault, load, provider, security, and rollout evidence. | Separate controlled environments. |

```mermaid
flowchart LR
    Contracts --> Runtime --> Application --> Process --> Production
```

```sh
cargo test -p cellule-runtime --features test-support --locked
python3 scripts/check-boundaries.py
python3 scripts/check-module-layout.py
node crates/cellule-runtime/docs/validate.mjs
```

The commands run from the workspace root. The [qualification harness](../qualification/README.md)
separates local contracts from provider and multi-process gates. The
[historical verification report](../../../docs/verification.md) records the
original extraction's actual checks; it is not a current production claim.

For test matrix, receipts, provider qualification, and evidence gates, read the [detailed reference](delivery-detailed.md).
