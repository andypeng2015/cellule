# Cellule

Cellule is an embedded Rust framework for distributed, SQLite-backed **Cells**.
A Cell has one fenced writer, records each command outcome with its state, and
can recover from a verified, authority-pinned root. Your application defines
the domain and owns ingress, authorization, credentials, and deployment policy.

![Cellule component boundaries: application policy, typed API, host lifecycle, runtime, SQLite, object storage, and authority](diagram/cellule-overview.svg)

| Term | Meaning |
| --- | --- |
| **Module** | Rust code that declares schema and typed operations. |
| **Cell type** | A descriptor that names a module, namespace, role, and shard count. |
| **Cell** | One provisioned state partition with a single fenced writer. |
| **Receipt** | A value returned with a durable command; use it to require that a later read observes that command. |

## Run your first Cell

Use Rust **1.97 or newer**. These examples use temporary SQLite files and an
in-memory object store, so they need no cloud account or credentials. Run them
from the repository root:

```sh
cargo run -p cellule-app --example application_descriptor --locked
cargo run -p cellule-app --example orders --locked
cargo run -p cellule-app --example attachments --locked
```

The first command compiles a module and Cell type into a stable application
descriptor; it does not start a Cell. The second provisions a SQL Cell, commits
order 42, and reads it back at the commit receipt. The third uploads a Blob
attachment and reads it at its completion receipt. The latter two print:

```text
order 42 total: 1999 cents
attachment stored: receipt for order 42
```

![First Cell path: describe, start, commit, and observe at a receipt](diagram/first-cell.svg)

The complete, runnable sources are
[application_descriptor.rs](crates/cellule-app/examples/application_descriptor.rs),
[orders.rs](crates/cellule-app/examples/orders.rs), and
[attachments.rs](crates/cellule-app/examples/attachments.rs). The
[quickstart](docs/quickstart.md) walks through each step and a local recovery
test.

## Define an application in Rust

A module owns its schema and operations. A `CellApplication` registers that
module and declares its Cell type. This is the registration from the compiled
[orders example](crates/cellule-app/examples/orders.rs); `Orders` and `ORDERS`
are defined immediately above it in that file:

```rust
struct OrdersApp;

impl CellApplication for OrdersApp {
    const NAME: &'static str = "orders-example";

    fn register(builder: &mut cellule_app::ApplicationBuilder) -> cellule_runtime::Result<()> {
        builder.register(Orders)?;
        builder.cell_type(CellType::new(
            Orders::NAME,
            "orders",
            ORDERS,
            CatalogRole::Sql,
            1,
        )?)?;
        Ok(())
    }
}
```

The module name, namespace ID, role, shard count, migrations, and operation
IDs become descriptor contracts. After compiling the application and starting
a Cell owner, `ApplicationHandle::<OrdersApp>` exposes a typed `SqlCell<Orders>`
handle. The [application guide](crates/cellule-app/docs/README.md) covers the
full authoring API and topology choices.

## Commit, then read at the receipt

This excerpt from the same [runnable orders example](crates/cellule-app/examples/orders.rs)
shows the application-facing command and query. `sql`, `now_ms`, and the order
constants come from that example. The request ID stays stable across retries;
the receipt asks the query to observe the published command:

```rust
let committed = sql
    .batch(
        MutationIdentity {
            request_id: RequestId::from_bytes([6; 16]),
            issued_at_ms: now_ms,
            expires_at_ms: now_ms + 60_000,
        },
        SqlBatch {
            statements: vec![SqlStatement {
                sql: "INSERT INTO orders (id, total_cents) VALUES (?1, ?2)".into(),
                parameters: vec![
                    SqlValue::Integer(ORDER_ID),
                    SqlValue::Integer(ORDER_TOTAL_CENTS),
                ],
            }],
        },
    )
    .await?;

let observed = sql
    .query(
        Some(committed.receipt),
        SqlBatch {
            statements: vec![SqlStatement {
                sql: "SELECT total_cents FROM orders WHERE id = ?1".into(),
                parameters: vec![SqlValue::Integer(ORDER_ID)],
            }],
        },
    )
    .await?;
```

The full example checks `observed.output` and drains the runtime on both
success and error paths. Use a **new request ID for each new logical command**;
reuse the same ID when retrying that command.

## What makes the reply durable?

![Durable command sequence: SQLite outcome, verified LTX root, authority CAS, then result and receipt](diagram/durable-command.svg)

The owner commits the mutation and its outcome in one SQLite transaction. On
the object-store path it prepares immutable, verified LTX bytes and pins the
exact root through the fenced authority compare-and-swap before replying. A
configured follower-log path can acknowledge after a recoverable follower
proof, with object publication following. Recovery uses only the pinned root
and verifies every required chunk; a bucket listing or stale local database
cannot choose state. Read the [runtime](crates/cellule-runtime/docs/runtime.md),
[storage](crates/cellule-runtime/docs/storage.md), and
[failover](crates/cellule-runtime/docs/failover-and-followers.md) guides for the
precise protocols.

## Find your layer

| Layer | Crate | Start here |
| --- | --- | --- |
| Identity | [cellule-types](crates/cellule-types/README.md) | [Identity guide](crates/cellule-types/docs/README.md) |
| Transport | [cellule-store](crates/cellule-store/README.md) | [Store guide](crates/cellule-store/docs/README.md) |
| Persistence | [cellule-ltx](crates/cellule-ltx/README.md) | [LTX guide](crates/cellule-ltx/docs/README.md) |
| Execution | [cellule-runtime](crates/cellule-runtime/README.md) | [Runtime guide](crates/cellule-runtime/docs/README.md) |
| Application | [cellule-app](crates/cellule-app/README.md) | [Author guide](crates/cellule-app/docs/README.md) |
| Lifecycle | [cellule-host](crates/cellule-host/README.md) | [Host guide](crates/cellule-host/docs/README.md) |
| Optional peer adapter | [cellule-peer-http](crates/cellule-peer-http/README.md) | [Peer guide](crates/cellule-peer-http/docs/README.md) |

Dependencies point down: `host → app → runtime → ltx → store → types`. The
optional peer adapter uses runtime contracts; the application still owns HTTP
endpoints and authorization. See [architecture](docs/architecture.md) for
ownership boundaries and persisted formats, and [embedding](docs/embedding.md)
for serving-node startup and shutdown.

## Verify and contribute

Run the local recovery scenario after the examples:

```sh
cargo test -p cellule-app --test integration \
  primitives::typed_application_executes_every_primitive_through_a_local_router \
  --locked -- --exact
```

The [contributor checks](CONTRIBUTING.md) cover formatting, targets, docs,
and contract validation. The [qualification guide](crates/cellule-runtime/docs/delivery.md)
separates local proof from provider and production evidence. See the
[release guide](docs/releasing.md) and [LTX attribution](crates/cellule-ltx/UPSTREAM.md)
for publication details. The [synthesis record](docs/synthesis.md),
[verification report](docs/verification.md), and
[dated performance evidence](crates/cellule-app/PERFORMANCE.md) describe their
recorded revisions and environments.
