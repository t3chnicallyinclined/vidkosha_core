# Workstream: CKB Anchoring Lite

## Ownership & Update Triggers
- **Owner:** Core chain integrator
- **Backups:** Contributor reviewers
- **Update when:** event schemas change, anchor cadence changes, or hash payload shape changes.

## 1. Summary
Plan to anchor Vidkosha Cortex runtime events (usage/payout receipts) to CKB with minimal scope: hash payloads, write to chain, and confirm inclusion. Also covers anchoring Axon checkpoints or Fiber channel receipts as optional payloads. No tokenomics or proprietary payouts in this file.

## 2. Goals & Non-Goals
**Goals**
- Define a minimal anchor payload (hash of usage/payout batch + metadata).
- Provide a CLI to build and submit anchor txs to a CKB testnet RPC.
- Verify inclusion and write proof back to local storage/logs.
- Keep chain integration behind a feature flag so off-chain mode remains default.

**Non-Goals (this phase)**
- Mainnet economics, NCC issuance, or proprietary payout math.
- Secret keys or production operational runbooks.

## 3. Target Design
- Inputs: NDJSON/JSON batch of `usage_event`/`payout_event` records (sample fixtures only).
- Hashing: deterministic hash over sorted fields; store hash + batch_id + timestamp.
- Submission: simple CKB transaction via CLI using a test key (local devnet or shared testnet).
- Confirmation: poll for inclusion; emit a local proof file with tx hash and block number.

## 4. Suggested Implementation Steps
1. Define a minimal anchor payload struct and hash function (payout batch hash; optional Axon checkpoint hash; optional Fiber channel receipt hash).
2. Add sample fixtures (sanitized) under `fixtures/chain/` covering payouts plus one Axon checkpoint example and one Fiber receipt example.
3. Implement `cargo run -- anchor --batch fixtures/chain/sample_payout_batch.json` to build and submit a tx (mock or testnet).
4. Add a verification command to confirm inclusion and write a proof file.
5. Wire a feature flag to keep chain code optional in builds/tests.

## 5. Acceptance Criteria
- CLI can hash a sample batch, submit to testnet/devnet, and confirm inclusion.
- Proof file is written with tx hash, block number, and payload hash.
- Off-chain builds remain default; chain code is gated by a flag.

## Notes
- Use test keys/endpoints only; never include secret keys or mainnet RPCs.
- Teams can swap configs/keys and richer payloads in their own pack.
