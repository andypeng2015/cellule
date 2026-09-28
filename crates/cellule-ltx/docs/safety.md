# Safety, failure classes, and limits

LTX CRC64 protects file structure and rolling database state; it is not an
authenticator. Cell roots and objects carry BLAKE3 digests. The embedding
service authenticates the authority record that selects a root.

| Invariant | Effect |
| --- | --- |
| First segment is a full snapshot | No delta-only restore. |
| Ordered, contiguous checksum chain | No gap or reordered mutation. |
| Declared size, hash, page coverage, checksums verified | Corrupt bytes fail closed. |
| Fresh restore destination | Existing state is never overwritten. |
| Fenced capture state | No acknowledgement after an unprovable cut. |
| Cancellation of dispatched work | Caller awaits or reconciles; it does not assume rollback. |

`LtxError::classify()` is the caller contract:

| Class | Caller response |
| --- | --- |
| `Retryable { after }` | Retry within own budget and honor provider delay. |
| `Capacity` | Reduce or release resources; side effect was refused. |
| `Permanent` | Change inputs or selected state. |
| `Ambiguous` | Reconcile before retrying. |
| `Fenced` | Close and restore authoritative state. |

`Limits` bounds file, capture, and database size. `DiskBudget` and host resource
admission bound local scratch, retained cuts, and remote I/O. The host owns
scheduling and cancellation; see [cellule-host](../../cellule-host/docs/README.md).

```sh
cargo test -p cellule-ltx --no-default-features --locked
cargo test -p cellule-ltx --features replica --locked
```

External decoder vectors and fuzz entry points live under
[`tests/vectors`](../tests/vectors/README.md) and [`fuzz`](../fuzz/README.md).
