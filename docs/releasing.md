# Releasing Cellule

Cellule publishes seven crates as one matched version. The workspace currently
pins every internal dependency to `=0.1.0`; change all crate versions and pins
together for a release. Publish in dependency order, then verify the released
set from a clean consumer. This guide is a release checklist, not evidence that
a release or provider qualification has happened.

## 1. Freeze the contract

Review producers and consumers before changing a persisted or wire format:

| Contract | Review before release |
| --- | --- |
| Cell and module identity | Namespace, partition rule, role, schema version, operation IDs, and descriptor digest. |
| Durable state | SQLite migration path, LTX encoding, root and object paths, WAL boundary, and recovery verification. |
| Coordination | Owner fence, authority compare-and-swap, request outcome, receipt, and follower proof. |
| Peer protocol | Signed messages, replay windows, and compatible rollout behavior. |
| Blob lifecycle | Cross-Cell references, quiescence, and deletion grace boundary. |

The [API guide](api.md) describes author-visible contracts. The
[architecture guide](architecture.md) explains why publication and recovery
must agree on one exact root. Record the source revision, target version,
compatibility decision, and migration evidence in release notes.

## 2. Verify the candidate

Run broad suites in CI or an isolated verification snapshot. On workstations
with the mounted Workspace volume, give this checkout its own
`CARGO_TARGET_DIR` beneath `$HOME/Workspace/crabbuild-target`.

```sh
cargo fmt --all --check
cargo check --workspace --all-targets --all-features --locked
cargo test --workspace --all-features --locked
cargo test -p cellule-ltx --no-default-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --all-features --no-deps --locked
python3 scripts/check-boundaries.py
python3 scripts/check-module-layout.py
python3 scripts/check-doc-rust-fences.py
python3 scripts/check-doc-links.py
node crates/cellule-runtime/docs/validate.mjs
```

These checks establish local code and documentation behavior. Provider,
scale, compatibility, and fault claims need their own measured artifacts and
signed receipts. Use the [qualification profiles](../crates/cellule-runtime/qualification/README.md)
and [delivery evidence guide](../crates/cellule-runtime/docs/delivery.md);
do not substitute a synthetic workload for protected evidence.

## 3. Package and inspect

From a clean candidate revision, package the complete workspace so Cargo can
resolve its unpublished path dependencies together:

```sh
cargo package --workspace --locked
```

Inspect packaged manifests, included files, license and attribution files,
and the generated archive contents. In particular, `cellule-ltx` carries its
[upstream provenance](../crates/cellule-ltx/UPSTREAM.md) and bundled license
files. Run the package check again after any manifest or include-list change.

## 4. Publish in dependency order

Wait until each version is visible in the registry index before publishing the
next crate. Use the same version for all seven crates.

```sh
cargo publish -p cellule-types --locked
cargo publish -p cellule-store --locked
cargo publish -p cellule-ltx --locked
cargo publish -p cellule-runtime --locked
cargo publish -p cellule-app --locked
cargo publish -p cellule-host --locked
cargo publish -p cellule-peer-http --locked
```

`cellule-peer-http` is optional for applications, but is part of the matched
workspace set. Publishing requires registry credentials and is an explicit
release action; none of the verification commands upload a crate.

## 5. Verify the published set

Create a clean consumer using the published exact versions, compile a typed
application, and run its local write/read path. Confirm the registry versions,
tag the exact source revision, and link the qualification artifacts in release
notes. If publication stops partway through the set, record which versions are
visible and finish or supersede that set deliberately; crates already uploaded
cannot be silently replaced.
