# Topology and descriptors

```mermaid
flowchart TD
    Namespace[Stable namespace ID] --> CellType[CellType]
    Role[Catalog role] --> CellType
    Partition[Fixed shard or entity key] --> CellType
    CellType --> Descriptor[Compiled descriptor digest]
    Descriptor --> Routing[Typed client routing]
```

| Surface | Contract |
| --- | --- |
| `CellType::new` | Declares module, name, namespace, role, and shard count. |
| Fixed shards | Scope hashes to one declared shard. |
| `with_entity_partitions` | One declared shard; canonical entity key derives a 33-byte partition. |
| `with_schema_range` | Declares an accepted schema interval. |
| `with_limits` | Bounds database and capture sizes per Cell. |
| `ApplicationBuilder` | Freezes module registry and topology into a descriptor. |

Changing a stable namespace, role, partition scheme, or descriptor changes
routing and persisted identity. Treat it as a versioned application change.
The [authoring example](../examples/authoring.rs) compiles one descriptor and
prints its digest.
