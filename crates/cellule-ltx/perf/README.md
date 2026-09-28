# LTX performance harnesses

These harnesses measure local capture and Cell-root costs. Results depend on
payload entropy, provider, hardware, cache state, and activation history; no
number here is a production SLO.

| Harness | Measures | Entry |
| --- | --- | --- |
| Local comparison | Matched capture/restore against the Celld lineage. | [`run.sh`](run.sh) |
| Cell publication | Root preparation, provider calls, sparse activation, and compaction. | [`replica-cost`](replica-cost/) |
| Historical measurements | Dated experiments and qualification limits. | [September 2026 record](history/2026-09-benchmark-notes.md) |

```mermaid
flowchart LR
    Workload[Fixed workload] --> SQLite[SQLite commit]
    SQLite --> Capture[LTX capture]
    Capture --> Objects[Immutable objects]
    Objects --> Root[Prepared root]
    Root --> Report[Time, bytes, provider calls]
```

Use a unique `CARGO_TARGET_DIR` on the mounted workspace volume. The
[Cellule LTX guide](../docs/README.md) defines correctness; a faster root
proposal does not change the authority CAS required for acknowledgement.
