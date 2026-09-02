---
name: oracle-capture
description: Analyst-role procedure to obtain ground truth from the original game (console overlays, screenshots, Frida tick traces) without contaminating the implementer. Use when a spec needs behavioural confirmation or when building differential tests against the original.
---

# Oracle capture (analyst role, Windows, local only)

You are acting as the **analyst** (ADR-0003). Your deliverables are documents and normalised traces, never code
for the subsystem you analyse. If you have read disassembly in this session, do not implement engine code in it.

## Setup

- Work on a private copy of the game (`OPENSHERWOOD_GAME_DIR`), never on the store installation.
- cnc-ddraw `ddraw.ini`: `windowed=true`, `fullscreen=false`, `renderer=gdi` or `opengl`, `maxfps=60`,
  `devmode=true` (does not lock the cursor) so screenshots and injection are easy.
- The community developer-console executable (rhmods.com) enables F11; the retail build has the same commands
  compiled in. See `docs/original/console-commands.md`.

## Visual ground truth (no disassembly needed)

1. Start the mission, open the console, run `HIDEINTERFACE` and the overlay command (`EULER`, `MOTION`,
   `CESTLAZONE`, `LIGHT`, `SEEKANDDESTROY`, `RAILROAD`, `PROJECTION`, `ELEVATION`).
2. Screenshot the whole map by scrolling in a fixed pattern (note camera positions), save under
   `harness/captures/<mission>/<overlay>/` (git-ignored).
3. Describe what the overlay shows in the relevant spec (e.g. "MOTION draws closed polygons in red matching chunk
   `STAT` records of type 2") and record the camera positions so it is reproducible.

## Tick traces (Frida)

1. Fingerprint `Robin Hood.exe` (SHA-256) and the data manifest; traces are keyed by both.
2. Identify hook targets with Ghidra headless (`support/analyzeHeadless.bat`) in a private project directory.
   Keep the address map private (`harness/captures/private/`, git-ignored, and never pasted into the repo).
3. Hook `timeGetTime` / `GetTickCount` to control time; hook the central update to step under harness control.
4. Emit one JSON line per tick following `oracle/schema/trace-v1.md`: tick, controlled time, actors by mission id
   or creation ordinal, position/elevation/facing, order, movement state, animation frame, RNG draw count.
5. Validate two independent ways (console overlay + screen projection) before trusting a field.
6. Normalise (strip strings copied from the game, pointers, paths) before sharing with the implementer.

## Output checklist

- Spec updated with claims + provenance (build hash, method, reproduction steps, confidence).
- Traces in `harness/captures/` with a manifest; nothing committed.
- Open questions listed at the end of the spec.
