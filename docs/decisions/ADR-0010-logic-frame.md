# ADR-0010: One logic frame of 46.875 ms drives everything

Date: 2026-09-13. Status: accepted (lead decision from the reviewed specifications; Codex review requested with
the first implementation batch).

## Context

The original program advances its whole simulation once per rendered frame: the level tick with the script
scheduler (`spec-script-vm.md` VM-100, VM-103), every actor's animation and movement (`spec-movement-animation-
camera.md` ANIM-001..004), every AI timer (`spec-ai-combat.md` AI-001..004) and the camera. Its main loop
busy-waits until the operating system's millisecond counter reports at least 40 ms since the frame began (400
ms in a debug slow-motion mode) and never catches up. On the measured host the counter's granularity makes the
realised frame 46.875 ms (21.333 frames per second), which the oracle recordings confirm (a walking frame of 4
px every 46.9 ms; the 32-frame sneak cycle of exactly 1.5 s). The realised length is host-dependent in the
original; the requested 40 ms is what the program asks for.

The current engine runs a 60 Hz world tick, a 64 Hz animation clock and a 25 Hz script clock, converted into
each other with assumptions (`TickRate`, `FrameLength`).

## Decision

1. **One clock.** The engine's simulation tick is the logic frame. Everything the specifications count in frames
   (script ticks, AI timers, animation timers, camera updates, sequence timers) counts engine ticks one to one.
   The three-clock model and its conversions are removed.
2. **Fixed length: 46.875 ms** (64 frames in exactly 3 s; a 1/64 s base with three counts per frame). This is
   an OpenSherwood decision, recorded as such in every spec's section 8: it reproduces the measured original on
   the reference host, and a fixed timestep is the only deterministic choice. The nominal 40 ms is documented
   but not used. `TICK_RATE` becomes the rational (64, 3) Hz; the harness's `tick_rate` follows.
3. **Presentation decouples from simulation.** The window renders at the display's rate and interpolates
   nothing: it shows the last simulated frame (the original drew each frame once). Input is sampled per logic
   frame as today.
4. **Consequences for the harness.** `step(n)` advances n logic frames; replays, checkpoints, auto-save periods
   and every test constant expressed in ticks are re-expressed in frames; the golden fixture is regenerated once
   with the change and the ruleset, snapshot and hash schema versions are bumped.
5. **Slow motion** (the original's debug 400 ms mode) is not implemented.

## Consequences

- All timing claims of the specifications transfer without conversion; the `TickRate` and `FrameLength`
  assumptions disappear.
- Movement speed comes from the animation's per-frame advance (ANIM-032), so the measured 85.3 px/s walk is
  4 px per frame at 21.333 frames per second, as the original does it.
- The rebuild's first batch (the script VM) lands together with this clock change, because the scheduler's
  periods (25 frames per Hourglass, 75 per victory check) are frame counts.
