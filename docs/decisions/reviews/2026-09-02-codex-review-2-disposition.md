# Disposition of Codex review 2 (2026-09-02, post-M0 code)

Full text: `2026-09-02-codex-review-2.md` (18 findings, verdict fix-then-merge). Status after the same-day fixes:

| # | Finding | Status |
|---|---|---|
| 1 | Hostile sprite records allocate unbounded | Fixed: the whole frame table is validated when the bank opens (dimensions <= 4096, stream <= 64 MiB, offsets inside the bank, page indices, frame count vs `.dic` trailer), `try_reserve` for streams, oversized frames are never cached. |
| 2 | Framebuffer 1 GiB from a snapshot viewport | Fixed: viewport validated <= 4096 per side; framebuffer dimension budget 4096 (64 MiB). |
| 3 | Validation panics / animation state unchecked | Fixed: `unsigned_abs`, animation indices/frames bounded in validation, `advance` saturating and modulo. Catalog-aware validation: deferred (catalog is not part of the snapshot). |
| 4 | Replay without budget | Fixed: format limits (64 MiB, 2^20 events, 2^16 checkpoints, tick <= 2^24) enforced in the parser; playback is bounded by them. Recording caps: deferred. |
| 5 | Oversized-line drain allocates | Open (the drain still uses `read_until`); tracked. |
| 6 | Replay header fields ceremonial; last event skipped | Partially fixed: `last_tick` now simulates `event.tick + 1`; header viewport/tick-rate/streams enforcement: open. |
| 7 | Session-level restore not transactional | Open; tracked (needs a staged-session refactor). |
| 8 | Fingerprint partial digests | Open by design for now (documented in `GameDir::fingerprint`); full streaming digest with a cache is on the roadmap. |
| 9 | Bank integrity at open | Fixed (see 1). |
| 10 | RHS origin overflow | Open; small. |
| 11 | Walk timing placeholder | Open by design; documented in `docs/formats/sprite-animations.md` and `engine.rs`. |
| 12 | Resize/focus leave input stale | Open; tracked for the window pass. |
| 13 | Surface colour space | Open; tracked for the presentation pass. |
| 14 | Looped music buffers whole track | Open; tracked for the audio pass. |
| 15 | Digest coverage | Open; the fixture still samples every 50th hash. |
| 16 | Python write not under deadline | Open; minor. |
| 17 | Capabilities / FIFO | Fixed: `hello` advertises `mission` and `replay`; snapshot ids are zero-padded so eviction is FIFO. |
| 18 | CI pinning | Open (actions by tag, requirements pinned without hashes). |

The open items are carried in `docs/roadmap.md` under "Deferred from the reviews".
