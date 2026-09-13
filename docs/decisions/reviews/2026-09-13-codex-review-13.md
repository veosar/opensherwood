# Codex adversarial review 13: the ADR-0009 governance change, commit b13372e (2026-09-13)

Disposition in `2026-09-13-codex-review-13-disposition.md`.

**The approach can be defensible, but this commit does not establish that it is legally sound.** The wall reduces copying risk; it does not independently authorize decompilation or publication. The legal conclusions below are a risk assessment, not a substitute for Polish software-copyright counsel reviewing the actual licence and analysis scope.

I reviewed `b13372e`, checked the installed Ghidra 12.1.3 source APIs, and exercised the gate with synthetic inputs in memory. I did not open `re/`, run the game, or modify files. The checkout contains a subsequent commit, `4948af1`; its relevant fix is identified below.

1. **[High] The decompilation mandate exceeds the legal justification documented for it.**
   [ADR-0009:16](C:/Users/przem/source/repos/opensherwood/docs/decisions/ADR-0009-decompilation-driven-reimplementation.md:16), [analyst skill:28](C:/Users/przem/source/repos/opensherwood/.agents/skills/analyst-ghidra/SKILL.md:28)

   “Understand each subsystem completely” and “Read everything” are unconditional instructions. Article 6 instead requires indispensability for interoperability, information not already readily available, authorized analysis, and confinement to necessary portions. Article 5(3) permits studying operation during authorized use; it is not blanket permission for static decompilation. *Top System* concerns necessary error correction, not authorization to reconstruct an entire replacement engine. [Directive 2009/24/EC, Articles 5–6](https://eur-lex.europa.eu/legal-content/EN/TXT/?uri=CELEX:32009L0024), [*Top System*, C‑13/20](https://eur-lex.europa.eu/legal-content/EN/TXT/?uri=celex%3A62020CJ0013).

   **Fix:** Require a per-investigation necessity record: interoperability target, missing information, available documentation, authorized analyst, relevant functions, and stopping condition. Identify the other programs—potentially including mission scripts—and explain the required interaction. Compatibility with “the player’s data” does not by itself establish that every combat algorithm or HUD implementation detail must be decompiled. Broader analysis needs a separately justified legal basis or permission.

2. **[High] Public specification release is treated as automatically permissible.**
   [ADR-0009:21](C:/Users/przem/source/repos/opensherwood/docs/decisions/ADR-0009-decompilation-driven-reimplementation.md:21), [legal.md:25](C:/Users/przem/source/repos/opensherwood/docs/legal.md:25)

   Polish art. 75 ust. 2 pkt 3 implements the conditional interoperability exception. Art. 75 ust. 3 restricts subsequent purposes, disclosure to others, and development of substantially similar expression. These restrictions matter even when the deliverable contains prose rather than code. Public dissemination therefore needs its own necessity assessment; neither publication nor a prohibition on publication follows automatically. [Polish Copyright Act, arts. 74–76](https://api.sejm.gov.pl/eli/acts/DU/2025/24/text.pdf).

   **Fix:** Separate private analytical evidence from specifications cleared for implementation and public release. Record why each released category of decompilation-derived information is necessary. Obtain Polish counsel’s assessment of the planned public workflow.

   GPLv3 can cover original contributions; it does not cure unauthorized copying or grant rights in the original game. Do not try to resolve the disclosure question by adding interoperability-only restrictions to otherwise GPL-licensed code.

3. **[Medium] The actual GOG/publisher contractual position is undocumented.**
   [legal.md:55](C:/Users/przem/source/repos/opensherwood/docs/legal.md:55)

   A representative GOG-published agreement prohibits reverse engineering subject to permission from applicable law, and also contemplates game-specific EULAs. That document is illustrative; it does not establish the terms accepted for this copy. [GOG-published agreement, §§2.2 and 9.1(b)](https://items.gog.com/preview/GOG_User_Agreement_EN_updated.pdf).

   Contrary contractual provisions cannot override the protected exceptions: Directive Article 8 and Polish art. 76 expressly address this. But that does not excuse conduct outside those exceptions. [Directive, Article 8](https://eur-lex.europa.eu/legal-content/EN/TXT/?uri=CELEX:32009L0024), [Polish Act, art. 76](https://api.sejm.gov.pl/eli/acts/DU/2025/24/text.pdf).

   **Fix:** Privately preserve the acquisition record and applicable GOG/publisher terms; document their versions and relevant clauses. Describe ownership as lawful access to a copy, not ownership of its copyright. Record on whose behalf analysts operate.

4. **[High] “Algorithms in our own words” is an insufficient expression filter.**
   [ADR-0009:22](C:/Users/przem/source/repos/opensherwood/docs/decisions/ADR-0009-decompilation-driven-reimplementation.md:22), [analyst skill:34](C:/Users/przem/source/repos/opensherwood/.agents/skills/analyst-ghidra/SKILL.md:34)

   The explicit bans on line-by-line paraphrases and transcribed pseudocode are good. However, nonliteral copying can survive renamed variables, prose, mathematics, or translation into Rust. *SAS Institute* distinguishes functionality from protected expression and explicitly cautions about recreating elements using acquired source/object code. Its facts involved no decompilation. [*SAS Institute*, C‑406/10, paragraphs 39–45](https://eur-lex.europa.eu/legal-content/EN/TXT/?uri=CELEX:62010CJ0406).

   **Fix:** Add these review rules, clearly identified as conservative project policy rather than claims that every listed item is copyrightable:

   | Material | Required boundary |
   |---|---|
   | Line-by-line paraphrase | Reject, including one instruction or basic block rewritten as one sentence. |
   | Internal structure | Do not prescribe recovered class layouts, function decomposition, call graphs, or branch structure unless a separately justified functional/interface requirement demands it. |
   | Identifiers | Use independently chosen internal names. Allow individually reviewed compatibility tokens where exact spelling is required; ordinary short names are not automatically protected. |
   | Constants | Permit justified individual functional facts with provenance. Reject reconstructing a copied table through many individually labelled “constants.” |
   | Constant/string tables | Reject copied content, ordering, and encoded substitutes. A formula that merely re-encodes a curated table does not satisfy the policy. |
   | Algorithms | Describe required results, state transitions, and observable ordering. Let implementers choose internal organization. |

   There is **no safe numerical granularity threshold**—functions, lines, or pseudocode steps. Detailed interface semantics can be necessary; a short description can still preserve expressive implementation choices. Independent mathematical generation of a standard table differs from disguising extracted values.

5. **[High] The reviewer exception and AI session rules leave a route around the wall.**
   [ADR-0009:29](C:/Users/przem/source/repos/opensherwood/docs/decisions/ADR-0009-decompilation-driven-reimplementation.md:29), [ADR-0003:26](C:/Users/przem/source/repos/opensherwood/docs/decisions/ADR-0003-clean-room-roles.md:26)

   Codex may review a spec against `re/` and then review implementation. Without further restrictions, that reviewer can supply code-shaped corrections derived from decompilation. Likewise, a “separate subagent” can inherit its parent’s transcript or send raw findings back to an implementing parent. A new session label alone establishes little.

   **Fix:** Define explicit roles and permitted handoffs:

   - An analyst or source-exposed reviewer produces only independently reviewed specification material.
   - A separate implementation reviewer checks code using cleared specifications.
   - Implementer sessions receive no inherited analyst context, raw tool results, private notes, shared summaries, or decompilation-bearing memory.
   - Restrict implementer filesystem/search access to exclude the analysis workspace; `.gitignore` is not access control.
   - Track exposure by subsystem and dependencies. Exposure through helper routines must count.
   - Route implementation questions through specification amendments; exposed reviewers must not send Rust patches or translated algorithms.

   A human cannot erase exposure by starting another session. Cloud-agent transcripts, logs, and retention also need treatment as analysis material; keeping files under `re/` does not contain their contents once sent elsewhere.

6. **[High] Every decompiler regex has broken word boundaries.**
   [check_no_assets.py:36](C:/Users/przem/source/repos/opensherwood/scripts/check_no_assets.py:36)

   **The committed file contains 21 literal `0x08` backspace bytes**, rather than the intended backslash-plus-`b` regex escapes. The raw-string prefix does not convert an existing backspace into a word boundary.

   The following synthetic fragment passes the actual gate despite containing three intended idiom families:

   ```
   <a one-line synthetic C fragment with an auto-named function, an undefined-typed return and an
   auto-named parameter; redacted here because the gate refuses such text>
   ```

   **Fix:** Replace the control bytes with literal `\b` sequences in the raw byte strings. Add tests exercising the actual checker with representative synthetic fragments, and reject unexpected control characters in policy source files. A green run against the repository does not test whether detection works.

7. **[High] Fixing the regex bytes still leaves substantial false negatives and false positives.**
   [check_no_assets.py:23](C:/Users/przem/source/repos/opensherwood/scripts/check_no_assets.py:23), [check_no_assets.py:83](C:/Users/przem/source/repos/opensherwood/scripts/check_no_assets.py:83)

   With only the boundaries repaired **in memory**, I reproduced:

   | Synthetic input | Result |
   |---|---|
   | C fragment with an auto-named function and an auto-named parameter only | Pass |
   | Three-family fragment padded above 4 MiB | Pass |
   | Small UTF-16 decompiler text | Pass |
   | `strings.tsv` containing exported text | Pass |
   | Small `analysis/demo.rep/idata/…/*.db` file | Pass |
   | Small `.gar` project archive | Pass |
   | Benign policy sentence naming three forbidden tokens | Reject |

   `.rep` is a **directory suffix**; checking only the tracked file’s suffix misses its contents. Ghidra’s project archive extension `.gar` is absent. C/header, XML, HTML, assembly, and string exports need not carry a forbidden extension. Additional ordinary idioms include the other auto-named local families, stack-array names, the extra-output names and the p-code helper macros. Renamed output can contain none.

   **Fix:** Check ancestor components for Ghidra project directories; cover `.gar`; inspect supported text encodings; eliminate silent size-based success; and detect exporter-specific signatures. Use bounded streaming or fail for review when inspection limits are exceeded. Replace whole-file exemptions with narrow, reviewed exceptions.

   Keep describing this as a heuristic. Generic `.gdt`/`.fidb` bans are conservative policy choices, not proof of infringement. No regex can certify that a specification avoids expressive copying.

8. **[High] The gate does not reliably inspect what will be published.**
   [check_no_assets.py:47](C:/Users/przem/source/repos/opensherwood/scripts/check_no_assets.py:47), [analyst skill:34](C:/Users/przem/source/repos/opensherwood/.agents/skills/analyst-ghidra/SKILL.md:34)

   `git ls-files` enumerates indexed paths, but the scanner reads working-tree contents. An untracked draft is omitted. A forbidden staged blob can be hidden by an unstaged clean replacement or deletion. The instruction to run the gate before saving a spec cannot inspect the proposed contents.

   CI runs after a push, when a public leak may already have occurred. It also inspects the checkout rather than every newly published commit, so adding and later deleting a leak within a pushed series escapes the final-tree check.

   **Fix:** Support explicit draft paths, scan index blobs before committing, and scan all outgoing commits before pushing. Retain CI as another check. Test staged/unstaged divergence, untracked drafts, and add-then-delete history.

9. **[High] Export destinations are unrestricted despite the promised confinement to `re/`.**
   [ExportStrings.java:15](C:/Users/przem/source/repos/opensherwood/scripts/ghidra/ExportStrings.java:15), [ExportInventory.java:18](C:/Users/przem/source/repos/opensherwood/scripts/ghidra/ExportInventory.java:18), [DecompileList.java:18](C:/Users/przem/source/repos/opensherwood/scripts/ghidra/DecompileList.java:18)

   All three accept arbitrary output directories. Passing `docs/` writes sensitive output directly into a publishable location. Their default `re/out` is relative to the process working directory. Existing files are overwritten. `strings.tsv` is a particularly straightforward leak that survives even the repaired heuristic.

   **Fix:** Require a configured analysis root, resolve real paths, account for symlinks/junctions, and refuse destinations outside that root. Keep analysis exports in an access-restricted workspace and label them as prohibited publication material.

   The documented `-process … -noanalysis -postScript …` syntax is valid. Supply absolute project/script/output paths, add `-readOnly` for export runs, and explicitly put `-log` and `-scriptlog` under the analysis root. `-noanalysis` only disables automatic analysis; it does not prevent project modifications. Ghidra documents default logs in the user directory, so they also need inclusion in the containment policy.

10. **[Medium; fixed subsequently] `ExportInventory.java` does not compile at `b13372e`.**
    [ExportInventory.java:31](C:/Users/przem/source/repos/opensherwood/scripts/ghidra/ExportInventory.java:31)

    At the requested commit, the file imports `ghidra.program.model.address.Address` but uses `AddressIterator` without importing it. Neither wildcard import from the listing or symbol packages supplies that type.

    **Fix:** Import `ghidra.program.model.address.AddressIterator`. Commit `4948af1` already resolves this through the address-package wildcard.

    The other checked calls exist in installed Ghidra 12.1.3. In particular, `ReferenceIterator` implements `Iterable<Reference>`, so the enhanced `for` loop in `ExportStrings` is valid. This was source/API verification, not a live compilation or headless execution test.

11. **[Medium] Failed or partial exports can appear successful, and reference results are incomplete.**
    [DecompileList.java:24](C:/Users/przem/source/repos/opensherwood/scripts/ghidra/DecompileList.java:24), [ExportStrings.java:21](C:/Users/przem/source/repos/opensherwood/scripts/ghidra/ExportStrings.java:21), [ExportInventory.java:27](C:/Users/przem/source/repos/opensherwood/scripts/ghidra/ExportInventory.java:27)

    `openProgram()` failure is ignored; `dispose()` is not protected by `finally`; decompilation failure produces a comment-only `.c` file followed by a success message. `PrintWriter` can suppress write errors. Cancellation produces partial inventory/string files with success messages.

    The string exporter follows references to a string’s exact starting address. References through pointer tables or into string interiors need additional handling. Inventory counts unique calling/called functions, not call sites. String `length` measures Java UTF-16 units, not encoded storage bytes, and whitespace replacement loses the original string value.

    **Fix:** Use throwing UTF-8 writers, checked directory creation, `finally` disposal, explicit cancellation/failure status, and atomic output publication. Emit a manifest containing build hash, tool version, requested/completed/failed entries, and completeness status. Name columns precisely; preserve strings using reversible escaping; identify unresolved indirect references. Missing results must not become evidence of absence.

12. **[High] The template lacks an enforceable evidence and implementation acceptance contract.**
    [SPEC-TEMPLATE.md:19](C:/Users/przem/source/repos/opensherwood/docs/original/SPEC-TEMPLATE.md:19), [SPEC-TEMPLATE.md:37](C:/Users/przem/source/repos/opensherwood/docs/original/SPEC-TEMPLATE.md:37)

    Scope, units, state transitions, native semantics, and open questions are useful foundations. But a document-level function list does not establish which claims are supported, and “reviewed” has no defined acceptance criteria.

    **Fix:** Add mandatory fields for:

    - **Claim evidence:** stable rule ID, observed/inferred/unknown status, supporting address or experiment, confidence, contradictions, and reproducible validation.
    - **Execution semantics:** integer widths, signedness, overflow, rounding, coordinate conventions, tick phase, simultaneous-event ordering, RNG consumption, lifecycle/reset behavior, and snapshot coverage.
    - **VM behavior:** argument coercion, stack effects, invalid IDs/handles, failure behavior, blocking/yielding, and callback order.
    - **Acceptance tests:** synthetic inputs with expected outputs/state changes; boundary and negative cases; local oracle procedures and tolerances.
    - **Implementation choices:** distinguish original behavior from OpenSherwood decisions and deliberate deviations. Map unresolved claims to ADR-0008 assumptions.
    - **Review identity:** actual build/edition/language, spec revision/hash, analyst and reviewer session IDs, unresolved blockers, and separate factual and publication approvals.

    **Function addresses themselves are generally factual coordinates, not program expression.** Their publication is not inherently a copying problem; this is an assessment, not a categorical legal exemption. Specify RVA versus virtual address versus file offset, image base, executable hash, and claim mapping. Keep instruction bytes, disassembly, recovered symbols, and comprehensive private address maps out of the public provenance. Addresses also remain subject to the disclosure assessment in finding 2.

The repository policy check and skill-mirror check passed, but the synthetic probes demonstrate why that does not validate the new safeguards. The two-tier approach is salvageable; its blanket analysis/publication rules and ineffective gate must be corrected before relying on it.

**Verdict: fix-then-merge.**