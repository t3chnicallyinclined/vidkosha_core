# Workstream: Operator Registry Lite

## Ownership & Update Triggers
- **Owner:** Core chain/data maintainer
- **Backups:** Contributor reviewers
- **Update when:** registry fields change, proof format changes, or storage backend changes.

## 1. Summary
Registry for node operators: capture capabilities, jurisdiction, and contact info; persist locally and emit a hashed registry snapshot suitable for CKB anchoring. No payouts or keys included.

## 2. Goals & Non-Goals
**Goals**
- Define a minimal operator record (id, capabilities, domains, jurisdiction, contact URI placeholder).
- Provide a local registry store (JSON/SQLite) with CRUD CLI.
- Export a deterministic hash/snapshot for anchoring (pairs with `ckb_anchor_lite`).
- Add basic validation and a health/status command.

**Non-Goals (this phase)**
- Payout math, NCC staking logic, or proprietary scoring.
- Storage of secrets or production identities.

## 3. Target Design
- Schema aligns with fields from `contexts/helix_schema.md` (`operator_node`, `usage_event`, `payout_event` minimal fields).
- CLI commands: `op-reg add|list|show|export|hash|health`.
- Storage: local JSON or SQLite; deterministic export for anchoring.

## 4. Suggested Implementation Steps
1. Define `OperatorRecord` schema and validation rules (documented fields only).
2. Implement CRUD CLI over JSON/SQLite.
3. Implement `export` + `hash` to produce a snapshot for anchoring.
4. Add `health` command to check last heartbeat/metadata completeness.
5. Add tests for validation, deterministic hashing, and CRUD round-trip.

## 5. Acceptance Criteria
- Operators can be added/listed/validated locally.
- Snapshot and hash are deterministic and ready for `ckb_anchor_lite`.
- Tests cover schema validation and hashing.

## Notes
- Keep records lean; do not store secrets or contract terms.
- Teams can extend with staking, payout weights, and KYC in their own pack.
