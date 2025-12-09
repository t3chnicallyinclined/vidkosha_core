# Vidkosha Cortex - LLM-Friendly Changelog (powered by Nervos CKB)

Lightweight activity log optimized for retrieval by agents—especially brand-new specialists spinning up on the system. Keep entries brief, timestamped, and focused on what changed plus what happens next.

---

## Ownership & Update Triggers
- **Owner:** `CTOAgent` maintains this changelog.
- **Backups:** `SeniorEngineerAgent` (implementation changes) and `AgentCreator` (doc consistency/onboarding).
- **Update when:** any meaningful code/doc/process change occurs; include title, what/why, commit (short hash), and next steps.
- **Mirror:** When entries reference other docs (README, workstreams, schema plans), keep those in sync.

## Entry Format
```
## YYYY-MM-DD HH:MM TZ — <Title>
- **What changed:** …
- **Why it matters:** …
- **Commit:** `<short-hash>`
- **Next up:** <three+ concrete next actions>
```

Append new entries to the top so the freshest context is always first.

**Changelog update rules for all agents:**
- Add `## Next up:` **only** to the newest entry you are adding, and always spell out at least three concrete next actions (comma-separated or as a short numbered list).
- When you add a new entry, remove the `Next up` line from the previous top entry.
- Every entry must include a `- **Commit:** ` line referencing the short hash of the git commit that introduced the change. Workflow: make your edits, run tests as needed, `git add` the affected files, `git commit -m "..."`, copy the 7-character short hash from `git rev-parse --short HEAD`, and insert it into the entry before pushing.
- When instructed to "update the changelog" (i.e., add or edit an entry), first run `git add -A`, then `git commit -m "<summary>"`, then `git push`; only after pushing do you edit this file. Apply this once per requested update to avoid loops.

<!-- Add the newest entry below this line. Older entries were intentionally pruned for the public reset. -->
