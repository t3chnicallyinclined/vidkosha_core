# Workstream: Treasury Policy Lite

## Ownership & Update Triggers
- **Owner:** Core governance maintainer
- **Backups:** Contributor reviewers
- **Update when:** policy fields change, serialization changes, or storage location changes.

## 1. Summary
Scaffold to manage treasury split parameters off-chain, export them for audit, and prepare them for future on-chain governance. Keeps sensitive financial assumptions out-of-repo.

## 2. Goals & Non-Goals
**Goals**
- Define a minimal `treasury_policy` schema (ops, reserve, liquidity, dev fund percents, effective epoch).
- Provide CLI to create/update/list/export policies and serialize to JSON/TOML.
- Produce a deterministic hash/snapshot for anchoring.
- Keep policy storage off-chain by default; chain emission is optional.

**Non-Goals (this phase)**
- Real financial modeling, token issuance, or revenue numbers.
- Legal wrappers, contracts, or tax guidance.

## 3. Target Design
- Schema mirrors the fields in `contexts/helix_schema.md` for `treasury_policy`.
- CLI commands: `treasury set|get|list|export|hash`.
- Storage: local JSON/TOML with deterministic ordering.
- Optional integration: hand off the hash to `ckb_anchor_lite`.

## 4. Suggested Implementation Steps
1. Define the schema and validation (percents sum to 100, epochs monotonically increase).
2. Implement CRUD/export/hash CLI.
3. Add sample fixtures under `fixtures/treasury/`.
4. Add tests for validation and deterministic hashing.

## 5. Acceptance Criteria
- Policies can be created/listed/exported with validation enforced.
- Hash export is deterministic and usable by anchoring.
- No real financial data is present in the repo.

## Notes
- Do not include actual treasury numbers or contracts.
- Teams can plug in real data and governance bindings in their own pack.
