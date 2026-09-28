//! Commit an order, then read it at the returned durable receipt.

use std::{
    sync::{Arc, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

use cellule_app::{ApplicationHandle, CellApplication, CellType};
use cellule_ltx::{CellReplica, DiskBudget, Host, Limits};
use cellule_runtime::cell::catalog::{CatalogEntry, CellCatalog};
use cellule_runtime::control::{Owner, authority::CellAuthority};
use cellule_runtime::identity::{IncarnationId, RequestId};
use cellule_runtime::ltx::CellStorageLayout;
use cellule_runtime::primitives::sql::{SqlBatch, SqlCell, SqlStatement, SqlValue, register_sql};
use cellule_runtime::registry::OperationDescriptor;
use cellule_runtime::{
    ApplicationId, BuildDescriptor, CatalogRole, CellClient, CellModule, CellRuntime, CellTarget,
    Digest, Error, MigrationDescriptor, ModuleDescriptor, MutationIdentity, NamespaceDescriptor,
    NamespaceId, RegistryBuilder, SessionId, SqlModule, SqlWorkerPool, TenantId,
    partition_for_shard,
};
use cellule_store::Store;
use object_store::{memory::InMemory, path::Path};

const ORDERS: NamespaceId = NamespaceId::from_bytes([1; 16]);
const SCHEMA: &str = "CREATE TABLE orders (id INTEGER PRIMARY KEY, total_cents INTEGER NOT NULL)";
const ORDER_ID: i64 = 42;
const ORDER_TOTAL_CENTS: i64 = 1_999;
const COMMANDS: [OperationDescriptor; 1] = [operation(1)];
const QUERIES: [OperationDescriptor; 1] = [operation(2)];
type ExampleResult<T> = Result<T, Box<dyn std::error::Error>>;

struct Orders;

impl SqlModule for Orders {
    const MODULE: &'static str = Self::NAME;
    const BATCH_COMMAND_ID: u32 = 1;
    const BATCH_QUERY_ID: u32 = 2;
}

impl CellModule for Orders {
    const NAME: &'static str = "orders";

    fn descriptor(&self) -> &'static ModuleDescriptor {
        static MIGRATIONS: OnceLock<[MigrationDescriptor; 1]> = OnceLock::new();
        static DESCRIPTOR: OnceLock<ModuleDescriptor> = OnceLock::new();
        DESCRIPTOR.get_or_init(|| ModuleDescriptor {
            name: Self::NAME,
            source_digest: Digest::from_bytes(
                *blake3::hash(include_bytes!("orders.rs")).as_bytes(),
            ),
            retained_codes: &[],
            schema_min: 1,
            schema_max: 1,
            migrations: MIGRATIONS.get_or_init(|| {
                [MigrationDescriptor {
                    version: 1,
                    sql: SCHEMA,
                    digest: Digest::from_bytes(*blake3::hash(SCHEMA.as_bytes()).as_bytes()),
                }]
            }),
            commands: &COMMANDS,
            queries: &QUERIES,
            workflow_definitions: &[],
            activity_types: &[],
            namespaces: &[NamespaceDescriptor {
                id: ORDERS,
                name: Self::NAME,
                role: CatalogRole::Sql,
                shards: 1,
                effect_targets: &[],
                dead_letter: None,
            }],
        })
    }

    fn register(self, registry: &mut RegistryBuilder) -> cellule_runtime::Result<()> {
        register_sql::<Self>(registry)
    }
}

const fn operation(id: u32) -> OperationDescriptor {
    OperationDescriptor {
        id,
        codec_version: 1,
        schema_min: 1,
        schema_max: 1,
        input_limit: 1024,
        output_limit: 1024,
    }
}

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

async fn commit_and_read_order(sql: &SqlCell<Orders>) -> ExampleResult<i64> {
    let now_ms = i64::try_from(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())?;
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

    // The receipt requires the query to observe this published command.
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
    let [result] = observed.output.as_slice() else {
        return Err(Error::Control("expected one order query result").into());
    };
    let [row] = result.rows.as_slice() else {
        return Err(Error::Control("expected one order row").into());
    };
    let [SqlValue::Integer(total_cents)] = row.as_slice() else {
        return Err(Error::Control("expected an integer order total").into());
    };
    if *total_cents != ORDER_TOTAL_CENTS {
        return Err(Error::Control("published order total differs").into());
    }
    Ok(*total_cents)
}

#[tokio::main]
async fn main() -> ExampleResult<()> {
    let application = Arc::new(OrdersApp::compile(BuildDescriptor {
        source_revision: "local-orders-example".into(),
        cargo_lock_digest: Digest::from_bytes(
            *blake3::hash(include_bytes!("../../../Cargo.lock")).as_bytes(),
        ),
    })?);
    let tenant = TenantId::from_bytes([2; 16]);
    let application_id = ApplicationId::from_bytes([3; 16]);
    let target = CellTarget::new(tenant, application_id, ORDERS, &partition_for_shard(0))?;
    let store = Store::new(Arc::new(InMemory::new()));
    let layout = CellStorageLayout::new(
        store,
        Path::from("orders-example"),
        *application_id.as_bytes(),
    );
    let registry = application.registry();
    let code = registry
        .module_code(Orders::NAME)
        .ok_or(Error::Registry("orders module is missing"))?;
    // Publish the catalog entry and fenced owner before bootstrapping the Cell.
    let proof = CellCatalog::new(layout.clone(), tenant)
        .provision(CatalogEntry::new(&target, CatalogRole::Sql, code, 1)?)
        .await?;
    let authority = CellAuthority::new(layout.clone());
    let incarnation = IncarnationId::from_bytes([4; 16]);
    let session = SessionId::from_bytes([5; 16]);
    let observed = authority
        .create_initial(
            &proof,
            incarnation,
            Owner {
                session,
                endpoint: "https://orders.local".into(),
            },
        )
        .await?;
    let files = tempfile::TempDir::new()?;
    let runtime = CellRuntime::new_with_replica_host(
        SqlWorkerPool::new(1, 4)?,
        16 * 1024 * 1024,
        session,
        Host::default().with_local_disk_budget(DiskBudget::new(1 << 30)),
    )?;
    let result: ExampleResult<i64> = async {
        let handle = runtime
            .bootstrap(
                proof,
                CellReplica::new(
                    layout,
                    *target.cell_id().as_bytes(),
                    *incarnation.as_bytes(),
                    Limits::default(),
                )?,
                authority,
                observed,
                files.path().join("orders.sqlite"),
                |transaction| {
                    transaction.execute_batch(SCHEMA)?;
                    Ok(())
                },
            )
            .await?;
        let client = CellClient::local(registry, handle);
        let typed =
            ApplicationHandle::<OrdersApp>::new(client, application, tenant, application_id)?;
        let sql = typed.sql::<Orders>(target)?;
        commit_and_read_order(&sql).await
    }
    .await;
    // Drain the runtime on both the success and error paths.
    let shutdown = runtime.shutdown().await;
    let total_cents = result?;
    shutdown?;
    println!("order {ORDER_ID} total: {total_cents} cents");
    Ok(())
}
