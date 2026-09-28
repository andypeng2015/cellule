# AGENTS.md

Scoped rules for `crates/cellule-app/`. Root guidance applies.

- Keep this crate dependency-light: runtime contracts only; no HTTP, provider,
  storage construction, credentials, or node lifecycle policy.
- Stable IDs, role, shard count, and descriptor bytes are application contracts.
- Do not expose raw SQLite, authority, replica, local paths, or arbitrary
  operation IDs through the author handle.
- Unit tests stay beside private implementation; public scenarios use
  `tests/integration.rs` and `tests/contracts.rs`. The integration suite's
  modules live directly in `tests/`; explicit Cargo targets keep those modules
  from becoming separate test binaries. The module-layout check rejects
  uncompiled test files.
- Run `python3 scripts/check-module-layout.py` after layout changes.
- Run the crate tests, format, Clippy, and dependency-tree checks after API
  changes.
