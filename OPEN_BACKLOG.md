# Open Backlog

Use these as community-friendly starting points. Link PRs to the relevant workstream file in `contexts/workstreams/`.

## Ready for Contribution
- Blockchain Indexing Lite
  - Tasks: add example config; implement block streaming (poll/WS) with retry; decode sample ABI logs; define minimal DB schema; add read API and metrics; tests with fixtures/devnet.
- Node Operator Lite
  - Tasks: add example config; implement start/stop wrappers; health checks (sync, peers, disk); optional webhook notifier; metrics/logging; tests with mocked RPC.
- Operator Registry Lite
  - Tasks: define `OperatorRecord`; CRUD CLI (JSON/SQLite); deterministic export + hash; validation + tests.
- CKB Anchoring Lite
  - Tasks: define anchor payload; sample fixtures; CLI to hash/submit/verify on testnet or mock; proof file output; feature flag gating.
- Treasury Policy Lite
  - Tasks: define policy schema; CLI set/get/list/export/hash; validation (percents sum to 100, epoch ordering); fixtures; tests.

## Good First Issues
- Add a neutral sample ABI and block/tx/event fixtures for the indexer tests.
- Add a mocked RPC health-check test harness for node-ops.
- Add sample operator records and a deterministic hash test for the registry CLI.
- Add sample treasury policy fixture and validation tests.

## How to Propose New Workstreams
- Copy `contexts/workstreams/workstream_template.md`.
- Keep content safe (no secrets, RPC keys, vendor credentials, or internal thresholds).
- Add clear goals, non-goals, and acceptance criteria.
