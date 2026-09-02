# Runs a maximum-effort, read-only Codex review of the whole repository and writes the result under
# docs/decisions/reviews/. Detached from the session that launched it (see AGENTS.md, cross-agent-review skill).
#   powershell -ExecutionPolicy Bypass -File scripts/codex_full_review.ps1
$ErrorActionPreference = "Continue"
$repo = Split-Path -Parent $PSScriptRoot
Set-Location $repo
$date = Get-Date -Format "yyyy-MM-dd"
$out = Join-Path $repo "docs\decisions\reviews\$date-codex-full-review.md"
$log = Join-Path $repo "harness\out\codex_full_review.log"
New-Item -ItemType Directory -Force (Split-Path $log) | Out-Null
$prompt = @"
You are performing the complete, maximum-effort independent review of this repository described in
docs/decisions/reviews/2026-09-02-full-review-brief.md. Read that brief first, then AGENTS.md, docs/legal.md,
docs/decisions/ADR-0003-clean-room-roles.md, docs/architecture.md, docs/roadmap.md, docs/harness.md, every
docs/formats/*.md, docs/original/*.md, the previous reviews in docs/decisions/reviews/, then every Rust crate under
crates/, the Python harness under harness/ (including harness/tools and harness/tests), scripts/, and
.github/workflows/ci.yml. Use `git log --stat` and `git log -p` where provenance matters, `cargo tree` for
licences, `cargo test --workspace` and `python -m pytest harness/tests/synthetic` to check what passes
(OPENSHERWOOD_GAME_DIR is C:\Users\przem\source\gamedata\robinhood for the data-backed tests; you may run them).
Produce the five deliverables listed in the brief as one long, well-structured markdown document: findings with
severity and file:line, verification of every format spec status, legal/clean-room audit of the committed history,
architecture and roadmap assessment with the recommended order of work, and a prioritised fix list. Be exhaustive
and concrete; the maintainer will act on this document without you. Do not modify files.
"@
codex exec -s read-only --skip-git-repo-check -c model_reasoning_effort="xhigh" -o "$out" $prompt *> $log
# The report is left in the working tree: a human (or the lead agent) reads it and commits it deliberately.
if (Test-Path $out) { Write-Host "review written to $out" }
