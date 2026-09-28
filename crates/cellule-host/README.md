# cellule-host

`CellNode` owns one runtime, its admission ledger, facilities, and task group.
An application supplies identity, provider, ingress, and authorization.

```mermaid
stateDiagram-v2
    [*] --> Starting
    Starting --> Ready: lease and required components installed
    Ready --> Draining: stop admission and producers
    Draining --> Closing: accepted work drained
    Closing --> Stopped: close log and withdraw session
```

| Guide | Topic |
| --- | --- |
| [Lifecycle](docs/lifecycle.md) | Builder, readiness, drain, and shutdown. |
| [Read replicas](docs/read-replicas.md) | Admission, recruitment, refresh, and eviction. |
| [Crate API](src/lib.rs) | Exported node and facility types. |

```sh
cargo test -p cellule-host --locked
```
