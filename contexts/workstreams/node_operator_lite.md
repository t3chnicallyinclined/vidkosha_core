# Workstream: CKB Node Operator Lite

## Ownership & Update Triggers
- **Owner:** Core CKB node-ops maintainer
- **Backups:** Contributor reviewers
- **Update when:** supported chains/clients change, health checks change, or packaging changes

## 1. Summary
Baseline for running and monitoring a CKB full node (`ckb` binary): health checks, restart hooks, metrics, and basic automation. No sensitive infra details included.

## 2. Goals & Non-Goals
**Goals**
- Provide a minimal ops script/CLI to start/stop a node with sane defaults.
- Health checks: sync status, peer count, latest block age, disk usage.
- Optional alert hooks (webhook placeholder) and structured logging.
- Basic exporter/metrics endpoints.

**Non-Goals (this phase)**
- Proprietary deployment topologies, secret keys/wallet ops, or cloud-specific runbooks.
- Advanced auto-scaling or multi-region orchestration (deferred).

## 3. Current State
- No node tooling assumed; contributors should target the CKB client with shared configs.

## 4. Target Design
- Config file for node binary path (`ckb`), data dir, RPC port (default 8114), P2P port (default 8115), and alert webhook URL placeholder.
- Commands: `node-ops start|stop|status|health`.
- Health command checks: tip height vs reference explorer/RPC (Pudge or Mirana), peer count, latest block age, disk usage threshold, optional DAO deposit presence.
- Metrics: expose a small `/metrics` or log in JSON.

## 5. Suggested Implementation Steps
1. Add config struct and example config file.
2. Implement start/stop wrappers for the chosen client binary.
3. Implement health checks hitting RPC endpoints and local system stats.
4. Add optional webhook notifier for failing health.
5. Add tests: config parsing, health check against mocked RPC, status command.

## 6. Acceptance Criteria
- `node-ops status` reports sync/health; `start/stop` wrap the node binary.
- Health check detects lagging node and reports via log/webhook placeholder.
- Metrics/logging present; no secrets required to run.

## Notes
- Do not hardcode secrets, RPCs, keys, or cloud details.
- Teams can extend with deploy/runbooks and secret management in their own pack.
