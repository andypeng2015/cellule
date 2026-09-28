# Releasing Cellule

The seven workspace crates publish to crates.io as `cellule-types`,
`cellule-store`, `cellule-ltx`, `cellule-runtime`, `cellule-app`, and
`cellule-host`, and the optional `cellule-peer-http`. They share one version: an embedding service depends on the
set, and the intra-workspace requirements pin it exactly (`=0.1.0`).

## Before the first release

1. Check the registry for existing names and versions:

   ```sh
   for crate in cellule-types cellule-store cellule-ltx cellule-runtime cellule-app cellule-host cellule-peer-http; do
     curl -s -A 'cellule-release-check' "https://crates.io/api/v1/crates/$crate" \
       | grep -q 'does not exist' && echo "$crate: free" || echo "$crate: TAKEN"
   done
   ```

2. Run the full gate from a clean checkout:

   ```sh
   cargo fmt --all --check
   cargo check --workspace --all-targets --locked
   cargo test --workspace --all-features --locked
   cargo test -p cellule-ltx --features replica --locked
   cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
   RUSTDOCFLAGS='-D warnings' cargo doc --workspace --all-features --no-deps --locked
   python3 scripts/check-boundaries.py
   python3 scripts/check-module-layout.py
   python3 scripts/check-doc-rust-fences.py
   python3 scripts/check-doc-links.py
   node crates/cellule-runtime/docs/validate.mjs
   ```

3. Package every crate. `cargo package` for a single crate cannot resolve the
   unpublished workspace dependencies, so package the workspace as a set:

   ```sh
   cargo package --workspace --locked
   ```

## Publishing order

Publish in dependency order so each crate can resolve the ones below it:

```sh
cargo publish -p cellule-types
cargo publish -p cellule-store
cargo publish -p cellule-ltx
cargo publish -p cellule-runtime
cargo publish -p cellule-app
cargo publish -p cellule-host
cargo publish -p cellule-peer-http
```

Wait for each crate to appear in the registry index before publishing the next
one. `cargo publish --dry-run` verifies a crate that has no unpublished
dependencies, which is why the order matters. Publishing requires a crates.io
token; nothing in this repository uploads on its own.

## After publishing

- Tag the release and record the exact revisions in the release notes.
- Embedding services can adopt the published `cellule-*` dependencies. Cellule
  remains the source of truth for their contracts; `docs/synthesis.md` is
  historical.
- Keep the bundled attributions with the published crates: `cellule-ltx`
  ships `LICENSE` and `LICENSE.pierrec-lz4`, and its `UPSTREAM.md` must keep
  naming the Celld, rustyriver, Litestream, and LTX sources.
