# Examples and test map

| Path | Proves |
| --- | --- |
| [`basic.rs`](../examples/basic.rs) | Compile KV and Queue Cells, write/read a setting, claim and acknowledge a job. |
| [`orders.rs`](../examples/orders.rs) | Local SQL write, published receipt, and read-back. |
| [`attachments.rs`](../examples/attachments.rs) | Multipart Blob upload, published receipt, and read-back. |
| [`contracts.rs`](../tests/contracts.rs) | Identity, descriptor, and digest contracts. |
| [Application integration suite](../tests/integration.rs) | Typed primitives, recovery, host routing, replicas, and rollout. |
| [Performance scenarios](../PERFORMANCE.md) | Workload commands and environment boundaries. |

```sh
cargo run -p cellule-app --example basic --locked
cargo run -p cellule-app --example orders --locked
cargo run -p cellule-app --example attachments --locked
cargo test -p cellule-app --locked
```

The default suite uses local fixtures. The ignored three-process scenario
requires a disposable object-store bucket and explicit environment.
