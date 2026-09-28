//! Compile two Cell types, then use KV and Queue through typed handles.
//!
//! Run: `cargo run -p cellule-app --example basic --locked`
//!
//!   BasicApp -> descriptor: register Settings (KV) and Jobs (Queue)
//!   Local owner -> two Cells: provision, bootstrap, bind typed handles
//!   KV Cell: atomic put -> receipt -> read at receipt
//!   Queue Cell: send -> claim -> validate lease -> acknowledge
//!
//! The KV receipt proves the read sees the published value. Queue delivery is
//! at least once; real consumers perform idempotent external work between
//! lease validation and acknowledgement. `sql.rs` demonstrates SQL and
//! `blob.rs` demonstrates Blob. This uses local fixtures, not service ingress.

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
use cellule_runtime::primitives::kv::{
    KvAtomicRequest, KvModule, KvMutation, install_kv_schema, register_kv,
};
use cellule_runtime::primitives::maintenance::MaintenanceModule;
use cellule_runtime::primitives::queue::{
    QueueClaimRequest, QueueLeaseOutcome, QueueModule, QueueSendRequest, install_queue_schema,
    register_queue,
};
use cellule_runtime::registry::OperationDescriptor;
use cellule_runtime::{
    ApplicationId, BuildDescriptor, CatalogRole, CellClient, CellModule, CellRuntime, CellTarget,
    Digest, Error, MigrationDescriptor, ModuleDescriptor, MutationIdentity, NamespaceDescriptor,
    NamespaceId, RegistryBuilder, SessionId, SqlWorkerPool, TenantId, partition_for_shard,
};
use cellule_store::Store;
use object_store::{memory::InMemory, path::Path};

type ExampleResult<T> = Result<T, Box<dyn std::error::Error>>;

const SETTINGS_NAMESPACE: NamespaceId = NamespaceId::from_bytes([2; 16]);
const JOBS_NAMESPACE: NamespaceId = NamespaceId::from_bytes([3; 16]);
const MIGRATION: &str = "-- basic example module schema v1";
const KV_COMMANDS: [OperationDescriptor; 1] = [operation(1, 4 * 1024 * 1024 + 64 * 1024)];
const KV_QUERIES: [OperationDescriptor; 2] = [
    operation(2, 4 * 1024 * 1024 + 64 * 1024),
    operation(3, 4 * 1024 * 1024 + 64 * 1024),
];
const QUEUE_COMMANDS: [OperationDescriptor; 5] = [
    operation(1, 1 << 20),
    operation(2, 1 << 20),
    operation(3, 1 << 20),
    operation(5, 1 << 20),
    operation(7, 1 << 20),
];
const QUEUE_QUERIES: [OperationDescriptor; 2] = [operation(4, 1 << 20), operation(6, 1 << 20)];

const fn operation(id: u32, limit: u32) -> OperationDescriptor {
    OperationDescriptor {
        id,
        codec_version: 1,
        schema_min: 1,
        schema_max: 1,
        input_limit: limit,
        output_limit: limit,
    }
}

fn migrations() -> &'static [MigrationDescriptor; 1] {
    static MIGRATIONS: OnceLock<[MigrationDescriptor; 1]> = OnceLock::new();
    MIGRATIONS.get_or_init(|| {
        [MigrationDescriptor {
            version: 1,
            sql: MIGRATION,
            digest: Digest::from_bytes(*blake3::hash(MIGRATION.as_bytes()).as_bytes()),
        }]
    })
}

// A module declares stable IDs and binds the primitive's typed operations.
struct Settings;

impl KvModule for Settings {
    const MODULE: &'static str = Self::NAME;
    const ATOMIC_COMMAND_ID: u32 = 1;
    const GET_QUERY_ID: u32 = 2;
    const LIST_QUERY_ID: u32 = 3;
}

impl CellModule for Settings {
    const NAME: &'static str = "settings";

    fn descriptor(&self) -> &'static ModuleDescriptor {
        static DESCRIPTOR: OnceLock<ModuleDescriptor> = OnceLock::new();
        DESCRIPTOR.get_or_init(|| ModuleDescriptor {
            name: Self::NAME,
            source_digest: Digest::from_bytes(*blake3::hash(include_bytes!("basic.rs")).as_bytes()),
            retained_codes: &[],
            schema_min: 1,
            schema_max: 1,
            migrations: migrations(),
            commands: &KV_COMMANDS,
            queries: &KV_QUERIES,
            workflow_definitions: &[],
            activity_types: &[],
            namespaces: &[NamespaceDescriptor {
                id: SETTINGS_NAMESPACE,
                name: Self::NAME,
                role: CatalogRole::Kv,
                shards: 1,
                effect_targets: &[],
                dead_letter: None,
            }],
        })
    }

    fn register(self, registry: &mut RegistryBuilder) -> cellule_runtime::Result<()> {
        register_kv::<Self>(registry)
    }
}

struct Jobs;

impl MaintenanceModule for Jobs {
    const MODULE: &'static str = Self::NAME;
    const TICK_COMMAND_ID: u32 = 7;
}

impl QueueModule for Jobs {
    const NAMESPACE: NamespaceId = JOBS_NAMESPACE;
    const SEND_COMMAND_ID: u32 = 1;
    const CLAIM_COMMAND_ID: u32 = 2;
    const LEASE_COMMAND_ID: u32 = 3;
    const VALIDATE_QUERY_ID: u32 = 4;
    const CONTROL_COMMAND_ID: u32 = 5;
    const INFO_QUERY_ID: u32 = 6;
}

impl CellModule for Jobs {
    const NAME: &'static str = "jobs";

    fn descriptor(&self) -> &'static ModuleDescriptor {
        static DESCRIPTOR: OnceLock<ModuleDescriptor> = OnceLock::new();
        DESCRIPTOR.get_or_init(|| ModuleDescriptor {
            name: Self::NAME,
            source_digest: Digest::from_bytes(*blake3::hash(include_bytes!("basic.rs")).as_bytes()),
            retained_codes: &[],
            schema_min: 1,
            schema_max: 1,
            migrations: migrations(),
            commands: &QUEUE_COMMANDS,
            queries: &QUEUE_QUERIES,
            workflow_definitions: &[],
            activity_types: &[],
            namespaces: &[NamespaceDescriptor {
                id: JOBS_NAMESPACE,
                name: Self::NAME,
                role: CatalogRole::Queue,
                shards: 1,
                effect_targets: &[],
                dead_letter: None,
            }],
        })
    }

    fn register(self, registry: &mut RegistryBuilder) -> cellule_runtime::Result<()> {
        register_queue::<Self>(registry)
    }
}

struct BasicApp;

impl CellApplication for BasicApp {
    const NAME: &'static str = "basic-example";

    fn register(builder: &mut cellule_app::ApplicationBuilder) -> cellule_runtime::Result<()> {
        builder.register(Settings)?;
        builder.register(Jobs)?;
        builder.cell_type(CellType::new(
            Settings::NAME,
            "settings",
            SETTINGS_NAMESPACE,
            CatalogRole::Kv,
            1,
        )?)?;
        builder.cell_type(CellType::new(
            Jobs::NAME,
            "jobs",
            JOBS_NAMESPACE,
            CatalogRole::Queue,
            1,
        )?)?;
        Ok(())
    }
}

fn identity(request_id: u8, now_ms: i64) -> MutationIdentity {
    MutationIdentity {
        request_id: RequestId::from_bytes([request_id; 16]),
        issued_at_ms: now_ms,
        expires_at_ms: now_ms + 60_000,
    }
}

async fn use_primitives(typed: &ApplicationHandle<BasicApp>) -> ExampleResult<()> {
    let now_ms = i64::try_from(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())?;

    // KV checks and writes one scope atomically; the receipt gates the read.
    let settings = typed.kv::<Settings>(SETTINGS_NAMESPACE)?;
    let committed = settings
        .atomic(
            identity(10, now_ms),
            KvAtomicRequest {
                scope: b"account/42".to_vec(),
                checks: Vec::new(),
                mutations: vec![KvMutation::Put {
                    key: b"theme".to_vec(),
                    value: b"dark".to_vec(),
                    expires_at_ms: None,
                }],
            },
        )
        .await?;
    let observed = settings
        .get(
            b"account/42".to_vec(),
            b"theme".to_vec(),
            Some(committed.receipt),
        )
        .await?;
    let setting = observed
        .output
        .ok_or(Error::Control("the committed setting is missing"))?;
    if setting.value != b"dark" {
        return Err(Error::Control("the committed setting differs").into());
    }
    println!("setting theme: {}", String::from_utf8(setting.value)?);

    // Queue work is at least once: claim a lease, validate it, then ack it.
    let jobs = typed.queue::<Jobs>()?;
    jobs.send(
        identity(11, now_ms),
        QueueSendRequest {
            producer_id: [12; 16],
            payload: b"send-email".to_vec(),
            available_at_ms: now_ms,
        },
    )
    .await?;
    let claimed = jobs
        .claim(
            identity(13, now_ms),
            0,
            QueueClaimRequest {
                limit: 1,
                lease_ms: 5_000,
            },
        )
        .await?;
    let [message] = claimed.output.as_slice() else {
        return Err(Error::Control("expected one claimed job").into());
    };
    if !jobs
        .validate_claim(0, claimed.output.clone(), Some(claimed.receipt))
        .await?
        .output
    {
        return Err(Error::Control("the claimed job lease is invalid").into());
    }
    // A real worker performs idempotent external work here, before acking.
    let acknowledged = jobs
        .ack(identity(14, now_ms), 0, message.message_id, message.token)
        .await?;
    if !matches!(acknowledged.output, QueueLeaseOutcome::Applied { .. }) {
        return Err(Error::Control("the claimed job was not acknowledged").into());
    }
    println!("queue job: {}", String::from_utf8(message.payload.clone())?);
    Ok(())
}

#[tokio::main]
async fn main() -> ExampleResult<()> {
    let application = Arc::new(BasicApp::compile(BuildDescriptor {
        source_revision: "local-basic-example".into(),
        cargo_lock_digest: Digest::from_bytes(
            *blake3::hash(include_bytes!("../../../Cargo.lock")).as_bytes(),
        ),
    })?);
    println!(
        "compiled {} with {} cell types, digest {:?}",
        application.name(),
        application.cell_types().len(),
        application.descriptor_digest()
    );

    let tenant = TenantId::from_bytes([4; 16]);
    let application_id = ApplicationId::from_bytes([5; 16]);
    let session = SessionId::from_bytes([6; 16]);
    let layout = CellStorageLayout::new(
        Store::new(Arc::new(InMemory::new())),
        Path::from("basic-example"),
        *application_id.as_bytes(),
    );
    let registry = application.registry();
    let files = tempfile::TempDir::new()?;
    let runtime = CellRuntime::new_with_replica_host(
        SqlWorkerPool::new(2, 8)?,
        16 * 1024 * 1024,
        session,
        Host::default().with_local_disk_budget(DiskBudget::new(1 << 30)),
    )?;

    let result: ExampleResult<()> = async {
        let mut handles = Vec::new();
        for (module, namespace, role, owner_id, file) in [
            (
                "settings",
                SETTINGS_NAMESPACE,
                CatalogRole::Kv,
                7,
                "settings.sqlite",
            ),
            ("jobs", JOBS_NAMESPACE, CatalogRole::Queue, 8, "jobs.sqlite"),
        ] {
            let target =
                CellTarget::new(tenant, application_id, namespace, &partition_for_shard(0))?;
            let code = registry
                .module_code(module)
                .ok_or(Error::Registry("basic module is missing"))?;
            let proof = CellCatalog::new(layout.clone(), tenant)
                .provision(CatalogEntry::new(&target, role, code, 1)?)
                .await?;
            let authority = CellAuthority::new(layout.clone());
            let incarnation = IncarnationId::from_bytes([owner_id; 16]);
            let observed = authority
                .create_initial(
                    &proof,
                    incarnation,
                    Owner {
                        session,
                        endpoint: format!("https://{module}.local"),
                    },
                )
                .await?;
            let handle = runtime
                .bootstrap(
                    proof,
                    CellReplica::new(
                        layout.clone(),
                        *target.cell_id().as_bytes(),
                        *incarnation.as_bytes(),
                        Limits::default(),
                    )?,
                    authority,
                    observed,
                    files.path().join(file),
                    move |transaction| match role {
                        CatalogRole::Kv => install_kv_schema(transaction),
                        CatalogRole::Queue => install_queue_schema(transaction),
                        _ => Err(Error::Registry("unexpected basic Cell role")),
                    },
                )
                .await?;
            handles.push(handle);
        }
        let client = CellClient::local_many(registry, handles)?;
        let typed =
            ApplicationHandle::<BasicApp>::new(client, application, tenant, application_id)?;
        use_primitives(&typed).await
    }
    .await;
    // Drain on both success and failure, after all accepted work finishes.
    let shutdown = runtime.shutdown().await;
    result?;
    shutdown?;
    Ok(())
}
