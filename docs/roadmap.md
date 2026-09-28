# Framework scope and roadmap

This page separates implemented framework behavior from work that still needs
application integration or measured qualification. It is a planning map, not
a promise of a hosted service or a release date.

## Implemented contracts

| Area | Current framework surface | Where to start |
| --- | --- | --- |
| Authoring | SQL, KV, Blob, Queue, Cron, Workflow, Activities, and Effects through typed application handles. | [API guide](api.md) and [primitives](../crates/cellule-runtime/docs/primitives.md) |
| Cell execution | One fenced writer, durable request outcomes, receipt-bound reads, and bounded admission. | [Architecture](architecture.md) |
| Persistence | Managed SQLite, immutable LTX history, exact-root publication, and verified recovery. | [LTX crate](../crates/cellule-ltx/README.md) |
| Fleet lifecycle | Signed enrollment, owner fencing, resource admission, reader replacement, warm promotion, drain, and shutdown. | [Framework integration](framework.md) |
| Optional peer transport | Runtime-contract HTTP adapter with pinned mTLS; applications own endpoints and authorization. | [Peer adapter](../crates/cellule-peer-http/README.md) |

The [quickstart](quickstart.md) exercises a local application path. Local
success is evidence for that path and environment; it does not establish cloud
provider, scale, fault, or upgrade behavior.

## Work to qualify before broader adoption

1. **Real providers and constrained fleets.** Run conditional object writes,
   range reads, multipart behavior, bounded Cell populations, and pressure
   shedding under measured provider and resource limits. Keep raw logs and
   bind them to the source, image, and profile digest.
2. **Failure and rollout paths.** Exercise interrupted publication, ambiguous
   replies, owner loss, reader replacement, fenced promotion, and shutdown
   under the [fault profiles](../crates/cellule-runtime/qualification/README.md).
3. **Compatibility.** Demonstrate upgrade and rollback behavior for any
   deployed storage prefix or signed peer population before claiming a
   compatible rollout. Persisted identities, paths, roots, and messages are
   contracts; no local API test can substitute for an upgrade run.
4. **Matched crate release.** Complete the verification and packaging gates,
   then publish the seven-crate set in dependency order using the
   [release guide](releasing.md).

Additional Timer, Projection, or hosted Queue consumer APIs should follow a
concrete application need. Any new capability must integrate with the current
registry, owner admission, maintenance budget, and host lifecycle; former APIs
from earlier codebases are not part of today's supported surface.

For proof levels and receipt requirements, read the
[delivery evidence guide](../crates/cellule-runtime/docs/delivery.md).
