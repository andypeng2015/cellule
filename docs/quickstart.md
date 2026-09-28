# Run a Cellule application

Use Rust 1.97 or newer. The examples use temporary SQLite files and in-memory
object storage; no cloud credentials are required. Keep Cargo artifacts in a
checkout-specific target directory on the Workspace volume on workstations with the mounted Workspace volume.

## Define an application

```sh
cargo run -p cellule-app --example application_descriptor --locked
```

The [application descriptor example](../crates/cellule-app/examples/application_descriptor.rs)
declares a repository module, its SQL namespace and migration, and one Cell
type. `ApplicationBuilder` registers them and compiles a descriptor with a
stable digest. The example prints that digest and stops before starting a Cell.

## Commit and read an order

```sh
cargo run -p cellule-app --example orders --locked
```

The [orders example](../crates/cellule-app/examples/orders.rs) registers a SQL
module, publishes its catalog entry, bootstraps a managed Cell, commits an order
through a typed handle, and reads its total using the returned commit receipt.
It checks the published value, shuts down the runtime, and prints:

```text
order 42 total: 1999 cents
```

```mermaid
flowchart LR
    Register[Register SQL module] --> Provision[Publish catalog entry]
    Provision --> Bootstrap[Bootstrap Cell]
    Bootstrap --> Commit[Commit order]
    Commit --> Read[Read with receipt]
    Read --> Verify[Verify 1999 cents]
    Verify --> Shutdown[Shut down runtime]
```

## Upload and read an attachment

```sh
cargo run -p cellule-app --example attachments --locked
```

The [attachments example](../crates/cellule-app/examples/attachments.rs)
creates a Blob Cell and publishes an order receipt as one part. It begins the
upload, stores the part, completes publication, then reads the bytes and content
type at the completion receipt. It prints:

```text
attachment stored: receipt for order 42
```

## Exercise the application and recovery

```sh
cargo test -p cellule-app --test integration \
  primitives::typed_application_executes_every_primitive_through_a_local_router \
  --locked -- --exact --nocapture
```

The [application integration suite](../crates/cellule-app/tests/integration.rs)
exercises SQL, KV, Blob, Queue, Cron, Workflow, Activities, and Effects. Its
[primitive scenario](../crates/cellule-app/tests/primitives.rs)
checks writes and read-back results, destroys the original local files, restores
from published roots with a fenced successor owner, then verifies reads and new
writes. Its object store is in memory; this is local recovery evidence, not cloud
provider qualification.

The public-host tests cover signed peer routing, unknown outcomes, owner loss,
read replicas, promotion, and entity topology:

```sh
cargo test -p cellule-app --test integration host --locked
cargo test -p cellule-app --test integration entities --locked
```

Run the separate-process fleet smoke against an isolated RustFS bucket. Supply
`AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `CELLULE_TEST_ENDPOINT`,
`CELLULE_TEST_BUCKET`, and a unique `CELLULE_TEST_PREFIX` as described in the
[performance guide](../crates/cellule-app/PERFORMANCE.md). CI provisions its own
disposable RustFS service.

```sh
CELLULE_PERF_ITERATIONS=1 cargo test -p cellule-app --test integration \
  process_performance::reference_balanced_three_process_fleet_end_to_end_performance \
  --locked -- --ignored --exact --nocapture
```

Continue with the [Cellule API guide](api.md) for typed capabilities,
receipts, read policies, and outcome handling. See [embedding](embedding.md) for
serving-node startup and shutdown, and
[qualification](../crates/cellule-runtime/qualification/README.md) for the
provider and fault evidence required beyond these local tests.
