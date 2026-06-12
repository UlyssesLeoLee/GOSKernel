---
name: sync-agents-md
description: Review recent commits, PR review feedback, and session corrections for durable lessons not yet captured in AGENTS.md, then fold them in (or prune stale entries) with minimal additive edits. Use when the user asks to update/sync/refresh AGENTS.md, or at the end of a body of work that surfaced a new hard-won convention or footgun.
---

# Sync AGENTS.md from feedback

Goal: keep `AGENTS.md` a small, accurate, high-signal guide by harvesting
*durable* lessons from recent activity — without letting it grow into a
changelog or duplicating `doc/RULE_GRAPH_PRIME.md` /
`doc/GOS_GOVERNANCE_v0_2.md`.

## 1. Establish the baseline

- Read `AGENTS.md`.
- `git log -1 --format=%aI -- AGENTS.md` to find when it last changed.
  Everything after that point is "recent activity" for this pass. If
  `AGENTS.md` doesn't exist yet, treat the whole history as in scope but cap
  it (e.g. last ~20 commits) — don't try to reconstruct the project's whole
  history into one file.

## 2. Gather candidates

Pull from whichever of these are available; skip sources that don't apply
rather than erroring:

- `git log --since=<date> --oneline` on the current branch — look for
  `fix:`, `revert`, "address review", "resync", "workaround" commits. Each
  one is a candidate: *what would have prevented this, or what should the
  next agent know going in?*
- Open PRs touching this branch: `gh pr view <n> --json reviews,comments`
  (or `gh api .../pulls/<n>/comments`). Repeated reviewer asks (CodeRabbit
  or human) about the same kind of issue imply an undocumented convention.
- This session's own transcript: explicit user corrections ("don't do X",
  "always Y") *and* confirmations of a non-obvious approach the user
  validated — both are signal, not just corrections.
- Your project memory (`~/.claude/projects/<project>/memory/*.md` with
  `metadata.type: feedback`), filtered to items that describe **repo
  conventions any agent/tool would need** — not Claude-specific or
  user-relationship preferences. Those don't belong in AGENTS.md.

## 3. Classify each candidate

For every candidate, decide:

- **Already in `AGENTS.md`** → drop it.
- **Already in a governance doc** (`doc/RULE_GRAPH_PRIME.md`,
  `doc/GOS_GOVERNANCE_v0_2.md`) → at most add/adjust a pointer; don't
  restate the rule.
- **One-off / specific to a single PR or bug** → drop it. AGENTS.md is for
  things that recur.
- **New and durable** → candidate edit. Write it as 1-3 lines: the rule,
  then why (if non-obvious).

Also check existing `AGENTS.md` entries for staleness: if an entry names a
file, function, or flag, `Grep`/`Read` to confirm it still exists in that
form. If not, update or remove the entry — verify before deleting (it may
just have moved).

## 4. Apply minimal edits

- Prefer folding a new lesson into the most relevant existing section over
  adding a new top-level heading.
- Keep the whole file scannable — if it's growing past a couple hundred
  lines, that's a sign entries are too verbose or too specific; tighten
  before adding more.
- Edit `AGENTS.md` directly (it's a working-tree file, not a commit). Do
  **not** commit/push as part of this skill — leave the diff for the user
  to review, per AGENTS.md's own git workflow section.

## 5. Report

Summarize: what was added/changed/removed and why, and what candidates you
considered but rejected (briefly — one line each). If nothing qualified,
say so; an unchanged AGENTS.md is a fine outcome.
