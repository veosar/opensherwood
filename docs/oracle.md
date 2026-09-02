# Oracle: using the original game as ground truth

The original executable (GOG build, SHA-256 `1d64cf088f1202e67045759fe23aaa879434ea662a922e93cff537a839da12b5`,
image base `0x00400000`, relocations stripped, no ASLR) is used in three ways, in this order of trust:

1. **Frida hooks** on a small number of functions identified by the analyst role (ADR-0003): the central
   simulation update and the player actor's position / order fields. Time functions (`timeGetTime`,
   `GetTickCount`) are hooked so each oracle step sees a controlled clock. Output: one JSON line per tick
   with actor identities (mission id or creation ordinal, never pointers), positions, facing, order, movement
   state, animation frame, and RNG draw counts if a narrow RNG hook exists.
2. **Memory scanning** (Cheat Engine / pymem style) only to discover and validate candidate layouts.
3. **The built-in console** (`docs/original/console-commands.md`) for visual and semantic cross-checks:
   `EULER`, `MOTION`, `CESTLAZONE`, `LIGHT`, `SEEKANDDESTROY`, `RAILROAD`, `PROJECTION`, `ELEVATION`,
   `BIG BROTHER`, `STATUS PC`, `REPORT`, `LEVEL TEXT`, with `HIDEINTERFACE` for clean captures.

## Public artefacts

Only the trace schema (`oracle/schema/trace-v1.md`) and procedures live in the repository. Address maps,
Frida scripts that embed addresses, Ghidra projects and any captured frames stay private on the analyst's machine.
Normalised traces (no pixels, no strings copied from the game) may be shared with the implementer.

## First experiment (M1 gate)

Mission `EmbTut_FoC_EC`, one left-click movement (the original walks on a left click, `ui-flow.md` 9.4):

1. Fingerprint the executable and the data manifest.
2. Analyst identifies the central update function and the player actor's stable id and position fields.
3. Hook the time functions; pause at the update boundary; advance under harness control.
4. Capture per tick: tick number, controlled time, actor id, position / elevation / facing, order, movement
   state, animation frame.
5. Inject one fixed pointer move + left click in a fixed-size window.
6. Record 200 steps. Validate the destination against the game's own actor overlay (`BIG BROTHER`) and the
   screen projection.

Goal: a trustworthy trajectory with a proven clock boundary and two independent validations of the actor fields.
Do not assume original frame == our tick: measure whether the original uses fixed, clamped or variable delta time
first. Exact numeric parity across MSVC 6 x87 and modern FPUs may be impossible; compare semantic transitions and
quantised values until the time model is established.

## Tooling on the analyst machine

Frida (Python `frida` 17), Ghidra 12 headless (`support/analyzeHeadless.bat`, JDK 21), cnc-ddraw windowed mode
for screenshots. Run the game from a private copy, never from the store install.
