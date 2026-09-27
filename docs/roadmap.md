# Framework scope and next gaps

The current implementation follows the Crab revision recorded in
[the synthesis ledger](synthesis.md). The supported author primitives are SQL,
KV, Blob, Queue, Cron, Workflow, Activities, and Effects. They share one actor,
transaction/publication path, bounded maintenance, and exact-root recovery.

Current fleet mechanics include signed node enrollment, owner fencing,
resource admission, pressure-driven shedding, immutable read replicas, fenced
warm promotion, and host-owned recruitment and shutdown. The optional peer HTTP
adapter supplies transport and mTLS pinning; the embedding service owns its
receiver, authorization, credentials, and rollout policy.

Next work must be driven by a concrete application and qualified against these
boundaries:

- Integrate any requested Timer, Projection, or hosted Queue consumer capability
  with the current registry, capacity checks, maintenance, and host lifecycle.
  These former Cellule-only APIs were superseded; they are not current support.
- Qualify real providers and constrained fleets against the extracted workspace,
  including interrupted publication, owner loss, reader replacement, and rollout.
- Qualify upgrades from any existing Cellule storage prefix before promising
  data or signed-peer compatibility across the synthesis boundary.
- Release the matched crate set only after packaging and the full verification
  gate pass. Keep LTX licenses and external-vector provenance with distributions.

See [architecture](architecture.md), [embedding](embedding.md),
[quickstart](quickstart.md), and [qualification](../crates/cellule-runtime/qualification/README.md)
for the implemented contracts and executable evidence.
