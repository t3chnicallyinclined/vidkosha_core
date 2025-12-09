# Workstream: CKB Indexing Lite

## Ownership & Update Triggers
- **Owner:** Core CKB maintainer
- **Backups:** Contributor reviewers
- **Update when:** node endpoints/ABIs change, indexing schema evolves, or API shape changes

## 1. Summary
Baseline for a lightweight CKB indexer: ingest CKB blocks, extract transactions/cells of interest, persist to a simple store, and expose a read API. No vendor secrets or sensitive datasets included.

## 2. Goals & Non-Goals
**Goals**
- Connect to a testnet or mainnet CKB RPC/WebSocket endpoint (configurable) and stream blocks.
- Parse and filter transactions/cells based on a small sample contract/lock example.
- Store indexed data in a simple DB (SQLite/Postgres selectable via config).
- Provide a minimal REST/GraphQL read API for queries.
- Include basic metrics/logging hooks.

**Non-Goals (this phase)**
- Proprietary analytics, MEV logic, or proprietary contracts.
- Production-grade HA/monitoring runbooks (deferred).

## 3. Current State
- Infra not assumed; contributors should rely on CKB RPCs or local devnets.
- No schema fixed yet; contributors may propose a minimal schema.

## 4. Target Design
- Config: CKB RPC URL (default `http://localhost:8114` for devnet or Pudge testnet), start block, polling/WS mode, DB URL, tables for blocks/tx/cells.
- Modules: `ingest` (pull/subscribe), `decode` (contract/lock-based), `store` (DB writes), `api` (read endpoints), `metrics` (basic counters).
- Dev UX: `cargo run -- indexer --config config/example_ckb_indexer.toml`.
- Sample focus: secp256k1 lock script filter (key hash) for funding/settlement; extendable to xUDT locks if needed.

## 5. Suggested Implementation Steps
1. Add config struct and example config file with a CKB RPC placeholder (Pudge/Mirana friendly) and default ports (RPC 8114, P2P 8115).
2. Implement block streaming (poll or WS) with retry/backoff.
3. Implement transaction/cell decoding for a sample secp256k1 lock (include a sample script and lock args) and optionally a xUDT example.
4. Define minimal DB schema (blocks, tx, cells) and migrations.
5. Add read API endpoints and basic metrics/logging.
6. Add tests: decode sample blocks/transactions; integration smoke against a local devnet or canned fixtures.
7. Stretch: note Axon sidechain checkpoints or Fiber funding/settlement txs can be indexed by adding filters for their lock scripts.

## 6. Acceptance Criteria
- Runs against a testnet (Pudge) or mainnet CKB RPC, or a local devnet with sample contract/lock, and writes to DB.
- Read API returns indexed cells/tx filtered by the sample lock.
- Tests/fixtures cover decode and ingest of sample blocks.
- No proprietary endpoints, contracts, or datasets included here.

## Notes
- Keep ABIs/examples generic. Do not include proprietary contracts or keys.
- Teams can swap configs/ABIs via their own pack at deploy time.
