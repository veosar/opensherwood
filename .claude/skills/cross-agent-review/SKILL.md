---
name: cross-agent-review
description: Request an adversarial review from the other agent (Claude asks Codex, Codex asks Claude) for architecture decisions, milestone completions, protocol changes and risky parsers. Use before merging anything that changes ADRs, the protocol, the determinism contract or a milestone checkbox.
---

# Cross-agent review

## When

- New or changed ADR; protocol / replay / hash schema change; milestone exit; any parser for a security-relevant
  or complex format (sprite bank, SCB VM); anything you are unsure about.

## From Claude Code (requesting Codex)

```
codex exec -s read-only -C <repo> -c model_reasoning_effort="xhigh" -o /tmp/review.md \
  "Review <scope> in this repository adversarially. Read AGENTS.md first. Focus on: correctness, determinism,
   clean-room/provenance compliance, missing tests, API mistakes. Output: numbered findings with severity,
   file:line references, and concrete fixes. End with a verdict: merge / fix-then-merge / redesign."
```

For a diff-scoped review: `codex exec review` (see `codex exec review --help`) from the repo root.

## From Codex (requesting Claude)

```
claude -p "Review <scope> adversarially. Read AGENTS.md first. ..." --permission-mode plan
```

## Handling the result

- Save the review under `docs/decisions/reviews/<date>-<topic>.md` when it changes a decision; otherwise keep it
  in the PR/commit message.
- Answer every finding: fixed, rejected with reason, or deferred with an issue. Never silently ignore.
- Findings that reveal a rule gap become a line in `AGENTS.md` or a skill, not a one-off fix.
