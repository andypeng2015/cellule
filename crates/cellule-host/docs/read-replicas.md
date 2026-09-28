# Read replica lifecycle

```mermaid
flowchart LR
    Publish[Owner publishes] --> Hint[Bounded activation hint]
    Hint --> Select[Validate selected reader boot]
    Select --> View[Authenticated immutable view]
    Poll[Periodic reconciliation] --> Select
    Drain[Node drain] --> Cancel[Cancel activation and join work]
```

| Boundary | Behavior |
| --- | --- |
| Memory | Snapshot reservations are separate from writer memory. |
| Installation | Manager starts after the node task group; dispatcher uses the same resolver. |
| Recruitment | Locally owned Cells send scoped hints through an authorized peer client. |
| Refresh | New view must prove its root and receipt before selection. |
| Missing reader | Queries return a replica error; they do not trigger activation. |
| Drain | Cancels activation before closing views. |

Commands do not wait for readers. Reconciliation repairs dropped hints and
membership changes; it is not a freshness guarantee. See
[application read policies](../../cellule-app/docs/invocation.md).
