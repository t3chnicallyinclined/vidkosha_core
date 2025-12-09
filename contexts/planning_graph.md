# Vidkosha Cortex Planning Graph – Helix-Backed Vision Board

Design spec for storing the Vision Board, workstreams, and context docs inside HelixDB so agents and contributors can resume work from the graph, even when the repo is closed.

---

## Ownership & Update Triggers

- **Owner:** `CTOAgent` maintains this spec.
- **Backups:** `AgentCreator` (planning graph integration) and `HelixFabricAgent` (schema alignment).
- **Update when:** planning node/edge schemas change, syncing/mirroring processes change, or Vision Board/workstream linkage changes.
- **Mirror:** Keep `contexts/vision_board.md`, `contexts/context.md`, and workstream files aligned when updating.

## 1. Purpose

- Mirror `contexts/vision_board.md` and workstream context files into Helix as first-class graph objects.
- Let agents list workstreams, fetch their latest context, and see linked files/issues without parsing markdown every time.
- Maintain Git as the human-editable source of truth while Helix acts as the live, queryable planning index.

---

## 2. Helix Schema (Planning Types)

### 2.1 Node Types

| Type | Purpose | Required Fields | Optional Fields |
| --- | --- | --- | --- |
| `vision_board` | Top-level board for a repo (e.g., Vidkosha Cortex Vision Board). | `board_id`, `title` | `metadata` (description, repo URL, default branch) |
| `workstream` | A coherent stream of work contributors can pick up. | `workstream_id`, `title`, `status`, `difficulty`, `owner`, `created_at`, `updated_at` | `labels`, `metadata` |
| `context_doc` | A versioned snapshot of a workstream context file. | `context_id`, `path`, `title`, `body`, `version`, `source`, `last_synced_commit`, `created_at`, `updated_at` | `metadata` (parsed sections, branch name, editor, etc.) |

**Field conventions**

- `board_id`: string (e.g., `"default"`).
- `workstream_id`: slug (e.g., `"operator_marketplace"`, `"routing_semantic_v2"`).
- `status`: `"idea" | "planned" | "active" | "paused" | "done"`.
- `difficulty`: `"S" | "M" | "L"`.
- `owner`: GitHub handle or `"seeking owner"`.
- `labels`: JSON array of strings (e.g., `["helix","dao","marketplace"]`).
- `source`: `"git" | "agent" | "user"`.
- `last_synced_commit`: git SHA at time of sync.
- `created_at` / `updated_at`: ISO-8601 UTC strings.

### 2.2 Edge Types

| Edge | From → To | Semantics |
| --- | --- | --- |
| `IN_VISION_BOARD` | `vision_board` → `workstream` | Workstream is listed on that board. |
| `HAS_CONTEXT` | `workstream` → `context_doc` | Current canonical context doc for the workstream. |
| `EVOLVES_FROM` | `context_doc` → `context_doc` | Newer context version evolves from an older one. |
| `RELATES_TO_FILE` | `context_doc` → `artifact` | Context mentions or is responsible for a code/doc artifact. |
| `HAS_ISSUE` | `workstream` → `artifact` | Workstream is associated with a GitHub issue/PR artifact. |

Edges carry minimal metadata (`created_at`, optional `notes`, `weight`).

---

## 3. Source-of-Truth Strategy

- **Git (repo)** remains the canonical editable source for:
  - `contexts/vision_board.md` (human-friendly Vision Board).
  - Workstream context markdown files (e.g., `contexts/workstreams/operator_marketplace.md`).
- **Helix** mirrors this planning layer into a graph that agents query by default.

Rules:

- Humans edit markdown; a sync tool pushes structured planning data into Helix.
- Agents consume planning state from Helix and only touch markdown with explicit human approval.
- Staleness is detected via `last_synced_commit` on `context_doc` versus current git HEAD.

---

## 4. Planning-Sync Workflow

A `planning-sync` command keeps Helix in sync with the repo.

### 4.1 Inputs

- `contexts/vision_board.md` – table of workstreams.
- Workstream context files under `contexts/` (e.g., `operator_marketplace.md`, `helix_graph_neighbors.md`, `node_operator_kit.md`, `routing_semantic_v2.md`, `agentcreator_compiler.md`).
- Current git commit SHA (e.g., from `git rev-parse HEAD`).

### 4.2 Parsing `vision_board.md`

- Read the **Workstreams Overview** table.
- For each row:
  - `Workstream` → `title`.
  - Derive `workstream_id` via slug (lowercase, replace spaces/`&` with `_`, strip punctuation).
  - `Status` → `status`.
  - `Difficulty` → `difficulty`.
  - `Owner` → `owner`.
  - `Context` → `path` (strip backticks, e.g., `contexts/workstreams/operator_marketplace.md`).
  - `Suggested Agents` → attach to `labels` or `metadata.agents`.
- Upserts:
  - `vision_board` node (`board_id = "default"`).
  - One `workstream` node per row (keyed by `workstream_id`).
  - `IN_VISION_BOARD` edges from board → each workstream.

### 4.3 Parsing Workstream Context Files

- For each `contexts/*.md` workstream file:
  - Extract:
    - `title` from first heading (`# Workstream: ...`).
    - Sections (`## 1. Summary`, `## 2. Goals & Non-Goals`, etc.) into structured text.
  - Construct `context_doc` fields:
    - `context_id` = file path (e.g., `"contexts/workstreams/operator_marketplace.md"`).
    - `path` = same as `context_id`.
    - `title` = heading.
    - `body` = full markdown.
    - `source` = `"git"`.
    - `last_synced_commit` = current git SHA.
  - Versioning:
    - Query Helix for an existing `context_doc` with this `context_id`.
    - If none: `version = 1`.
    - If exists and `last_synced_commit` differs: `version = previous_version + 1` and create a new node; add `EVOLVES_FROM` new → old.
    - If exists and `last_synced_commit` matches: optionally update timestamps/metadata only.
- Upserts:
  - New `context_doc` node when content/commit changed.
  - `EVOLVES_FROM` edge when a new version is created.

### 4.4 Linking Workstreams to Context Docs

- For each workstream row:
  - Find the corresponding `workstream` node.
  - Find latest `context_doc` for its `path` (by `version` or `updated_at`).
  - Ensure a `HAS_CONTEXT` edge from `workstream` → latest `context_doc`.

### 4.5 Linking Context Docs to Files and Issues (Optional v1)

- While parsing `body`:
  - For each inline code path `` `src/...` `` or `` `contexts/...` ``:
    - Upsert an `artifact` node (kind `file`, with `path`/`uri`).
    - Add `RELATES_TO_FILE` edge from `context_doc` → that `artifact`.
  - For each GitHub issue/PR URL:
    - Upsert `artifact` (kind `github_issue` / `github_pr`).
    - Add `HAS_ISSUE` from `workstream` → that `artifact`.

---

## 5. Agent Behavior on the Planning Graph

### 5.1 Listing Workstreams (Vision Board)

- Query Helix for `vision_board` with `board_id = "default"`.
- Traverse `IN_VISION_BOARD` to collect `workstream` nodes.
- Present:
  - `workstream_id`, `title`, `status`, `difficulty`, `owner`, `labels`.
- Optionally sort by `status` (active first) and then by difficulty.

### 5.2 Rehydrating a Workstream

Given a user intent like "work on operator marketplace":

1. Locate `workstream` by `workstream_id` or fuzzy match on `title`.
2. Follow `HAS_CONTEXT` to fetch the latest `context_doc`.
3. Summarize:
   - Summary/goals/non-goals from parsed sections or top of `body`.
   - Current state and key files (`RELATES_TO_FILE`).
   - Open questions and acceptance criteria.
4. Propose a short, concrete implementation plan tied to actual files and commands.
5. Optionally show recent history by following the `EVOLVES_FROM` chain.

### 5.3 Detecting Staleness vs Git

- Check `context_doc.last_synced_commit`.
- If different from the repo’s HEAD (when known), agents should say:
  - "Planning graph was last synced at commit `<sha>`. If markdown has changed since then, run `cargo run -- planning-sync` to refresh Helix."

---

## 6. CLI & Module Design (For Implementation)

Planned Rust integration (not yet implemented):

- Module: `src/planning/mod.rs`
  - Types:
    - `WorkstreamSpec { workstream_id, title, status, difficulty, owner, labels, context_path }`.
    - `ContextDocSpec { context_id, path, title, body, sections, source, last_synced_commit }`.
  - Functions:
    - `load_vision_board(path: &Path) -> Vec<WorkstreamSpec>`.
    - `load_context_docs(dir: &Path) -> Vec<ContextDocSpec>`.
    - `sync_to_helix(workstreams, contexts, commit_sha, helix_client)`.

- CLI subcommand (in `src/main.rs`):
  - `planning-sync`:
    - Reads git HEAD SHA.
    - Calls `load_vision_board` + `load_context_docs`.
    - Calls `sync_to_helix`.
    - Prints summary: counts of boards, workstreams, context docs, and upserts.

---

## 7. Notes & Next Steps

- This spec is intentionally additive and does not change existing memory or DAO schemas.
- Once the schema is reflected in `contexts/helix_schema.md` and the `planning-sync` tool exists, agents can rely on Helix as the primary source for planning state.
- Future extensions:
  - Link `workstream` to `usage_event` / `payout_event` for economics-aware planning.
  - Add `proposal` links so DAO governance decisions about workstreams are queryable from the same graph.
