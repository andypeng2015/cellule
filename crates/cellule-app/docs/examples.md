# Examples and test map

| Path | Proves |
| --- | --- |
| [`authoring.rs`](../examples/authoring.rs) | Module registration and stable descriptor compilation. |
| [`orders.rs`](../examples/orders.rs) | Local SQL write, published receipt, and read-back. |
| [`contracts.rs`](../tests/contracts.rs) | Identity, descriptor, and digest contracts. |
| [Reference application](../tests/reference_application.rs) | Typed primitives, recovery, host routing, replicas, and rollout. |
| [Performance scenarios](../PERFORMANCE.md) | Dated workload definitions and environment boundaries. |

```sh
cargo run -p cellule-app --example authoring --locked
cargo run -p cellule-app --example orders --locked
cargo test -p cellule-app --locked
```

The default suite uses local fixtures. The ignored three-process scenario
requires a disposable object-store bucket and explicit environment; the
[framework verification report](../../../docs/verification.md) records one
local RustFS run. Dated benchmark reports are provenance, not current SLOs.
