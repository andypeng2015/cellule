# Run a Cellule application

This path starts with a descriptor, then runs two local Cells and one recovery
test. Use Rust **1.97 or newer** and run commands from the workspace root. The
examples use temporary SQLite files and in-memory object storage; no cloud
credentials are required. On a workstation with the mounted Workspace volume,
put `CARGO_TARGET_DIR` under `$HOME/Workspace/crabbuild-target` and give each
checkout its own directory.

| Step | Run | What you should observe |
| --- | --- | --- |
| 1 | `application_descriptor` | A compiled application name, one Cell type, and descriptor digest. |
| 2 | `orders` | Order 42 is committed and read back as 1999 cents. |
| 3 | `attachments` | A Blob receipt is uploaded and read back. |
| 4 | Focused integration test | All eight primitives survive a local owner recovery. |

## 1. Compile an application descriptor

```sh
cargo run -p cellule-app --example application_descriptor --locked
```

The [source](../crates/cellule-app/examples/application_descriptor.rs)
registers a `Repository` module, its SQL namespace and schema migration, and a
`CellType`. `ApplicationBuilder::finish` checks the module/topology match and
prints the descriptor digest. No Cell or SQLite worker starts in this step.

The descriptor is more than a display name: stable namespace, role, shard
count, migration versions, operation IDs, and source/lockfile digests bind the
binary to its persisted state. Read [topology](../crates/cellule-app/docs/topology.md)
before changing a released declaration.

## 2. Commit and read an order

```sh
cargo run -p cellule-app --example orders --locked
```

Expected application output:

```text
order 42 total: 1999 cents
```

The [runnable source](../crates/cellule-app/examples/orders.rs) follows the
full local path:

1. `Orders` declares a SQL schema and fixed command/query IDs; `OrdersApp`
   compiles the module and one SQL Cell type.
2. The example builds a `CellTarget`, in-memory `Store`, and
   `CellStorageLayout`, then provisions the catalog entry and initial fenced
   owner.
3. `CellRuntime::bootstrap` opens managed SQLite and installs the schema.
   `ApplicationHandle::<OrdersApp>` binds a local `CellClient` to tenant and
   application identity, then returns `SqlCell<Orders>`.
4. `SqlCell::batch` writes the order with a stable `MutationIdentity`. The
   command output carries a receipt after durable publication.
5. `SqlCell::query(Some(committed.receipt), ...)` reads at or beyond that
   position. The example checks that the row contains `1999` and drains the
   runtime even if the operation fails.

```mermaid
flowchart LR
    Register[Register module and Cell type] --> Provision[Provision catalog and owner]
    Provision --> Bootstrap[Bootstrap managed SQLite]
    Bootstrap --> Commit[Commit order]
    Commit --> Read[Query at receipt]
    Read --> Verify[Check row and shut down]
```

The fixed IDs and local endpoint in this example are fixtures. In a service,
use stable application IDs and distinct request IDs for distinct logical
commands. Reuse an identity only to resolve or retry the **same** command.

## 3. Upload and read an attachment

```sh
cargo run -p cellule-app --example attachments --locked
```

Expected application output:

```text
attachment stored: receipt for order 42
```

The [Blob source](../crates/cellule-app/examples/attachments.rs) provisions a
Blob Cell and adds a `BlobArtifactStore` to its application handle. It sends
`Begin`, `PutPart`, and `Complete` as separately identified mutations. The
completion receipt gates a `BlobQuery::Read`; the example checks the returned
bytes and content type. Staging a part alone does not publish an attachment.

## 4. Exercise all primitives and recovery

```sh
cargo test -p cellule-app --test integration \
  primitives::typed_application_executes_every_primitive_through_a_local_router \
  --locked -- --exact --nocapture
```

The [scenario](../crates/cellule-app/tests/primitives.rs) executes SQL, KV,
Blob, Queue, Cron, Workflow, Activities, and Effects through typed handles.
It checks read-back results, removes the original local SQLite files, restores
from published roots under a fenced successor owner, and checks reads and new
writes. Its object store is in memory. This is local application and recovery
evidence, not cloud-provider qualification.

For narrower host behavior, run the named integration groups:

```sh
cargo test -p cellule-app --test integration host --locked
cargo test -p cellule-app --test integration entities --locked
```

These cover signed peer routing, ambiguous outcomes, owner loss, read replicas,
promotion, and entity partitioning. Use an isolated CI or verification snapshot
for broad or process suites as described in [CONTRIBUTING](../CONTRIBUTING.md).

## Continue beyond local fixtures

| Goal | Next guide |
| --- | --- |
| Pick a primitive and handle its result | [API](api.md) and [primitive behavior](../crates/cellule-runtime/docs/primitives.md) |
| Understand ownership and recovery | [Architecture](architecture.md) |
| Add Cellule to a serving application | [Framework integration](framework.md) |
| Run provider and process evidence | [Qualification](../crates/cellule-runtime/qualification/README.md) and [performance scenarios](../crates/cellule-app/PERFORMANCE.md) |

The ignored three-process RustFS smoke requires a disposable bucket, explicit
credentials, and a unique prefix. Its environment and exact selector live in
the [performance guide](../crates/cellule-app/PERFORMANCE.md); it is a separate
qualification step rather than a prerequisite for this quickstart.
