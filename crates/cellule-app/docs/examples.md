# Examples and test map

| Path | Proves |
| --- | --- |
| [`application_descriptor.rs`](../examples/application_descriptor.rs) | Module registration, Cell topology, and descriptor compilation. |
| [`orders.rs`](../examples/orders.rs) | Local SQL write, published receipt, and read-back. |
| [`attachments.rs`](../examples/attachments.rs) | Multipart Blob upload, published receipt, and read-back. |
| [`contracts.rs`](../tests/contracts.rs) | Identity, descriptor, and digest contracts. |
| [Application integration suite](../tests/integration.rs) | Typed primitives, recovery, host routing, replicas, and rollout. |
| [Performance scenarios](../PERFORMANCE.md) | Workload commands and environment boundaries. |

```sh
cargo run -p cellule-app --example application_descriptor --locked
cargo run -p cellule-app --example orders --locked
cargo run -p cellule-app --example attachments --locked
cargo test -p cellule-app --locked
```

The default suite uses local fixtures. The ignored three-process scenario
requires a disposable object-store bucket and explicit environment.
