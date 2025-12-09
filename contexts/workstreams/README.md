# Workstreams (CKB-Focused)

This branch ships only CKB-focused workstreams. AI/Helix/Fabric plans are kept minimal here. To add a new workstream:

1) Copy `workstream_template.md` to `contexts/workstreams/<name>.md`.
2) Keep content high-level and non-sensitive: no secrets, keys, vendor IDs, internal pricing, or roadmap specifics.
3) Make ownership, goals/non-goals, and acceptance criteria clear so contributors can engage.
4) If deeper implementation notes exist, reference an external playbook instead of copying it here.

Teams can replace these files with their own pack at deploy time. Current workstreams:

- `ckb_indexing_lite.md` (CKB indexer)
- `node_operator_lite.md`
- `operator_registry_lite.md`
- `ckb_anchor_lite.md`
- `treasury_policy_lite.md`
