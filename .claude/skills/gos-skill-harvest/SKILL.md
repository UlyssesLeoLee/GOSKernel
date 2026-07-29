---
name: gos-skill-harvest
description: PROACTIVELY trigger after any testing event completes in GOSKernel — CI check passes/fails and is resolved, a cargo test run finishes, governance verify passes, or a debugging session concludes with fixes merged. Do NOT wait to be asked. Extract novel programming patterns and hard-won lessons from the session, deduplicate against existing skills, write new project skills to .claude/skills/, and report the total project skill library size in chat.
---

# GOS Skill Harvest

After a testing or debugging session, distill what was hard, surprising, or reusable into project skills so future sessions don't repeat the same discoveries.

## When to run (automatic — no user prompt needed)

Run immediately after any of these events, without waiting to be asked:
- A CI check transitions to green (verify, cargo check, governance all pass)
- A `cargo test` run completes (pass or fail+fixed)
- A debugging session ends with at least one non-trivial fix pushed
- A merge conflict resolved and fixes confirmed by CI

Do NOT run after trivial one-liner typo fixes with no investigative work behind them.

## Step 1 — Identify candidate patterns from this session

Scan the current conversation for:
- Errors that required investigation (compile errors, runtime failures, CI failures)
- Constraints that turned out to be load-bearing (e.g. ordering matters, type widths matter)
- API mismatches found (function didn't exist, signature differed from assumption)
- Architectural rules enforced by CI/governance tools
- Safety invariants specific to the GOSKernel no_std/bare-metal context
- Documentation/pseudocode that diverged from the actual implementation

For each candidate, write a one-line summary: **what the pattern is** and **why it's non-obvious**.

## Step 2 — Deduplicate against existing skills

Read the `name` and `description` frontmatter of every skill in:
- `~/.claude/skills/*/SKILL.md` (user-level global skills)
- `.claude/skills/*/SKILL.md` (project-level skills, this project)

For each candidate from Step 1:
- If already fully captured → skip, note which skill covers it
- If partially captured or outdated → update the existing skill instead of creating a duplicate
- If novel → proceed to Step 3

## Step 3 — Create new skill files

For each novel pattern, create `.claude/skills/<kebab-case-name>/SKILL.md`:

```markdown
---
name: <kebab-case-name>
description: <one-sentence trigger — specific enough that future-Claude knows exactly when to apply it>
---

# <Title>

## The rule

<What to do / what to avoid — concrete and actionable, one short paragraph>

## Why it's non-obvious

<The constraint or gotcha that makes this worth writing down — not obvious from reading the code>

## GOSKernel context

<Which crates, files, or subsystems this applies to>

## From this session

<The specific error or fix from the session that triggered this skill>
```

Keep each skill focused on **one** pattern. Don't bundle unrelated lessons.

## Step 4 — Report in chat

After creating (or skipping) all candidates, output this block in the chat window:

```
## Skill Harvest — YYYY-MM-DD

**Created:**
- `<skill-name>` — <one-line summary>

**Skipped (already covered):**
- <pattern> → covered by `<existing-skill>`

**Project skill library (.claude/skills/):**
  <skill-name>/SKILL.md     X bytes
  <skill-name>/SKILL.md     X bytes
  ...
  ─────────────────────────────
  Total: N skills, X.X KB
```

Compute total by reading each `.claude/skills/*/SKILL.md` and summing file sizes.
