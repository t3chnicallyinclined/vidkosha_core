# Node Operator Bootstrap Kit

Goal: let a new operator clone the repo, light up the fabric endpoints, and verify Helix reads/writes fast.

## What you get
- `.env.example` – env template for LLM, embeddings, Helix, and seed corpus knobs.
- `docker-compose.yml` – CPU-friendly embeddings service (bge-m3) by default; optional profiles for Helix and LLM if you have images/GPUs.
- `bootstrap.sh` – dependency + endpoint checks, then `cargo run -- helix-smoke` (and rich smoke if `RUN_RICH=1`).
- `ingest_seed.sh` – loads `seed_corpus.ndjson` into Helix using the configured write query.
- `seed_corpus.ndjson` – two starter memories encoding operator workflow/process.

## Prereqs
- `docker` + `docker compose`
- `cargo`, `curl`, `jq`
- Access to an OpenAI-compatible LLM endpoint (or enable the `llm` compose profile if you have GPU + weights)

## Bring-up steps
1) Copy env and tweak:
   ```bash
   cp scripts/node-operator/.env.example .env
   # set HELIX_API_TOKEN, OPENAI_BASE_URL, RAG_EMBEDDING_BASE_URL, etc.
   ```
   Then skim `contexts/helix_schema.md` (node/edge expectations, embeddings) and the front-desk guidance in `agents/agent_readme.md` so your node matches the schema and prompt discipline.
2) Start services (pick what you need):
   - Embeddings (CPU): `docker compose -f scripts/node-operator/docker-compose.yml up -d embeddings`
   - Helix (if you have a trusted image/tag): `docker compose -f scripts/node-operator/docker-compose.yml --profile helix up -d helix`
   - LLM (GPU, optional): `docker compose -f scripts/node-operator/docker-compose.yml --profile llm up -d llm`
   Or run Helix/LLM outside Compose per `README.md` instructions.
3) Bootstrap checks:
   ```bash
   ./scripts/node-operator/bootstrap.sh        # uses .env
   RUN_RICH=1 ./scripts/node-operator/bootstrap.sh  # also runs helix-rich-smoke
   ```
4) Seed the fabric:
   ```bash
   ./scripts/node-operator/ingest_seed.sh
   ```

## Notes and knobs
- `HELIX_WRITE_QUERY`/`HELIX_SEARCH_QUERY`/`HELIX_DELETE_QUERY` default to the v2 HelixQL routes; `HELIX_RICH_WRITE_QUERY` stays on v2 as well. Update if your gateway uses different names.
- Seed defaults come from `.env` (`NODE_OPERATOR_SEED_*`). Edit `seed_corpus.ndjson` to add more starter memories; `ingest_seed.sh` now embeds via `RAG_EMBEDDING_*` and writes with `write_memory_v2`.
- For Copilot/GPT guidance, use `contexts/operator_prompt.md` and the Copilot Playbook in `README.md`.
- Always log changes in `contexts/changelog.md` with three `Next up` bullets; mirror setup changes into `README.md`.

## Keeping this kit current

- When the default LLM/embeddings model or vector dim changes, update `.env.example` + `docker-compose.yml` and note it in `contexts/changelog.md`.
- When Helix query names change (`write_memory*`, search/delete), align `HELIX_WRITE_QUERY` / `HELIX_RICH_WRITE_QUERY` defaults and refresh `ingest_seed.sh` payload fields to match.
- If seed corpus structure changes, edit `seed_corpus.ndjson` + `NODE_OPERATOR_SEED_*` knobs and keep the smoke commands in `bootstrap.sh` aligned.
- After any of the above, skim the root `README.md` to ensure its operator guidance matches this kit.
- Run `scripts/node-operator/check_env_parity.sh` to catch drift between the root `.env.example` and this kit’s `.env.example` (LLM model, embeddings model/dim, Helix query defaults).
