# LTX guide

| Read | For |
| --- | --- |
| [Local capture](capture.md) | Managed writer, committed cuts, and checkpoints. |
| [Root publication](publication.md) | Immutable proposal and Cell authority boundary. |
| [Recovery](recovery.md) | Exact restore, sparse activation, and continuation. |
| [Safety and limits](safety.md) | Verification, error classes, and resource ownership. |
| [Crate entry](../README.md) | Runnable local round trip. |
| [Provenance](../UPSTREAM.md) | Celld ancestry and licenses. |

LTX does not choose a Cell owner. [`cellule-runtime`](../../cellule-runtime/docs/README.md)
selects an exact root through authority CAS.
