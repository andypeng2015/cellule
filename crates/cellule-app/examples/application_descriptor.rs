//! Declare a module and Cell type, then compile an application descriptor.
//!
//! Run with `cargo run -p cellule-app --example application_descriptor`.
//! This example only describes the application; it does not start a Cell.

use std::sync::OnceLock;

use cellule_app::{ApplicationBuilder, CellType};
use cellule_runtime::cell::catalog::CatalogRole;
use cellule_runtime::identity::{Digest, NamespaceId};
use cellule_runtime::registry::{
    BuildDescriptor, CellModule, MigrationDescriptor, ModuleDescriptor, NamespaceDescriptor,
    RegistryBuilder,
};

/// A module that owns the repository namespace.
struct Repository;

const REPOSITORY_NAMESPACE: NamespaceId = NamespaceId::from_bytes([2; 16]);
const SCHEMA: &str = "CREATE TABLE repositories (id INTEGER PRIMARY KEY, name TEXT NOT NULL)";

// Module descriptors borrow their lists for the life of the process.
static NAMESPACES: [NamespaceDescriptor; 1] = [NamespaceDescriptor {
    id: REPOSITORY_NAMESPACE,
    name: "repository",
    role: CatalogRole::Sql,
    shards: 1,
    effect_targets: &[],
    dead_letter: None,
}];

impl CellModule for Repository {
    const NAME: &'static str = "repository";

    fn descriptor(&self) -> &'static ModuleDescriptor {
        static MIGRATIONS: OnceLock<[MigrationDescriptor; 1]> = OnceLock::new();
        static DESCRIPTOR: OnceLock<ModuleDescriptor> = OnceLock::new();
        DESCRIPTOR.get_or_init(|| ModuleDescriptor {
            name: Self::NAME,
            source_digest: Digest::from_bytes(
                *blake3::hash(include_bytes!("application_descriptor.rs")).as_bytes(),
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
            commands: &[],
            queries: &[],
            workflow_definitions: &[],
            activity_types: &[],
            namespaces: &NAMESPACES,
        })
    }

    /// This descriptor has no commands or queries to bind.
    fn register(self, _registry: &mut RegistryBuilder) -> cellule_runtime::Result<()> {
        Ok(())
    }
}

fn main() -> cellule_runtime::Result<()> {
    let mut builder = ApplicationBuilder::new(
        "repository",
        BuildDescriptor {
            source_revision: "application-descriptor-example".into(),
            cargo_lock_digest: Digest::from_bytes(
                *blake3::hash(include_bytes!("../../../Cargo.lock")).as_bytes(),
            ),
        },
    )?;
    builder.register(Repository)?;
    builder.cell_type(CellType::new(
        "repository",
        "repository",
        REPOSITORY_NAMESPACE,
        CatalogRole::Sql,
        1,
    )?)?;

    let application = builder.finish()?;
    println!(
        "compiled {} with {} cell type(s), digest {:?}",
        application.name(),
        application.cell_types().len(),
        application.descriptor_digest()
    );
    Ok(())
}
