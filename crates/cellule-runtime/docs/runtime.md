# Execution and receipts

One Cell actor serializes admission and publication. A bounded SQLite worker
runs the application callback; the actor owns the result gate and lifecycle.

```mermaid
sequenceDiagram
    participant Client
    participant Actor
    participant SQLite
    participant Store
    Client->>Actor: Typed command + request identity
    Actor->>SQLite: Execute and store outcome atomically
    SQLite-->>Actor: Committed WAL cut
    Actor->>Store: Prepare immutable root or durable follower proof
    Store-->>Actor: Authority CAS or quorum proof
    Actor-->>Client: Committed output + receipt
```

| Phase | Owner | Invariant |
| --- | --- | --- |
| Admission | Actor | Validate target, identity, epoch, deadline, and resource budget. |
| Execution | SQL worker | Callback and deduplication outcome commit in one SQLite transaction. |
| Capture | LTX | Capture every committed WAL boundary. |
| Publication | Actor | Select exact root through owner-fenced CAS or recoverable follower proof. |
| Reply | Client boundary | Return only a result with a durable receipt. |

The request ledger records the encoded outcome. Reusing the same request ID and
operation digest returns that outcome; a changed digest is rejected. A timeout
after dispatch is ambiguous. The caller resolves by identity instead of
re-executing a possibly committed operation.

Queries can require a minimum receipt. `ReadPolicy::CurrentOwner` preserves
owner order. `ReadPolicy::Replica` returns only a snapshot that proves its
actual position; it never silently falls back to the owner.

The pure coordination kernel in [`src/coordination/mod.rs`](../src/coordination/mod.rs)
chooses transitions without I/O. Actor adapters gather observations, call the
kernel, then execute effects. This separation lets the simulator and TLA+
model replay the same decisions.

For actor, deadline, takeover, and drain contracts, read the [detailed reference](runtime-detailed.md).
