# Test & Evaluation Framework

A companion document to `context.md` that defines how we validate the value of specialized agents created by AgentCreator. This framework provides a controlled testing methodology to quantify how much domain-specific context and agent specialization improve real-world task outcomes.

Tracking: listed on `contexts/vision_board.md` as "Specialist Evaluation Framework (AgentCreator A/B)"; roadmap Phase 3 calls for running this after AgentCreator scaffolding lands.

---

## Ownership & Update Triggers

- **Owner:** `AgentCreator` (evaluation design for specialists).
- **Backups:** `SeniorEngineerAgent` (test harness/code) and `ResearcherAgent` (methodology rigor).
- **Update when:** evaluation design changes, metrics or tasks change, test harness changes, or specialization criteria change.
- **Mirror:** Reflect updates in `agents/agent_creator.md`, `agents/agent_template.md`, and relevant code/README sections describing tests.

## 1. Overview

After AgentCreator is working, we will run controlled tests comparing baseline answers (Agent only) vs. specialized-agent answers (Agent + domain expert) on the same prompt. The goal is to quantify how much value specialization and agent-specific context add to real tasks.

---

## 2. Test Methodology

### 2.1 Two-Condition Comparison

Every evaluation runs the same prompt through two configurations:

| Condition | Configuration | Description |
| --- | --- | --- |
| **Baseline** | Agent only | General model with no custom specialist for the domain |
| **Specialized** | Agent + Domain Expert | Agent routes to a specialist created by AgentCreator with its own context file + domain RAG notes |

### 2.2 Controlled Variables

- Same underlying LLM model for both conditions
- Same prompt text
- Same temperature and inference settings
- Same session context (fresh start, no prior conversation)

---

## 3. Step-by-Step Evaluation Process

### Step 1: Pick a Domain

Select a domain that requires real structure and domain nuance, not just generic fluff.

**Example domains:**
- Tax optimization for small SaaS
- YouTube documentary script structure
- Kubernetes cost optimization for multi-tenant workloads
- Smart contract security auditing
- Open-source project governance

### Step 2: Craft a Detailed Test Prompt

Create a single, detailed test prompt that:
- Requires structured thinking
- Needs domain-specific terminology and constraints
- Has a clear success criteria
- Cannot be answered adequately with surface-level knowledge

### Step 3: Run Test A – Baseline

1. Use only Agent (no special domain agent)
2. Submit the test prompt
3. Save the full response as `baseline_output.md` in `evaluations/{domain}/`

### Step 4: Create the Specialist

Use AgentCreator to spawn a domain specialist:

1. Generate system prompt tailored to the domain
2. Generate context file following `agent_template.md`
3. Define RAG tags and suggested metadata
4. Add domain-specific notes into RAG for that agent

**Example specialists:**
- `TaxStrategyAgent`
- `YouTubeScriptAgent`
- `K8sCostAgent`
- `ContractAuditAgent`

### Step 5: Run Test B – Specialized

1. Ask Agent the exact same prompt
2. Allow routing to the new specialist
3. Save as `specialized_output.md` in `evaluations/{domain}/`

### Step 6: Compare Results

Evaluate both outputs across these dimensions:

| Dimension | What to Look For |
| --- | --- |
| **Specificity** | Domain-specific details vs. generic answers |
| **Structure** | Step-by-step clarity and logical organization |
| **Domain Language** | Correct use of terminology and constraints |
| **Practicality** | Are the steps actually executable? |
| **Completeness** | Coverage of edge cases and considerations |
| **Accuracy** | Correctness of domain-specific claims |

### Step 7: Self-Critique (Optional)

Feed both outputs to ResearcherAgent or a dedicated ReviewerAgent:
- "Which response is better and why?"
- "What did the specialist add that the baseline missed?"
- "What are the gaps in each response?"

---

## 4. Scoring Rubric

Use this rubric for consistent evaluation:

| Score | Label | Description |
| --- | --- | --- |
| 1 | Poor | Generic, surface-level, or incorrect |
| 2 | Adequate | Covers basics but lacks depth or specificity |
| 3 | Good | Solid coverage with some domain insight |
| 4 | Very Good | Strong domain expertise, practical and structured |
| 5 | Excellent | Expert-level, comprehensive, actionable |

Apply this score to each dimension in Section 3, Step 6.

---

## 5. Documentation Template

After each evaluation, document results:

```markdown
## Evaluation: {Domain}
**Date:** YYYY-MM-DD
**Test Prompt:** (brief description)
**Specialist Created:** {AgentName}

### Baseline Scores
- Specificity: X/5
- Structure: X/5
- Domain Language: X/5
- Practicality: X/5
- Completeness: X/5
- Accuracy: X/5
- **Total:** XX/30

### Specialized Scores
- Specificity: X/5
- Structure: X/5
- Domain Language: X/5
- Practicality: X/5
- Completeness: X/5
- Accuracy: X/5
- **Total:** XX/30

### Delta Analysis
- Score improvement: +X points
- Key differences observed:
  1. ...
  2. ...
  3. ...

### Conclusions
...
```

---

## 6. Integration with Roadmap

This evaluation framework activates in **Phase 3 – AgentCreator**:

1. Once AgentCreator can spawn new specialists end-to-end, immediately run the first controlled evaluation.
2. Use evaluation results to refine AgentCreator's template generation.
3. Document findings in the changelog.
4. Feed learnings back into context files.

---

## 7. Next Actions

1. Finalize AgentCreator implementation (prerequisite).
2. Select first evaluation domain.
3. Design test prompt.
4. Run baseline evaluation.
5. Create specialist via AgentCreator.
6. Run specialized evaluation.
7. Document and analyze results.
