Reviewed the requested range as it existed at the start: `f79df35..a1e8c1f`.

1. **High — hostile sprite records can allocate many gigabytes or abort the process.**  
   `crates/opensherwood-formats/src/sprite_decode.rs:135-151,179-182,216-217`; `crates/opensherwood-assets/src/sprites.rs:94-125`  
   Dimensions are unrestricted `u16`; `width * height`, `rec.length`, and RGBA expansion are allocated before any aggregate limit. The cache limit is checked only afterward, and an individually oversized frame is still cached. On 32-bit this can panic from capacity arithmetic; on 64-bit it can OOM-abort.  
   **Fix:** validate all records before use with checked arithmetic, maximum dimensions/pixels/stream bytes, BKS file bounds, and `try_reserve`; reject a frame exceeding the cache limit.

2. **High — the framebuffer “cap” still permits a 1 GiB allocation from a tiny RPC snapshot.**  
   `crates/opensherwood-render/src/lib.rs:35-42`; `crates/opensherwood-core/src/world.rs:335-360`  
   `16384 × 16384 × 4` allocates 1 GiB, while snapshots accept viewports up to 32768. Silent clamping also makes the rendered viewport disagree with authoritative state and may exceed the GPU’s requested limits.  
   **Fix:** impose a checked total-byte budget, reject unsupported viewport sizes during snapshot validation, return allocation errors instead of clamping, and validate against `wgpu::Limits`.

3. **High — inline snapshot validation remains panicable and does not validate animation state.**  
   `crates/opensherwood-core/src/world.rs:335-400,564-577`; `crates/opensherwood-core/src/anim.rs:74-95`  
   `i32::MIN.abs()` can panic in validation. Validating an animation never checks its set, animation index, frame index, duration, or elapsed count. An inline snapshot with `elapsed == u32::MAX` reaches `elapsed += 1`; `frame == u32::MAX` reaches `frame + 1`. Debug builds panic while release builds wrap, violating cross-build determinism.  
   **Fix:** use range checks or `unsigned_abs`, validate animation state against the attached catalog, and make `advance` defensively checked even after validation.

4. **High — replay playback and recording have no global memory/time budget.**  
   `crates/opensherwood-protocol/src/lib.rs:413-466,487-496`; `crates/opensherwood-app/src/engine.rs:263-288,615-711`  
   A tiny replay containing a checkpoint at `u64::MAX` drives an effectively infinite playback loop. File playback uses unbounded `read_to_string`; parsing has no byte/line/event/checkpoint limits. Recording can accumulate unlimited events and checkpoints across calls, followed by an arbitrarily large response.  
   **Fix:** define format-wide byte, line, event, checkpoint, and maximum-tick limits; enforce them during streaming parse and recording; reject durations beyond the engine budget.

5. **High — the 16 MiB request limit drains excess input into an unbounded vector.**  
   `crates/opensherwood-app/src/rpc.rs:98-111,145-160`  
   After detecting an oversized line, `read_until` stores the entire remainder in `sink`, recreating the allocation bomb the limit was meant to prevent. Window mode additionally terminates its reader silently on this error while remaining permanently in controlled mode.  
   **Fix:** drain with `fill_buf`/`consume` or a fixed scratch buffer; communicate transport errors/disconnection to the event loop and define whether it exits or resumes interactive ticking.

6. **Medium — replay header fields are ceremonial, and terminal events can be skipped.**  
   `crates/opensherwood-protocol/src/lib.rs:319-359,487-496`; `crates/opensherwood-app/src/engine.rs:668-706`  
   Playback ignores `viewport`, `tick_rate`, and `rng_streams`; validation accepts arbitrary positive values and arbitrary algorithms/seeds/stream IDs. `last_tick()` returns an event’s tick rather than `tick + 1`, so an event-only replay drops its final event. Tick-zero checkpoints are never compared.  
   **Fix:** either initialize from every header field or reject anything differing from current configuration; validate named RNG streams fully; compute the checked maximum of `event.tick + 1` and checkpoint ticks; compare tick-zero checkpoints before stepping.

7. **Medium — snapshot restoration is not version-complete or transactionally safe at session level.**  
   `crates/opensherwood-core/src/world.rs:599-616`; `crates/opensherwood-app/src/engine.rs:525-553`  
   `Snapshot.hash_schema` is never checked. For a different scenario, the session resets and mutates assets/world before `World::restore` checks snapshot version and ruleset, so rejection can leave the session changed. Inline snapshots also carry no content/catalog fingerprint.  
   **Fix:** validate the complete envelope before I/O, build replacement world/assets in temporary state, then atomically swap; bind data-backed snapshots to the content fingerprint or restrict inline restoration.

8. **Medium — the content fingerprint still accepts changed game data.**  
   `crates/opensherwood-assets/src/lib.rs:242-280`  
   Files over 1 MiB hash only their first and last 64 KiB. Same-sized modifications in the middle of `.dic`, `.bks`, archives, maps, or scripts are invisible. Read/seek failures silently contribute an empty/partial digest. This contradicts the replay guarantee that different data sets are rejected.  
   **Fix:** stream-hash every resolved file completely and return errors; cache completed digests using carefully validated metadata if startup cost matters.

9. **Medium — dictionary/bank integrity is not established when the bank opens.**  
   `crates/opensherwood-formats/src/dic.rs:58-85,98-133`; `crates/opensherwood-formats/src/sprite_decode.rs:64-89`; `crates/opensherwood-assets/src/sprites.rs:49-79`  
   `pages.frame_count` is not compared with the parsed frame table. The loader does not verify page indices, dimensions, first offset, the complete offset chain, or final extent against BKS length. Bad records remain latent because only requested frames are decoded—and renderer errors are silently converted to fallback circles.  
   **Fix:** validate the entire metadata table at open and distinguish missing sprites from corruption.

10. **Medium — hostile RHS origins can overflow during catalog construction.**  
    `crates/opensherwood-app/src/engine.rs:43-60`; `crates/opensherwood-formats/src/anim_table.rs:306-315`  
    `u32` origins are cast to `i32` and subtracted without checks. Large values can wrap during the cast and overflow the subtraction.  
    **Fix:** calculate in `i64`, validate representability, and make catalog conversion return `Result`.

11. **Medium — walk animations contradict the documented timing representation.**  
    `crates/opensherwood-app/src/engine.rs:55-59`; `crates/opensherwood-core/src/anim.rs:73-95`  
    Retail walk frames have zero tick duration and a high-half distance advance. The conversion changes zero into one tick and discards the advance, producing deterministic but incorrect playback.  
    **Fix:** retain both timing components. After behavioral confirmation, add an authoritative distance accumulator to animation state, snapshot, and hash; until then document this explicitly as placeholder behavior.

12. **Medium — resize, fullscreen, and focus changes leave canonical input stale or stuck.**  
    `crates/opensherwood-app/src/window.rs:338-345,499-520,531-555`  
    Resizing/fullscreen changes the letterbox transform without re-emitting the stationary cursor’s logical position; mouse presses use the previously stored world pointer. Focus loss emits no releases, leaving buttons/modifiers/arrows held indefinitely. F11 press is consumed, but its release becomes an unmatched `KeyUp(Function(11))`.  
    **Fix:** track physical pressed state, synthesize releases on focus loss, consume both F11 transitions, and enqueue a freshly transformed pointer move on resize/fullscreen and immediately before button events.

13. **Medium — presentation color and surface-error behavior are backend-dependent.**  
    `crates/opensherwood-app/src/window.rs:47,125-129,206-218,246-259,292-299,559-566`  
    The first advertised surface format is accepted without choosing a color space, while the source texture is always `Rgba8Unorm`. An sRGB swapchain therefore gamma-encodes bytes treated as linear; other backends may look different. Zero-size minimization is converted to 1×1 while continuous polling/redrawing continues, and timeout/OOM errors become repeated log messages.  
    **Fix:** select an sRGB surface and `Rgba8UnormSrgb` source consistently, suspend configuration/rendering at zero size, treat timeout as transient, and exit on OOM.

14. **Medium — looped music buffers the entire decoded track, and failed transitions retain old music.**  
    `crates/opensherwood-audio/src/lib.rs:81-96`; `crates/opensherwood-app/src/engine.rs:191-231`  
    Rodio’s `repeat_infinite()` uses a buffered source, so a full decoded music track accumulates in memory. The whole compressed file is already held in a `Vec`. The old player is stopped only after the new decoder succeeds; missing, unsupported, or unmapped scenario music therefore leaves the previous track playing.  
    **Fix:** use `Decoder::new_looped` over a bounded seekable stream/file and explicitly stop or transition the previous track before handling missing/invalid replacements.

15. **Medium — the cross-platform digest does not cover the newly claimed deterministic paths.**  
    `harness/tools/golden_digest.py:26-43`; `harness/tests/data/test_map_view.py:14-35`; `.github/workflows/ci.yml:50-52`  
    Only every 50th hash and the final hash are stored, allowing a transient divergence to reconverge undetected. The synthetic scenario exercises neither camera movement nor sprites, and the data-backed test compares only two runs on one host and still passes if sprite decoding fails into fallback circles.  
    **Fix:** store a cumulative digest of every per-tick hash; add asset-free synthetic background/catalog/sprite cases plus camera, replay, and restore sequences; assert that sprite frames were actually supplied.

16. **Medium — the Python deadline still does not cover the complete call.**  
    `harness/opensherwood_harness/rpc.py:151-176`  
    The timeout starts only after synchronous `stdin.write/flush`; a wedged child that stops reading can still hang there. Malformed JSON or an ID mismatch also leaves the transport alive and potentially desynchronized.  
    **Fix:** bound request size, perform writes through a timed worker/OS-specific nonblocking mechanism, and kill/reap the process on malformed or mismatched responses.

17. **Low — protocol capability and snapshot FIFO claims are inaccurate.**  
    `crates/opensherwood-app/src/engine.rs:426-431,510-518`  
    `hello.capabilities` omits replay despite exposing replay methods. Snapshot eviction uses lexicographic `BTreeMap` order, so after `snap-9`, `snap-10` is considered older than `snap-2`; it is not FIFO.  
    **Fix:** advertise `replay`; use `VecDeque` or a numeric insertion-order index for handles.

18. **Low — CI is locked only partially.**  
    `.github/workflows/ci.yml:18-45`; `harness/requirements.txt:1-5`  
    Actions use mutable tags including `dtolnay/rust-toolchain@master`, runner images are floating, and only top-level Python dependencies are pinned without hashes or transitive resolution. Thus the M0 “reproducible CI” disposition remains overstated.  
    **Fix:** pin action commit SHAs and a fully resolved hash-locked Python requirements file; use explicit runner images where practical.

**Verdict: fix-then-merge.** The ordinary integer camera/state/hash path looks deterministic, and the overall architecture does not require redesign. However, M0 findings 4, 6, 9, 11–13, 16, 17, and 20 are not fully closed, and the hostile-input/replay issues are release blockers.

The branch advanced during review with the excluded WIP; this assessment remains pinned to `a1e8c1f`. The later `d37c02c` commit fixed the NumPy/OpenCV pin conflict that existed at that point, so I did not count it as an open finding. The repository-mandated Claude cross-check was attempted but blocked by the environment’s firewall. No files were modified.