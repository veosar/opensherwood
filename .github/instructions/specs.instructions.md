---
applyTo: "docs/original/**/*.md,docs/formats/**/*.md,docs/decisions/**/*.md,docs/legal.md"
---
# Behaviour specifications, format specs and ADRs

Source of truth: ADR-0009 (expression filter, two-tier wall), ADR-0003, `docs/original/SPEC-TEMPLATE.md`.

- Flag decompiler output or its traces: auto-generated names (`FUN_`, `DAT_`, `param_N`, `local_`, `undefined`),
  transcribed pseudocode, one instruction or basic block rewritten as one sentence, the binary's own identifiers or
  strings, game text, call-graph or class-layout structure not required by an interface.
- Flag tables of tuned values (balance numbers, colour lists, curves) carried as content. Allowed: individual
  functional facts with provenance, and data-file field to meaning mappings (interface facts) entry by entry; for a
  generated table, the rule or the data file and field it is loaded from must be named.
- A spec `docs/original/spec-*.md` must follow the template: necessity record, claims with ids, evidence and
  confidence, constants with source, acceptance tests, identity block with review and publication approvals.
  Status must not be advanced to `reviewed` or `implemented` without the review or ruleset it cites.
- Every format fact needs Provenance (observed, inferred or unknown) and, for the executable, only function
  addresses and the build hash; never a screenshot with game art.
- ADRs are never rewritten: a change of decision is a new ADR that supersedes the old one, plus a row in
  `docs/decisions/README.md`.
- Shared handoffs and notes must not contain private analysis from `re/`.
