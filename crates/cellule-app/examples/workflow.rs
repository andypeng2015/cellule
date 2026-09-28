//! Start a durable Workflow and run its external Activity once.
//!
//! Run: `cargo run -p cellule-app --example workflow --locked`
//!
//!   Application -> Workflow Cell: start("welcome")
//!   Workflow Cell -> durable history: Running + Activity intent
//!   ActivitySupervisor -> ActivityHandler: claim, validate, execute
//!   ActivitySupervisor -> Workflow Cell: record completion
//!   Application -> Workflow Cell: read Completed state at receipt
//!
//! The activity here only echoes local bytes. A real activity uses its stable
//! `ActivityContext::idempotency_key` when calling an external service.

mod support;

use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, OnceLock},
};

use cellule_app::{ApplicationHandle, CellApplication, CellType};
use cellule_runtime::primitives::maintenance::{MaintenanceModule, register_maintenance};
use cellule_runtime::primitives::workflow::{
    ActivityContext, ActivityExecution, ActivityHandler, ActivityRunOutcome, ActivitySupervisor,
    WorkflowAction, WorkflowActivityModule, WorkflowContext, WorkflowDecision, WorkflowDefinition,
    WorkflowModule, WorkflowStatus, install_workflow_schema, register_activity, register_workflow,
    register_workflow_activities,
};
use cellule_runtime::registry::OperationDescriptor;
use cellule_runtime::{
    ApplicationId, BuildDescriptor, CatalogRole, CellClient, CellModule, Digest, Error,
    MigrationDescriptor, ModuleDescriptor, NamespaceDescriptor, NamespaceId, RegistryBuilder,
    SessionId, TenantId,
};

use support::{ExampleResult, LocalCells, identity, now_ms};

const FLOWS: NamespaceId = NamespaceId::from_bytes([71; 16]);
const DEFINITION: Digest = Digest::from_bytes([72; 32]);
const MIGRATION: &str = "-- workflow example schema v1";
const ACTIVITY_TYPES: &[&str] = &["echo"];
const COMMANDS: [OperationDescriptor; 8] = [
    operation(1),
    operation(2),
    operation(3),
    operation(4),
    operation(6),
    operation(7),
    operation(8),
    operation(9),
];
const QUERIES: [OperationDescriptor; 2] = [operation(5), operation(10)];

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

struct Welcome;

impl WorkflowDefinition for Welcome {
    fn digest(&self) -> Digest {
        DEFINITION
    }

    fn transition(
        &self,
        state: &[u8],
        event: &[u8],
        context: WorkflowContext,
    ) -> cellule_runtime::Result<WorkflowDecision> {
        if event == b"begin" {
            return Ok(WorkflowDecision {
                status: WorkflowStatus::Running,
                state: b"waiting-for-activity".to_vec(),
                result: None,
                actions: vec![WorkflowAction::Activity {
                    activity_type: "echo".into(),
                    input: b"welcome sent".to_vec(),
                    due_at_ms: context.now_ms(),
                    expires_at_ms: context.now_ms() + 60_000,
                }],
            });
        }
        if event.starts_with(b"activity\0") {
            return Ok(WorkflowDecision {
                status: WorkflowStatus::Completed,
                state: b"done".to_vec(),
                result: Some(event.to_vec()),
                actions: Vec::new(),
            });
        }
        Err(Error::Command(if state.is_empty() {
            "unexpected workflow start event"
        } else {
            "unexpected workflow completion event"
        }))
    }
}

static WELCOME: Welcome = Welcome;
static DEFINITIONS: &[&dyn WorkflowDefinition] = &[&WELCOME];

struct Echo;

impl ActivityHandler for Echo {
    const TYPE: &'static str = "echo";

    fn execute(
        _context: ActivityContext,
        input: Vec<u8>,
    ) -> Pin<Box<dyn Future<Output = ActivityExecution> + Send + 'static>> {
        Box::pin(async move { ActivityExecution::Completed(input) })
    }
}

struct Flows;

impl MaintenanceModule for Flows {
    const MODULE: &'static str = Self::NAME;
    const TICK_COMMAND_ID: u32 = 9;
    const WORKFLOW_DEFINITIONS: &'static [&'static dyn WorkflowDefinition] = DEFINITIONS;
}

impl WorkflowModule for Flows {
    const MODULE: &'static str = Self::NAME;
    const NAMESPACE: NamespaceId = FLOWS;
    const CURRENT_DEFINITION: &'static dyn WorkflowDefinition = &WELCOME;
    const DEFINITIONS: &'static [&'static dyn WorkflowDefinition] = DEFINITIONS;
    const START_COMMAND_ID: u32 = 1;
    const SIGNAL_COMMAND_ID: u32 = 2;
    const CANCEL_COMMAND_ID: u32 = 3;
    const CONTROL_COMMAND_ID: u32 = 4;
    const GET_QUERY_ID: u32 = 5;
}

impl WorkflowActivityModule for Flows {
    const ACTIVITY_TYPES: &'static [&'static str] = ACTIVITY_TYPES;
    const ACTIVITY_CLAIM_COMMAND_ID: u32 = 6;
    const ACTIVITY_COMPLETE_COMMAND_ID: u32 = 7;
    const ACTIVITY_EXTEND_COMMAND_ID: u32 = 8;
    const ACTIVITY_VALIDATE_QUERY_ID: u32 = 10;
}

impl CellModule for Flows {
    const NAME: &'static str = "flows";

    fn descriptor(&self) -> &'static ModuleDescriptor {
        static MIGRATIONS: OnceLock<[MigrationDescriptor; 1]> = OnceLock::new();
        static DESCRIPTOR: OnceLock<ModuleDescriptor> = OnceLock::new();
        DESCRIPTOR.get_or_init(|| ModuleDescriptor {
            name: Self::NAME,
            source_digest: Digest::from_bytes(
                *blake3::hash(include_bytes!("workflow.rs")).as_bytes(),
            ),
            retained_codes: &[],
            schema_min: 1,
            schema_max: 1,
            migrations: MIGRATIONS.get_or_init(|| {
                [MigrationDescriptor {
                    version: 1,
                    sql: MIGRATION,
                    digest: Digest::from_bytes(*blake3::hash(MIGRATION.as_bytes()).as_bytes()),
                }]
            }),
            commands: &COMMANDS,
            queries: &QUERIES,
            workflow_definitions: &[DEFINITION],
            activity_types: ACTIVITY_TYPES,
            namespaces: &[NamespaceDescriptor {
                id: FLOWS,
                name: Self::NAME,
                role: CatalogRole::Workflow,
                shards: 1,
                effect_targets: &[],
                dead_letter: None,
            }],
        })
    }

    fn register(self, registry: &mut RegistryBuilder) -> cellule_runtime::Result<()> {
        register_workflow::<Self>(registry)?;
        register_workflow_activities::<Self>(registry)?;
        register_activity::<Self, Echo>(registry)?;
        register_maintenance::<Self>(registry)
    }
}

struct WorkflowApp;

impl CellApplication for WorkflowApp {
    const NAME: &'static str = "workflow-example";

    fn register(builder: &mut cellule_app::ApplicationBuilder) -> cellule_runtime::Result<()> {
        builder.register(Flows)?;
        builder.cell_type(CellType::new(
            Flows::NAME,
            "flows",
            FLOWS,
            CatalogRole::Workflow,
            1,
        )?)?;
        Ok(())
    }
}

#[tokio::main]
async fn main() -> ExampleResult<()> {
    let application = Arc::new(WorkflowApp::compile(BuildDescriptor {
        source_revision: "local-workflow-example".into(),
        cargo_lock_digest: Digest::from_bytes(
            *blake3::hash(include_bytes!("../../../Cargo.lock")).as_bytes(),
        ),
    })?);
    let local = LocalCells::new(
        "workflow-example",
        TenantId::from_bytes([73; 16]),
        ApplicationId::from_bytes([74; 16]),
        SessionId::from_bytes([75; 16]),
    )?;
    let result: ExampleResult<()> = async {
        let registry = application.registry();
        let handle = local
            .bootstrap(
                &registry,
                Flows::NAME,
                FLOWS,
                CatalogRole::Workflow,
                76,
                install_workflow_schema,
            )
            .await?;
        let typed = ApplicationHandle::<WorkflowApp>::new(
            CellClient::local(registry, handle),
            application,
            local.tenant,
            local.application_id,
        )?;
        let workflow = typed.workflow::<Flows>()?;
        let workflow_id = b"welcome/42".to_vec();
        let started = workflow
            .start(
                identity(77, now_ms()?),
                workflow_id.clone(),
                b"begin".to_vec(),
            )
            .await?;
        if !matches!(
            started.output,
            cellule_runtime::primitives::workflow::WorkflowOutcome::Applied {
                status: WorkflowStatus::Running,
                ..
            }
        ) {
            return Err(Error::Control("workflow did not start").into());
        }
        // The supervisor owns the lease check and records the handler's result.
        let supervisor = ActivitySupervisor::new(typed.activities::<Flows>()?, 5_000)?;
        let receipt = match supervisor.run_once(0, None).await? {
            ActivityRunOutcome::Completed { receipt, .. } => receipt,
            _ => return Err(Error::Control("activity did not complete").into()),
        };
        let observed = workflow.state(workflow_id, Some(receipt)).await?;
        let state = observed
            .output
            .ok_or(Error::Control("workflow is missing"))?;
        if state.status != WorkflowStatus::Completed {
            return Err(Error::Control("workflow did not complete").into());
        }
        println!("workflow welcome/42: completed");
        Ok(())
    }
    .await;
    let shutdown = local.shutdown().await;
    result?;
    shutdown?;
    Ok(())
}
