# Application qualification and performance

The reference application tests typed writes, receipt-bound reads, recovery,
peer routing, reader recruitment, rollout, and process fleets. Run performance
scenarios only against a disposable bucket and a fresh evidence directory.

| Scenario | Entry | Evidence |
| --- | --- | --- |
| Local typed action | [Quickstart](../../docs/quickstart.md) | Write, publish, read-back. |
| Three-process RustFS | [`qualification/run.sh`](qualification/run.sh) | Local and forwarded gateway calls, receipts, drained sessions. |
| Entity fleet and scaling | [`qualification/entities.py`](qualification/entities.py), [`scale.py`](qualification/scale.py) | Isolated Cell ledgers and bounded traffic. |
| Reader and rollout variants | [Reference integration suite](tests/reference_application.rs) | Selection, replacement, and recovered receipts. |
| Dated measurements | [Performance reports](performance/) | Hardware/profile-specific provenance. |
| Original qualification notes | [September 2026 record](performance/2026-09-27-qualification-notes.md) | Historical commands and limits. |

```mermaid
flowchart LR
    Action[Typed action] --> Gateway[Gateway node]
    Gateway --> Owner[Owner Cell]
    Owner --> Store[Durable root]
    Store --> Read[Receipt-bound read]
    Read --> Report[Integrity and latency evidence]
```

Set `CELLULE_TEST_ENDPOINT`, `CELLULE_TEST_BUCKET`, and a unique
`CELLULE_TEST_PREFIX`; supply matching provider credentials. The
[qualification guide](../cellule-runtime/docs/delivery.md) distinguishes local
correctness from provider, fleet, and production evidence. Historical
measurements do not establish current capacity.
