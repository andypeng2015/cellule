//! Local fixtures shared by the advanced runnable examples.
//!
//! An application still has to supply a provider, catalog, authority, and
//! owner before it can call a Cell. This helper keeps that repeated local-only
//! setup out of the Cron, Workflow, and Effects lessons. It does not model a
//! serving node, HTTP ingress, credentials, or fleet admission.

use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use cellule_ltx::{CellReplica, DiskBudget, Host, Limits, rusqlite::Transaction};
use cellule_runtime::cell::{
    actor::CellHandle,
    catalog::{CatalogEntry, CellCatalog},
};
use cellule_runtime::control::{Owner, authority::CellAuthority};
use cellule_runtime::identity::{IncarnationId, RequestId};
use cellule_runtime::ltx::CellStorageLayout;
use cellule_runtime::registry::Registry;
use cellule_runtime::{
    ApplicationId, CatalogRole, CellRuntime, CellTarget, Error, MutationIdentity, NamespaceId,
    SessionId, SqlWorkerPool, TenantId, partition_for_shard,
};
use cellule_store::Store;
use object_store::{memory::InMemory, path::Path};

pub type ExampleResult<T> = Result<T, Box<dyn std::error::Error>>;

pub fn now_ms() -> ExampleResult<i64> {
    Ok(i64::try_from(
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis(),
    )?)
}

pub fn identity(request_id: u8, now_ms: i64) -> MutationIdentity {
    MutationIdentity {
        request_id: RequestId::from_bytes([request_id; 16]),
        issued_at_ms: now_ms,
        expires_at_ms: now_ms + 60_000,
    }
}

pub struct LocalCells {
    pub runtime: CellRuntime,
    pub tenant: TenantId,
    pub application_id: ApplicationId,
    session: SessionId,
    layout: CellStorageLayout,
    files: tempfile::TempDir,
}

impl LocalCells {
    pub fn new(
        prefix: &str,
        tenant: TenantId,
        application_id: ApplicationId,
        session: SessionId,
    ) -> ExampleResult<Self> {
        let layout = CellStorageLayout::new(
            Store::new(Arc::new(InMemory::new())),
            Path::from(prefix),
            *application_id.as_bytes(),
        );
        Ok(Self {
            runtime: CellRuntime::new_with_replica_host(
                SqlWorkerPool::new(4, 16)?,
                64 * 1024 * 1024,
                session,
                Host::default().with_local_disk_budget(DiskBudget::new(1 << 30)),
            )?,
            tenant,
            application_id,
            session,
            layout,
            files: tempfile::TempDir::new()?,
        })
    }

    pub async fn bootstrap<F>(
        &self,
        registry: &Registry,
        module: &'static str,
        namespace: NamespaceId,
        role: CatalogRole,
        owner_id: u8,
        initialize: F,
    ) -> ExampleResult<CellHandle>
    where
        F: for<'connection> FnOnce(&Transaction<'connection>) -> cellule_runtime::Result<()>
            + Send
            + 'static,
    {
        let target = CellTarget::new(
            self.tenant,
            self.application_id,
            namespace,
            &partition_for_shard(0),
        )?;
        let code = registry
            .module_code(module)
            .ok_or(Error::Registry("example module is missing"))?;
        let proof = CellCatalog::new(self.layout.clone(), self.tenant)
            .provision(CatalogEntry::new(&target, role, code, 1)?)
            .await?;
        let authority = CellAuthority::new(self.layout.clone());
        let incarnation = IncarnationId::from_bytes([owner_id; 16]);
        let observed = authority
            .create_initial(
                &proof,
                incarnation,
                Owner {
                    session: self.session,
                    endpoint: format!("https://{module}.local"),
                },
            )
            .await?;
        Ok(self
            .runtime
            .bootstrap(
                proof,
                CellReplica::new(
                    self.layout.clone(),
                    *target.cell_id().as_bytes(),
                    *incarnation.as_bytes(),
                    Limits::default(),
                )?,
                authority,
                observed,
                self.files.path().join(format!("{module}.sqlite")),
                initialize,
            )
            .await?)
    }

    pub async fn shutdown(&self) -> ExampleResult<()> {
        self.runtime.shutdown().await?;
        Ok(())
    }
}
