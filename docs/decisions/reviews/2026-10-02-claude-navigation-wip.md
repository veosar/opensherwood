# Implementation review: navigation / RHP (uncommitted changes on `main`)

**Verdict: fix-then-merge.** The overall design is sound: the map content is shared and stays out of the snapshot, the dynamic state (`NavState`) is hashed, and walk orders become a step sequence. The cleared core algorithms match the spec: the quadrant rule, corner walks, the F1 search, the door route with NAV-123, fixture 5 with injected costs, NAV-058's two sequences including the conditional edge rule, admission, the NAV-111 reference-corner chain, and the P1/P2 order. H1–H8 block the merge. H1 and M1 each need a local redesign, not a module redesign.

Tags: **[F]** defect in the new code, **[I]** inherited or incomplete integration, **[U]** claim no spec supports, **[G]** spec gate for the analyst or maintainer.

## High

**H1 [F] Lifts and buildings cannot be routed through.**
- `map.rs:810-815` drops every door link on side B of lift and building doors, and `search.rs:912-915` drops those doors from the route's initial set. NAV-120/121 link doors that share a sector on either side; the lift sector or the building is that shared side.
- Effects: no route from one floor up the stairs to another (six maps have lifts), no way out of a building (NAV-191), no route through one.
- `nav_plan` also refuses walkers standing in a building or on the default sector (`walk.rs:243-247`).
- `build_walk_sequence` cannot take a building as the target and returns right after the first `Enter`.
- **Fix:**
  - record side-B links and initial-set entries using the run-time side-B identity;
  - let the route and the plan accept building and default-sector numbers;
  - continue the sequence after a building door;
  - implement NAV-130 step 4.
- Add tests: stairs between two floors, entering and leaving a building, a route through a building.

**H2 [F, regression from the beeline fix] Guard programs and patrols break on map-backed worlds.**
- `walk.rs:301` now leaves `target = None`, but `program_walks` (`world.rs` ~3261) still checks only `e.target.is_some()`.
- So every walk that was actually issued is treated as unreachable: the program counter advances, the guard waits 10 ticks, and `npc_gait` is never applied.
- The guard's next program point then cancels the walk still running.
- The arrival increment is gated on `walk.is_none()` (~3095), so a finished nav walk never advances the program.
- **Fix:** treat `walk.is_some()` as planned and copy `npc_gait` into the walk's gait/run. Skip guards whose walk is running. Advance the program counter or patrol index when the walk sequence itself ends: finished means next point; cancel or expired failure means skip and wait 10.

**H3 [F] Non-deterministic arithmetic.**
- `f32::hypot` uses the platform maths library, and Rust documents its precision as non-deterministic.
- It feeds wall normals, door costs, link lengths, the search's g/h and the jump-zone choice: `map.rs:580, 820, 1133, 1208, 1226`, `search.rs:494`, `fixtures.rs:40`.
- **Fix:** replace it with an f32 `(dx*dx + dy*dy).sqrt()`, which uses only correctly rounded IEEE operations.

**H4 [F] Restore validation can panic and lets impossible states through.**
- `Availability::check` (`state.rs:286-312`) compares counts only. A region's node sequences are not checked per region or as permutations, so foreign or duplicate node indices reach `map.nodes[ni]` at `search.rs:580/659/751` and panic.
- `NavState::check` (`state.rs:557-591`) does not check:
  - that `completed` is in state `Completed` (a `Queued` one deadlocks P1/P2 permanently), queue entries are `Queued`, failed entries are `FailedPending`;
  - owners < entity count, ids unique and below `next_id`, `sector`/`sector_number`/`layer` agreeing, priority ≤ 3;
  - building occupants valid, unique and in at most one building, with `displayed` a subset of occupants;
  - the `search_budget` value.
- `validate_with` (`world.rs` ~1920) skips navigation entirely when there is no `navmap`; it should require a default `NavState` then. It also does not check an entity's `sector` against its `layer`, or the door and sector indices inside walk steps.
- Add negative restore tests with crafted snapshots.

**H5 [F] Versions not bumped.**
- A new `navigation` hash part is emitted for every world, Entity and World gained serialized fields, and map-backed semantics changed.
- `hash.rs:8` (21), `lib.rs:39` (19) and `world.rs:1342` (22) are unchanged.
- Bump all three and regenerate the synthetic harness goldens. A hash-schema change requires `cross-agent-review`.

**H6 [F] Work budgets are bypassed and do not bound the real work.**
- Bypassed:
  - `plan_path_with` returns Ok without charging its caller (`world.rs:1577-1580`);
  - `walk.rs:264` and `walk.rs:510` each start a fresh 2^20 budget;
  - the per-tick dispatch (up to 1e6 units) runs outside `SimBudget`;
  - the VM's `budget_aborts` counter can no longer count anything.
- One unit does not bound real work:
  - `walls_in` allocates and zeroes an array of all of the layer's walls on every call (`map.rs` ~1035);
  - picking the next node scans the whole queue, stale entries included (`search.rs:646-655`);
  - `admits` clones the entire `NavState` for the waived door (`walk.rs:251-263`);
  - door links are built in O(D²) (`map.rs:803-826`);
  - walls and polygons are registered in every cell of their bounding box, while `MAX_OBJECTS` is 2^18 and up to 64 layers are allowed.
- A script or a modded map can stall a tick.
- **Fix:**
  - charge the caller and return `WorkExhausted`;
  - reuse scratch arrays with generation stamps;
  - use a binary heap keyed on (f, seq);
  - pass `waive_open: Option<u32>` to admission instead of cloning state;
  - group doors by sector before linking;
  - register only the cells a segment actually crosses;
  - add caps.

**H7 [F, U] `NavDoorInteraction` is documented but not implemented.**
- `vm.rs` says the door "opens on arrival"; no code does that.
- The leaf's side is chosen by layer equality (`walk.rs:228-232`), so it is always side A for same-layer passages.
- The `PassDoor` step's crossing admission then refuses the closed door, so a click on a closed door never opens it.
- **Fix:** route to the door's side that faces the walker's sector, and add an "open if admitted under the waiver" step at `near` before `PassDoor`.

**H8 [F/I] Scripted, AI and rail walks resolve their target with the player-click rules (`walk.rs:221`).**
- Where floors are stacked, they get the top layer.
- Targets that lie on no sector are refused.
- Walkers on the default sector are refused, so reinforcements entering from the map border never start walking.
- NAV-130 takes `(sector, layer)` from the order itself (`layers-and-doors.md` 1.6).
- **Fix:** add an API with an explicit `(sector_number, layer)`; use click resolution only for player input.

## Medium

- **M1 [F]** Crossing phases are inferred from position (`walk.rs` ~660-686).
  - A blocked first move skips the layer marker and still "completes".
  - If the threshold equals the endpoint, the marker fires forever.
  - The phase is not in the snapshot.
  - **Fix:** use an explicit phase in `crossing` and hash it.
  - [I] The stair gait variants are missing.
- **M2 [F]** NAV-140 step 1: a failed unstick fails the element (`walk.rs:524-531`). The spec moves the character to the centre of the box after round 50 and continues; only the step-3 failure fails the element.
- **M3 [F, latent]** The lock-pick branch reads the file's lock (`map.doors[..].lock[0]`, `state.rs:1187`) instead of the live `NavState.doors`.
- **M4 [I/U] Buildings.**
  - `active = false` stands in for "not displayed" (`walk.rs:754`), which freezes stepping and AI; `take_out` re-activates actors a script had deactivated (`walk.rs:768`).
  - The rules for carried characters and displayed marks (NAV-190/191) are missing.
  - Tenants are never placed per NAV-193.
  - CAVE ids are copied as entity slots without mapping, count check or class check (`mission.rs:669-680`, `map.rs:795-800`).
  - Reading CAVE as the tenants chunk is unsupported: `rhm.md` says it is not established. Add a data test that the CAVE count equals the building count on every map.
- **M5 [F]** 10 of the 13 new nav Assumptions are never recorded; only `walk.rs:226/636/645` record any.
  - `NavAvailabilityOrder` (`vm.rs:1463-1465`) says file order, but the code keeps the original order.
  - `NavProximity*` and `NavSliding` are absent; the blocked-movement branch clears the order instead of stopping and counting.
- **M6 [U] Provenance comments are wrong.**
  - `state.rs:133-136` says review 48 "rejected" 8.3; it actually found 8.3 ready for maintainer approval.
  - `search.rs:72-80` uses f64 products "as corrected by review 48 finding 2", but the revision-6 policy is all-f32, where fixture 6 must come out blocked; the module doc says f32 too.
  - `nav/mod.rs:3` cites revision 5 / review 37.
  - Add tests for fixture 6 and 8.5 fixture 1.
- **M7 [F, latent]** `apply_patch` re-dispatches the minimum-key request rather than the one it returned to the head (`state.rs:916-929`, `861`). Empty results go straight to `failed`, so a patch in the same tick cannot re-search them. Review 48 finding 1 is still open.
- **M8 [G/F]** These conventions are not in the spec:
  - wall normals from the polygon's signed area (`map.rs:570-599`);
  - bounding-box cell registration, row-major traversal and dedupe (`map.rs:913-931`, `1031-1061`);
  - re-enabled obstacles not moving to the end of the cell lists.
  - Add a data test that windings are uniform on all maps.
- **M9 [I/G] Bonds.**
  - Touching counts as crossing, so a step ending exactly on a bond swaps the area twice.
  - Multi-bond chain ordering and illegal-crossing recovery are missing.
  - There are no WOAW areas in `NavContent` (no height, no ground kind).
  - `parse_woaw` still reads a link count, not section 2.4's linked flag.
- **M10 [F] Missing acceptance tests.**
  - Synthetic: 7.2-1, 10, 11, 12, 13a–i, 14 (F2/F4 — essential because the original order is kept), 15c / full unstick.
  - Data: the 7.1 offset table, the TUPO counts, and `NavMap::build` plus a search on all nine maps asserting `max_search_work` stays under the budget.
- **M11 [F] Parser and map-build hardening.**
  - `rhp.rs:681` reserves 21 bytes per node; the minimum is 20.
  - FARM/AZ are not in the garbage (fuzz) test.
  - NAV-013 and passage-door types 0/3/7 are not enforced at load.
  - `NavMap::build` (`map.rs:721-742`) does not check that an edge stays in one sector and is listed at its node B, so the search can leave the sector; it also ignores bad record indices and accepts size classes below 1 (`map.rs:955-964`).
  - The module doc at `rhp.rs:4` is stale.
  - `Bond.unknown_0x0c` and the PPPP zone's `unknown_layer` are used with meanings attached (`mission.rs:636, 648`); rename them or tie them to G10.
- **M12 [U]** `nav_lift_end` is a 1 px geometric guess (`walk.rs:308-325`); 8.2 says the lift end is recorded state.
- **M13 [U]** `NavAbilities` is not an 8.7 variant, and the class mapping (`walk.rs:112-126`) is guessed.
- **M14 [F] Natives in the current tree.**
  - 186-189 with an unknown handle return `Fault::UncheckedAccess` (~1730-1735); 8.4 says the setters do nothing.
  - Native 5 is not bounded by the TUPO count.
- **M15 [F]**
  - `take(MAX_WAYPOINTS)` (`state.rs:887`) silently truncates a path; it should fail instead.
  - The `corner_walk` fallback (`search.rs:418-430`, and `reached = 0` at `777`) invents semantics.
- **M16 [U]** `nav_place_entities` (`walk.rs:75-103`) resolves invalid placements geometrically; NAV-011 says they go on the default sector. The placement's layer is not checked against the sector's layer.

**Low:**
- `layer_screen_shift` contains `* 0`, which clippy's deny-by-default `erasing_op` rejects.
- There are double blank lines that fmt will flag.
- The 8 px grid is still built for map-backed worlds.
- The map view flattens all layers into one walkable area (`engine.rs:1916`).

## Spec gates (for the analyst or maintainer, not code)

- **G1** NAV-040 puts a lift door's side B on layer `layer_count-1`; the code silently uses the lift's own layer (`map.rs:777-779`).
- **G2** NAV-200's endpoints (type 5 ends at A, types 4/6 at B) conflict with NAV-130's approach test at `far`: one crossing direction always cancels.
- **G3** NAV-040 leaf registration when the higher layer is `layer_count-1`.
- **G4** NAV-144's multi-g tie wording conflicts with test 17's expected `1, 2, 4`; the code follows the test.
- **G5** NAV-130's "entered / left side" wording.
- **G6** The cell and normal conventions of M8.
- **G7** 8.5(v), approval of 8.1 together with review 48 finding 1, and approval of 8.3.
- **G8** CAVE as the tenants chunk, and the element-index space.
- **G9** Where abilities come from, and the class mapping.
- **G10** The bond touch rule, and the layer field of PPPP zones.
- **G11** What "stands on a lift end" means.

Per review 48, none of this clears an A-to-Z mission walk, so this work should not tick any roadmap checkbox.

## Already fixed in the tree during the review (need tests)

- The walk step never completing: `walk.rs:382`, `walk.rs:570`.
- The beeline toward the final target: `walk.rs:301`; this fix caused H2.
- `nav_fail_walk` now cancels requests: `walk.rs:389`.
- `search_budget` is now hashed: `state.rs:1073`.
- Door and building natives are partly wired to `NavState`.
- A synthetic `navigation` scenario was added with `harness/tests/synthetic/test_navigation.py`.

## Verification for the parent

- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run --workspace`.
- `python -m pytest harness/tests/synthetic`, after the version bumps.
- With `OPENSHERWOOD_GAME_DIR` set: the gamedata tests and `python -m pytest harness/tests/data`.
- The new tests named under H1, H2, H4, H7, H8, M1 and M10.
