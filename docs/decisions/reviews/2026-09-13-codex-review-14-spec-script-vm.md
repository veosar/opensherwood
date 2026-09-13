# Codex spec review 14: docs/original/spec-script-vm.md, draft at ccaf5df (2026-09-13)

Spec reviewer role (ADR-0009), with access to `re/`. Answered in the spec's next revision; the disposition is the spec's changelog.

Reviewed [spec-script-vm.md](C:/Users/przem/source/repos/opensherwood/docs/original/spec-script-vm.md), blob `92aedbf44c6e3e974bc44beb5be1c1d46b7b36c8`, against the matching executable hash. Checked every opcode handler, all 265 native registrations and wrapper arities/result conventions, and all 39 scripts. The reproducible random sample, seed `9009`, was: 16, 18, 20, 39, 60, 79, 84, 88, 103, 114, 127, 138, 148, 152, 158, 160, 170, 180, 184, 186, 187, 194, 200, 210, 215, 228, 235, 238, 246, 248. All arities match; the following findings prevent clearance.

1. **High — VM-030; natives 3, 10, 75: incorrect element-table model.**
   **Addresses:** `005709c0`, `004c2720`, `00570850`, `00571590`, `00579bd0`, `005714d0`, `004d8460`.
   Player characters are explicitly registered in the table searched by native validation. Native 75 therefore does not categorically exclude them. The fallback table searched by natives 3 and 10 is the cart-family table, not the player-character table. Correct the table construction, membership, lookup and inverse-lookup claims, including §7.5. Native 3 also fails to reject indices below −1 before indexing.

2. **High — VM-091, VM-103, VM-243: finalisation outcomes are reversed, and victory is not immediate termination.**
   **Addresses:** `004c6ef0`, `004e3220`, `004e3260`, `00404480`.
   Successful end bookkeeping leads to `Finalize(0)` and tick result 2; unsuccessful bookkeeping leads to `Finalize(1)` and result 3. Furthermore, declaring victory sets a success/context flag distinct from the finalisation flags. Replace the assertion that winning necessarily causes `Finalize(1)` on the next tick with the actual end-state transitions. Seventeen mission `Finalize` functions read this parameter.

3. **High — VM-103: missing early exits change observable scheduling.**
   **Address:** `004c6ef0`.
   `CheckVictoryCondition` returning 2 runs end bookkeeping and returns before incrementing the tick counter. Automatic defeat checks return after that increment but before element updates, sequence draining and timers. Several additional guards also bypass those later phases. The numbered tick order currently implies that execution continues through them.

4. **High — VM-218, VM-222; natives 41, 202, 203: modal completion is incorrectly specified.**
   **Addresses:** `004ca410`, `0053a1a0`, `0053a0b0`, `0053a4c0`, `0052b100`, `0052b260`, `00544b80`, `0053bb80`, `0053c390`, `00579f30`.
   These display paths enter a synchronous modal UI loop. Sequence completion occurs after that loop returns, not immediately when the page opens. Native 202 likewise does not return to the following script instruction while its modal page remains open. Correct §7.10’s rejection of “203 holds the sequence.”

5. **High — VM-086; §5: numerous `void` result declarations are wrong.**
   **Addresses, by native:** 19–22: `00404d20`, `00404d50`, `00404d80`, `00404db0`; 39–41: `00405050`, `00405080`, `00405090`; 66: `00405530`; 70–71: `004055f0`, `00405630`; 99: `00405b20`; 103: `00405bc0`; 127: `00405f70`; 129: `00405fc0`; 152–154: `004063c0`, `004063f0`, `00406420`.
   These wrappers expose the native’s low-byte result rather than replacing it with zero. Change the result conventions and specify success/failure values. Native 100 (`00405b50`, target `00571550`) and native 224 (`00406f40`, target `0057adf0`) preserve a full-word result despite also being labelled `void`. Lack of corpus result reads does not establish a void convention.

6. **High — VM-086; natives 206–208: bitwise results are full-width.**
   **Addresses:** wrappers `00406c30`, `00406c60`, `00406c90`; bodies `0057a280`, `0057a290`, `0057a2a0`.
   AND, OR and XOR return their complete 32-bit results. Neither bodies nor wrappers truncate to eight bits. Remove the truncation claim from §3.3, §5 and §7.11. Native 128’s wrapper (`00405fa0`) also preserves the full word; it is not an example of wrapper-enforced byte truncation.

7. **High — VM-001, VM-068: exceptional floating-point comparisons are wrong.**
   **Addresses:** `0063b2d0`—particularly `0063b415`–`0063b424`—and `00638440`–`00638e90`.
   The machine predicates are not the claimed blanket IEEE “unordered → false” semantics. With masked invalid-operation exceptions, unordered comparisons produce true for `<=`, `<` and `==`, and false for `>=`, `>` and `!=`. The version check similarly accepts an unordered comparison; it is not a bitwise equality check against 1.5. Specify these exceptional outcomes and the floating-point environment qualification.

8. **Medium — VM-061, VM-064: arithmetic edge cases are incomplete or incorrect.**
   **Addresses:** `00635cb0`, helper `00642b7c`; `00636680`.
   Float-to-int conversion uses a 64-bit integer conversion whose low word is retained. It does not uniformly return `0x80000000` outside the signed 32-bit range: the exactly representable input 4294967296 produces zero. Also document the integer-division trap for `INT_MIN / −1`, not only division by zero.

9. **Medium — VM-003: a supposedly unused function-header field is read.**
   **Addresses:** `0063b070`, `00639300`, `0063a250`, `00634d30`.
   Callback entry reads `size_of_volatile` from the function record and uses it for initial local storage allocation. Opcode 0x03 subsequently replaces that allocation. Distinguish this from the other unused metadata and from normal retail execution, where the prologue agrees with the header.

10. **Medium — VM-081: immediate result retrieval is not mandatory.**
    **Addresses:** `00634de0`, `00635090`.
    The result slot persists until overwritten. Unrelated intervening instructions do not invalidate it. Replace “must directly follow” with the actual overwrite condition; direct adjacency is a corpus observation.

11. **High — VM-122: the engine does originate messages.**
    **Addresses:** `0050f710`, call sites `0050f8bb` and `0050fd58`; target `00578d80`.
    Both call sites send message **1001 to the level**, subject to their surrounding state guards. Remove “the engine itself sends none” and specify these engine-originated notifications and their trigger conditions.

12. **High — VM-091, VM-092, VM-108: the second AI callback path is mischaracterised.**
    **Address:** `0040dcb0`.
    This dispatcher produces seven codes, **100–106**, not five codes 102–106. Its first argument is not universally governed by the general dispatcher’s “source code 3, otherwise zero” rule. It also ignores the callback return value. Restrict the “zero drops the event” rule to the path that actually checks it, and supply the missing externally meaningful event semantics.

13. **High — §5 native 210; §6.2: the claimed repeated-first-character test is a decompiler artefact.**
    **Address:** `0057a4c0`, especially `0057a5f1`–`0057a61d`.
    The machine code indexes successive player characters. It returns whether any inspected character satisfies either skill test. Remove the claim that the loop ignores its index and the corresponding open question. This native is called eleven times and its result is read every time.

14. **High — §5 natives 18/19; §4 and §7.11: camera movement parameter mislabelled as zoom.**
    **Addresses:** `00571270`, `00571330`, `00571200`, `004cdfc0`.
    The value written by 18/19 controls camera movement scaling; it is not the zoom target written by native 21. Remove “zoom target 2.0,” “as 18 with zoom target f,” and the “default zoom set by 18” constant. Re-establish the movement behaviour and completion effects from its consumer.

15. **Medium — VM-095, VM-110; native 154: native and direct zone-entry preconditions differ.**
    **Addresses:** `00577390`, `0057fcc0`, `00577220`, `0057fdb0`.
    Native 154 calls the entry dispatcher only when its membership search already finds the actor. An absent actor produces an error and no entry callback. The direct entry dispatcher can add an absent actor. Specify this discrepancy rather than treating native 154 as an ordinary addition.

16. **High — VM-100, VM-101, VM-104, VM-222: clock and pause claims overreach their evidence.**
    **Addresses:** `0050f710`, `005105d0`, `004c8380`, `004cdfc0`.
    The 40/400 ms waits have guards; they are not unconditional frame guarantees. `PostInitialize` follows the *attempted* first tick call, which can be skipped. Camera completion occurs in the rendering/update path and can advance sequences outside the listed level-tick phases. Specify these separate execution opportunities and their guards before asserting “no script activity” whenever the level tick is skipped.

17. **Medium — VM-103, VM-221: timer phase and overflow behaviour need correction.**
    **Addresses:** `004c6ef0`, `00575350`, `00586f10`, `00587160`.
    Timers added before the timer pass can be counted in that same tick. Timers appended by completion callbacks during the pass are outside its captured visit count. Specify simultaneous completion order and this insertion boundary. Zero and negative counters also wrap as 32-bit values; “never completes” is not literally correct.

18. **High — VM-210–VM-217: the sequence state contract is incomplete.**
    **Addresses:** `00582560`, `00585570`, `00585320`, `004646e0`.
    Launch does not simply mark every element running: queued elements remain pending until admission. The abort cascade depends on propagation flags; it is not an unconditional consequence of every refused/cancelled state assignment. Specify the externally observable propagation cases, synchronous completion order, and behaviour when callbacks mutate or launch sequences during dispatch. Also settle 16-bit barrier-level overflow at `00571100`.

19. **Medium — VM-093–VM-095: callback context is dynamically inherited.**
    **Addresses:** `00467230`, `004ca410`, `004ba760`, `004ba5c0`, `005798c0`.
    Actor-target messages install the target as current actor; level-target messages do not install a replacement. A nested message invoked during a scroll callback can still observe the enclosing current scroll. Replace “native 192 returns null in every other callback” with context-lifetime rules, including nested callbacks and restoration.

20. **High — VM-089; §7.12: universal safe-error claims contradict the executable.**
    **Addresses:** `00571590`, `00571760`, `00570e20`, `005781f0`, `00578680`, `005f8030`, `00639370`.
    Bad arguments can cause unchecked accesses, execution after an error report, arithmetic traps, fatal termination or nontermination. The spec already admits several such cases elsewhere. Replace “the original never traps” and “every bad handle or index logs and returns” with per-operation failure classifications.

21. **Medium — VM-051, VM-071; §7.1/13 and §8: corpus validation contains false statements.**
    **Evidence:** fresh probes over all 39 SCBs; relevant interpreter addresses `00635150`, `00634de0`, `00635090`.
    The aggregate totals match: 208,679 instructions, 42,734 native calls and 192 ids. However:
    - Maximum observed native arity is **6**, not 8.
    - The 79 non-NOP successors of opcode 0x07 are not all jumps; 47 are immediate loads.
    - Many callbacks contain no value-return opcode, including all 39 `Finalize` and all 39 `PostInitialize` functions.
    - Natives **162, 241 and 262**, labelled unused in their rows, each occur once.

    Correct the validation statements and distinguish static instruction adjacency from executed control flow.

22. **High — §5/§6: mission-used native semantics remain unavailable to implementers.**
    **Addresses:** among others `00579d70`, `00579ef0`, `005714a0`, `00579f10`, `005795b0`, `0057b2b0`, `0057bfe0`, `00572300`, `00571fb0`.
    Unsettled natives include 13, 16, 17, 162, 173, 241 and 261; several return values directly consumed by scripts. Native 173 alone has fourteen result-reading calls. Descriptions such as “a campaign state byte,” “forwards,” or an unidentified property are not implementable semantics. Resolve them, or explicitly exclude them from clearance and assign claim status and assumption mappings.

23. **High — VM-108, VM-216, VM-231; §6.1: necessary subsystem interfaces are missing.**
    **Addresses:** `00410620`, `004646e0`, `0046b210`, `00467a50`, `0046bd40`.
    An implementer still lacks meaningful AI-event codes, admission/priority rules, movement failure and arrival conditions, animation-loop completion, speech completion, and corresponding cancellation notifications. These may belong in separate specifications, but this specification needs explicit, reviewed dependencies. They cannot simultaneously remain unread and support the claim that every native and scheduling rule is settled.

24. **High — VM-010–VM-013 and the specification as a whole: missing acceptance and state contracts.**
    **Addresses:** `006390b0`, `0063a250`, `0063a320`, `00585320`, `004c6ef0`; documentation-level omissions otherwise have no executable address.
    There are no synthetic acceptance cases with expected outcomes. Corpus statistics do not substitute for them. Add cases for nested callbacks, persistent returns, simultaneous timers, modal suspension, abort propagation and invalid inputs. Specify snapshot-visible state and restoration boundaries, RNG ownership/seeding, and the intentional departures from original behaviour. Unknowns need explicit `Assumption` mappings.

25. **High — VM-002, VM-031, VM-204/VM-230: expression filter fails on unjustified internal structure.**
    **Addresses:** `0063a940`, `00639370`, `00570a70`–`00570cb0`, `005866a0`, `00585b70`.
    Offending text includes:
    > “is stored in a 12-byte slot: the opcode at byte 0, the 8 operand bytes at bytes 4..11”

    > “Element kinds are read from a type word of the element; the natives test it with masks.”

    > “Elements are created with a *kind* code … and typed parameter slots”

    The SCB’s nine-byte encoding and native ids are interoperability interfaces. The recovered heap layout, internal type representation and comprehensive internal element-kind organisation are not justified as required interfaces. Keep those details in analyst notes; express required categories, effects and ordering independently.

26. **High — §5 natives 91/126: internal translation tables substitute for behaviour.**
    **Addresses:** `005762d0`, `00575e20`.
    Offending text includes:
    > “internal states 1..6 map to 1, 2, 3, 6, 4, 5; else 0.”

    Native 91 similarly enumerates internal movement states and their output translations. These reproduce internal case mappings while leaving the implementer without the corresponding observable conditions. Replace them with externally meaningful state descriptions. The opcode and native-id interface tables themselves are not objectionable merely because they are tables.

27. **Medium — VM-005, VM-091: compatibility-token exceptions are unmarked.**
    **Addresses:** `004c0510`, callback invokers listed in VM-091.
    The opening assertion “no identifiers or strings of the binary” is too absolute when the document includes `StartUp`, `Initialize`, `ProcessMessage` and other exact callback names. These spellings are necessary compatibility tokens; mark that exception individually and explain their file/interface role, as ADR-0009 requires.

28. **High — identity block and necessity record: present in part, but insufficient for handoff.**
    **Claim/address:** document-level; no executable address.
    The build hash matches, `draft` honestly records pending review, and the absence of oracle recordings is disclosed. However, there is no unique analyst-session identifier, reviewer identity, exposure record, separate publication-approval status, or explicit analyst-authorisation statement. The stopping assertion—
    > “Reading stopped when every opcode, every native id and the scheduling rules were settled”

    —contradicts both the open questions and the findings above. Replace it with the actual completed scope and unresolved dependencies; record factual review and publication approval separately.

**Verdict: fix-then-clear.**