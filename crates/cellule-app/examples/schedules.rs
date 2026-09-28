//! Register a Cron schedule and deliver one due occurrence through Effects.
//!
//! Run: `cargo run -p cellule-app --example schedules --locked`
//!
//!   Application -> Cron Cell: upsert fixed-interval schedule
//!   Application -> Cron Cell: read schedule at the returned receipt
//!   Maintenance Tick -> Cron Cell: fire due occurrence + durable effect intent
//!   EffectSupervisor -> Receiver Cell: signed, inbox-deduplicated delivery
//!   Application -> Receiver Cell: read one recorded reminder
//!
//! This example drives one Tick and one effect cycle explicitly over an
//! in-process signed peer loopback. A serving application owns the schedule
//! scanner, transport, authorization, and supervisors; compiling a schedule
//! alone never starts an independent timer.

mod support;

use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

use cellule_app::{ApplicationHandle, CellApplication, CellType};
use cellule_runtime::cell::actor::CellHandle;
use cellule_runtime::peer::{
    EffectPeerClient, PeerAuthorizer, PeerCellResolver, PeerDispatcher, PeerPrincipal,
    PeerRoundTrip, PeerSigner, PeerVerifier, VerifiedPeerRequest,
};
use cellule_runtime::primitives::cron::{
    CronInvocation, CronModule, CronMutation, CronQueryResult, CronTarget, install_cron_schema,
    register_cron,
};
use cellule_runtime::primitives::effects::{
    EffectModule, EffectRunOutcome, EffectSupervisor, register_effect_delivery,
};
use cellule_runtime::primitives::maintenance::{
    MaintenanceModule, MaintenanceTickCommand, MaintenanceTickOutcome, MaintenanceTickRequest,
};
use cellule_runtime::primitives::sql::{SqlBatch, SqlModule, SqlStatement, SqlValue, register_sql};
use cellule_runtime::registry::{Command, CommandContext, CommandResult, OperationDescriptor};
use cellule_runtime::{
    ApplicationId, BuildDescriptor, CatalogRole, CellClient, CellModule, CellTarget, Digest, Error,
    MigrationDescriptor, ModuleDescriptor, NamespaceDescriptor, NamespaceId, RegistryBuilder,
    SessionId, TenantId, partition_for_shard,
};
use ed25519_dalek::SigningKey;

use support::{ExampleResult, LocalCells, identity, now_ms};

const SCHEDULES: NamespaceId = NamespaceId::from_bytes([81; 16]);
const RECEIVER: NamespaceId = NamespaceId::from_bytes([82; 16]);
const SCHEDULE_MIGRATION: &str = "-- schedules example schema v1";
const RECEIVER_SCHEMA: &str = "CREATE TABLE reminders (schedule BLOB NOT NULL, occurrence INTEGER NOT NULL, payload BLOB NOT NULL, PRIMARY KEY(schedule, occurrence))";
const SCHEDULE_COMMANDS: [OperationDescriptor; 4] =
    [operation(1), operation(3), operation(4), operation(5)];
const SCHEDULE_QUERIES: [OperationDescriptor; 3] = [operation(2), operation(6), operation(7)];
const RECEIVER_COMMANDS: [OperationDescriptor; 2] = [operation(1), operation(2)];
const RECEIVER_QUERIES: [OperationDescriptor; 1] = [operation(3)];

const fn operation(id: u32) -> OperationDescriptor {
    OperationDescriptor {
        id,
        codec_version: 1,
        schema_min: 1,
        schema_max: 1,
        input_limit: 1 << 20,
        output_limit: 1 << 20,
    }
}

// The receiver contract is compiled with the schedule. Its command would run
// under the destination Cell's own transaction when an effect is delivered.
struct RecordReminder;

impl Command for RecordReminder {
    const MODULE: &'static str = "receiver";
    const ID: u32 = 1;
    const CODEC_VERSION: u32 = 1;
    type Input = CronInvocation;
    type Output = ();

    fn execute(
        context: &mut CommandContext<'_, '_>,
        input: Self::Input,
    ) -> cellule_runtime::Result<CommandResult<Self::Output>> {
        context.sql(&cellule_runtime::primitives::sql::SqlBatch {
            statements: vec![cellule_runtime::primitives::sql::SqlStatement {
                sql: "INSERT OR IGNORE INTO reminders (schedule, occurrence, payload) VALUES (?1, ?2, ?3)".into(),
                parameters: vec![
                    cellule_runtime::primitives::sql::SqlValue::Blob(input.schedule_id.to_vec()),
                    cellule_runtime::primitives::sql::SqlValue::Integer(
                        i64::try_from(input.occurrence)
                            .map_err(|_| Error::Command("cron occurrence exceeds SQL integer range"))?,
                    ),
                    cellule_runtime::primitives::sql::SqlValue::Blob(input.payload),
                ],
            }],
        })?;
        Ok(CommandResult::Success(()))
    }
}

struct Receiver;

impl SqlModule for Receiver {
    const MODULE: &'static str = Self::NAME;
    const BATCH_COMMAND_ID: u32 = 2;
    const BATCH_QUERY_ID: u32 = 3;
}

impl CellModule for Receiver {
    const NAME: &'static str = "receiver";

    fn descriptor(&self) -> &'static ModuleDescriptor {
        static MIGRATIONS: OnceLock<[MigrationDescriptor; 1]> = OnceLock::new();
        static DESCRIPTOR: OnceLock<ModuleDescriptor> = OnceLock::new();
        DESCRIPTOR.get_or_init(|| ModuleDescriptor {
            name: Self::NAME,
            source_digest: Digest::from_bytes(
                *blake3::hash(include_bytes!("schedules.rs")).as_bytes(),
            ),
            retained_codes: &[],
            schema_min: 1,
            schema_max: 1,
            migrations: MIGRATIONS.get_or_init(|| {
                [MigrationDescriptor {
                    version: 1,
                    sql: RECEIVER_SCHEMA,
                    digest: Digest::from_bytes(
                        *blake3::hash(RECEIVER_SCHEMA.as_bytes()).as_bytes(),
                    ),
                }]
            }),
            commands: &RECEIVER_COMMANDS,
            queries: &RECEIVER_QUERIES,
            workflow_definitions: &[],
            activity_types: &[],
            namespaces: &[NamespaceDescriptor {
                id: RECEIVER,
                name: Self::NAME,
                role: CatalogRole::Sql,
                shards: 1,
                effect_targets: &[],
                dead_letter: None,
            }],
        })
    }

    fn register(self, registry: &mut RegistryBuilder) -> cellule_runtime::Result<()> {
        registry.bind_command::<RecordReminder>()?;
        register_sql::<Self>(registry)
    }
}

struct Schedules;

impl MaintenanceModule for Schedules {
    const MODULE: &'static str = Self::NAME;
    const TICK_COMMAND_ID: u32 = 3;
    const CRON_TARGETS: &'static [CronTarget] = &[CronTarget::new(
        Receiver::NAME,
        RECEIVER,
        RecordReminder::ID,
        1,
        1 << 20,
    )];
}

impl CronModule for Schedules {
    const NAMESPACE: NamespaceId = SCHEDULES;
    const MUTATE_COMMAND_ID: u32 = 1;
    const QUERY_ID: u32 = 2;
}

impl EffectModule for Schedules {
    const MODULE: &'static str = Self::NAME;
    const CLAIM_COMMAND_ID: u32 = 4;
    const LEASE_COMMAND_ID: u32 = 5;
    const VALIDATE_QUERY_ID: u32 = 6;
    const STATUS_QUERY_ID: u32 = 7;
}

impl CellModule for Schedules {
    const NAME: &'static str = "schedules";

    fn descriptor(&self) -> &'static ModuleDescriptor {
        static MIGRATIONS: OnceLock<[MigrationDescriptor; 1]> = OnceLock::new();
        static DESCRIPTOR: OnceLock<ModuleDescriptor> = OnceLock::new();
        DESCRIPTOR.get_or_init(|| ModuleDescriptor {
            name: Self::NAME,
            source_digest: Digest::from_bytes(
                *blake3::hash(include_bytes!("schedules.rs")).as_bytes(),
            ),
            retained_codes: &[],
            schema_min: 1,
            schema_max: 1,
            migrations: MIGRATIONS.get_or_init(|| {
                [MigrationDescriptor {
                    version: 1,
                    sql: SCHEDULE_MIGRATION,
                    digest: Digest::from_bytes(
                        *blake3::hash(SCHEDULE_MIGRATION.as_bytes()).as_bytes(),
                    ),
                }]
            }),
            commands: &SCHEDULE_COMMANDS,
            queries: &SCHEDULE_QUERIES,
            workflow_definitions: &[],
            activity_types: &[],
            namespaces: &[NamespaceDescriptor {
                id: SCHEDULES,
                name: Self::NAME,
                role: CatalogRole::Cron,
                shards: 1,
                effect_targets: &[RECEIVER],
                dead_letter: None,
            }],
        })
    }

    fn register(self, registry: &mut RegistryBuilder) -> cellule_runtime::Result<()> {
        register_cron::<Self>(registry)?;
        register_effect_delivery::<Self>(registry)
    }
}

struct SchedulesApp;

impl CellApplication for SchedulesApp {
    const NAME: &'static str = "schedules-example";

    fn register(builder: &mut cellule_app::ApplicationBuilder) -> cellule_runtime::Result<()> {
        builder.register(Schedules)?;
        builder.register(Receiver)?;
        builder.cell_type(CellType::new(
            Schedules::NAME,
            "schedules",
            SCHEDULES,
            CatalogRole::Cron,
            1,
        )?)?;
        builder.cell_type(CellType::new(
            Receiver::NAME,
            "receiver",
            RECEIVER,
            CatalogRole::Sql,
            1,
        )?)?;
        Ok(())
    }
}

// This signed loopback stands in for the service-owned peer transport. The
// dispatcher still verifies the request before the destination touches SQL.
struct ReceiverResolver {
    target: CellTarget,
    handle: CellHandle,
}

impl PeerCellResolver for ReceiverResolver {
    fn resolve(
        &self,
        target: CellTarget,
    ) -> Pin<Box<dyn Future<Output = cellule_runtime::Result<CellHandle>> + Send + 'static>> {
        let allowed = target == self.target;
        let handle = self.handle.clone();
        Box::pin(async move {
            if allowed {
                Ok(handle)
            } else {
                Err(Error::CellNotActive)
            }
        })
    }
}

struct ReminderAuthorizer;

impl PeerAuthorizer for ReminderAuthorizer {
    fn authorize(&self, request: &VerifiedPeerRequest) -> cellule_runtime::Result<()> {
        if request.permits("example.reminder.deliver") {
            Ok(())
        } else {
            Err(Error::PeerAuthorization(
                "reminder delivery is not permitted",
            ))
        }
    }
}

struct Loopback {
    verifier: Arc<PeerVerifier>,
    dispatcher: Arc<PeerDispatcher>,
}

fn peer_now_ms() -> cellule_runtime::Result<i64> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| Error::Peer("clock is before the Unix epoch"))?;
    i64::try_from(elapsed.as_millis()).map_err(|_| Error::Peer("clock exceeds peer range"))
}

impl PeerRoundTrip for Loopback {
    fn send(
        &self,
        target: CellTarget,
        request: Vec<u8>,
        _remaining_ms: u32,
    ) -> Pin<Box<dyn Future<Output = cellule_runtime::Result<Vec<u8>>> + Send + 'static>> {
        let verifier = Arc::clone(&self.verifier);
        let dispatcher = Arc::clone(&self.dispatcher);
        Box::pin(async move {
            let verified = verifier.verify(&request, peer_now_ms()?)?;
            if verified.target() != &target {
                return Err(Error::Peer("round trip target changed"));
            }
            dispatcher.dispatch_bytes(&verified, peer_now_ms()?).await
        })
    }
}

#[tokio::main]
async fn main() -> ExampleResult<()> {
    let application = Arc::new(SchedulesApp::compile(BuildDescriptor {
        source_revision: "local-schedules-example".into(),
        cargo_lock_digest: Digest::from_bytes(
            *blake3::hash(include_bytes!("../../../Cargo.lock")).as_bytes(),
        ),
    })?);
    let local = LocalCells::new(
        "schedules-example",
        TenantId::from_bytes([83; 16]),
        ApplicationId::from_bytes([84; 16]),
        SessionId::from_bytes([85; 16]),
    )?;
    let result: ExampleResult<()> = async {
        let registry = application.registry();
        let schedule_handle = local
            .bootstrap(
                &registry,
                Schedules::NAME,
                SCHEDULES,
                CatalogRole::Cron,
                86,
                install_cron_schema,
            )
            .await?;
        let receiver_handle = local
            .bootstrap(
                &registry,
                Receiver::NAME,
                RECEIVER,
                CatalogRole::Sql,
                87,
                |transaction| {
                    transaction.execute_batch(RECEIVER_SCHEMA)?;
                    Ok(())
                },
            )
            .await?;
        let client = CellClient::local_many(
            Arc::clone(&registry),
            [schedule_handle, receiver_handle.clone()],
        )?;
        let typed = ApplicationHandle::<SchedulesApp>::new(
            client,
            application,
            local.tenant,
            local.application_id,
        )?;
        let schedule_target = CellTarget::new(
            local.tenant,
            local.application_id,
            SCHEDULES,
            &partition_for_shard(0),
        )?;
        let receiver_target = CellTarget::new(
            local.tenant,
            local.application_id,
            RECEIVER,
            &partition_for_shard(0),
        )?;
        let schedules = typed.cron::<Schedules>()?;
        let now = now_ms()?;
        let schedule_id = [88; 16];
        let committed = schedules
            .mutate(
                identity(89, now),
                CronMutation::Upsert {
                    schedule_id,
                    target_index: 0,
                    target_partition: partition_for_shard(0).to_vec(),
                    payload: b"send-reminder".to_vec(),
                    interval_ms: 60_000,
                    next_due_ms: now,
                },
            )
            .await?;
        let observed = schedules.get(schedule_id, Some(committed.receipt)).await?;
        let CronQueryResult::Get(Some(schedule)) = observed.output else {
            return Err(Error::Control("schedule is missing").into());
        };
        if !schedule.enabled || schedule.payload != b"send-reminder" {
            return Err(Error::Control("schedule differs").into());
        }
        // A host scheduler normally selects the current root and submits this
        // bounded Tick. It emits one durable effect for the due occurrence.
        let tick = typed
            .command::<MaintenanceTickCommand<Schedules>>(
                &schedule_target,
                identity(90, now_ms()?),
                MaintenanceTickRequest {
                    expected_commit_sequence: committed.receipt.commit_sequence,
                },
            )
            .await?;
        if !matches!(tick.output, MaintenanceTickOutcome::Applied { processed } if processed > 0) {
            return Err(Error::Control("due schedule was not advanced").into());
        }
        let after_tick = schedules.get(schedule_id, Some(tick.receipt)).await?;
        let CronQueryResult::Get(Some(after_tick)) = after_tick.output else {
            return Err(Error::Control("fired schedule is missing").into());
        };
        if after_tick.occurrence != 1 {
            return Err(Error::Control("expected one cron occurrence").into());
        }

        // The effect supervisor validates the source lease, sends a signed
        // command to the destination inbox, then acknowledges the source.
        let signer = Arc::new(PeerSigner::new(
            SessionId::from_bytes([85; 16]),
            registry.release_digest(),
            SigningKey::from_bytes(&[91; 32]),
        ));
        let verifier = Arc::new(PeerVerifier::new(
            SessionId::from_bytes([85; 16]),
            registry.release_digest(),
            signer.verifying_key(),
        ));
        let dispatcher = Arc::new(PeerDispatcher::new(
            registry,
            Arc::new(ReceiverResolver {
                target: receiver_target.clone(),
                handle: receiver_handle,
            }),
            Arc::new(ReminderAuthorizer),
        ));
        let peer = EffectPeerClient::new(
            signer,
            PeerPrincipal {
                issuer: "schedules-example".into(),
                subject: "local-runner".into(),
                actions: vec![
                    "cell.read".into(),
                    "cell.write".into(),
                    "example.reminder.deliver".into(),
                ],
            },
            Arc::new(Loopback {
                verifier,
                dispatcher,
            }),
        );
        let source = typed.effects::<Schedules>(schedule_target)?;
        let supervisor = EffectSupervisor::new(source, peer, 5_000)?;
        if !matches!(
            supervisor.run_once().await?,
            EffectRunOutcome::Delivered { .. }
        ) {
            return Err(Error::Control("cron effect was not delivered").into());
        }

        let receiver = typed.sql::<Receiver>(receiver_target)?;
        let counted = receiver
            .query(
                None,
                SqlBatch {
                    statements: vec![SqlStatement {
                        sql: "SELECT COUNT(*) FROM reminders".into(),
                        parameters: Vec::new(),
                    }],
                },
            )
            .await?;
        let [result] = counted.output.as_slice() else {
            return Err(Error::Control("expected one reminder count query").into());
        };
        let [row] = result.rows.as_slice() else {
            return Err(Error::Control("expected one reminder count row").into());
        };
        let [SqlValue::Integer(count)] = row.as_slice() else {
            return Err(Error::Control("expected integer reminder count").into());
        };
        if *count != 1 {
            return Err(Error::Control("expected one delivered reminder").into());
        }
        println!("schedule reminder: one occurrence delivered");
        Ok(())
    }
    .await;
    let shutdown = local.shutdown().await;
    result?;
    shutdown?;
    Ok(())
}
