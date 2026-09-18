# Codex spec re-review 25: docs/original/spec-script-vm.md (revision 4, commit afdfaec), 2026-09-13/18

Spec reviewer role (ADR-0009), with access to `re/`. Answered in revision 5. Batch 1 of the implementation continues on the cleared subset.

Reviewed [revision 4](C:/Users/przem/source/repos/opensherwood/docs/original/spec-script-vm.md), commit `afdfaec666b3a1d8eee8f4d927ead008628b1516`, blob `04ae60aa5d4dd88c5e3b0687165d9ed3dc3d5a79`, against the decompilation and VM notes. All 12 review-21 findings were checked. The listed commit/blob pairs match. Only remaining defects follow; addresses are virtual, image base `0x400000`.

1. **High — VM-014, VM-088; §8.3; acceptance case 17: parameter-buffer semantics still contradict execution.**
   **Addresses:** `006392e0`, `0063a250`, `0063a320`, `0063a400`, `00634eb0`.

   VM-088’s “restored when it pops” is false: the captured parameter buffer is discarded; the current buffer remains the callee’s, as VM-013 correctly states. Native-buffer symmetry likewise requires balanced callback behavior; it is not automatic restoration.

   Case 17’s residue becomes a parameter of `ProcessMessage` upon callback entry. It does not subsequently shift the parameters of a script function called from that callback. Section 8.3’s parenthetical enumeration still omits the current script-parameter buffer.

   **Correction:** preserve VM-013’s semantics throughout; enumerate both buffers explicitly; have the resumed callback read the residue and shifted callback arguments before testing a separate script call.

2. **High — VM-201; §8.1: recording guards remain overgeneralized, and the wrap fault is off by one.**
   **Addresses:** `00570f00`, `00571100`, `0057a020`, `005740b0`, `00573450`, `005743a0`, `00574a70`.

   “Every other recording native still append[s]” remains false. Natives 45, 59, 63 and 212 reject level zero before reaching an append. Native 30 tests recording existence, despite the sentence grouping it with level-counter tests.

   Starting at level 1, wrap occurs on the **65,535th increment**, not §8.1’s “65536th increment.”

   **Correction:** distinguish each native’s preliminary guard from the common append guard; retain timers/messages as verified examples of level-zero appends; fault on the attempted transition from 65,535 to zero.

3. **High — VM-203; native rows 46/47; acceptance case 5: recording and entry effects remain inaccurate.**
   **Addresses:** `00577650`, `005779a0`, `004fc8f0`, `0055f290`, `00573d10`, `00573da0`.

   VM-203 incorrectly includes 46 and 57 among compound walk recorders adding door passages, waits and turns. Native 46 records one walk; 57 records a seek. Native 46’s “facing set, movement stopped” is not established by its placement path: the supplied direction determines entry geometry, and the coordinate-refresh helper does not establish those additional effects.

   Native 47 uses the current position as an initial walk origin when unlisted, but stores the supplied location as the entered-list entry; the row conflates those values. Case 5 still assumes a “known `p'`” from an unspecified entry rule.

   **Correction:** describe these effects separately; supply a grounded entry fixture or explicitly leave that assertion deferred.

4. **High — VM-219; native rows 18–20; acceptance case 19: camera conversion and interference remain incomplete.**
   **Addresses:** `005758d0`, `00571270`, `00571330`, `005713f0`, `004cdfc0`.

   Native 20 can change zoom through the same conversion fallback as 18/19, contradicting “touches neither … the zoom.” An undersized map alone does not select that fallback: the pre-clamp derived coordinate also matters.

   “Nothing waits for the scroll” overlooks an existing camera element that 18/19 preserve and arrival can complete. Native 19’s value **1.0** affects subsequent ramp-index behavior, so its influence is not confined to the first displacement.

   Case 19’s exact 2/3-pixel movement needs axis-aligned, sufficiently distant destinations because components are truncated. Its next-drawn-frame assertion after native 20 also needs protection from intervening scroll/follow updates.

   **Correction:** establish the exact shared conversion, including screen dimensions, vertical bounds and fallback conditions; distinguish direct writes from later effects; add uncontested and active-camera fixtures.

5. **High — VM-089; §8.1: deterministic fault recovery remains underspecified.**
   **Addresses:** `00639300`, `00639370`, `0063a320`, `00635210`, `00579730`.

   “Buffers are cleared as at a callback end” contradicts normal buffer persistence. Missing-callback handling does not say what happens to arguments already appended before lookup. “As if the frame had returned with 0x06” does not define termination through multiple script frames or a nested engine callback.

   Native 223 is also incorrectly placed among unchecked **writes**; it reads banner state.

   **Correction:** specify buffer disposition, both result registers, unwind boundary, enclosing-callback continuation and dynamic-context restoration for each terminating departure. Define whether an open recording survives. Correct native 223’s classification or explicitly identify its exceptional policy.

6. **High — §0, §§4.1–4.3; native rows 100, 106/107: the unresolved-effect register is still incomplete.**
   **Addresses:** `00571550`, `00578820`, `00578830`.

   Adding native 165 fixes that omission, but “the third style” and “a HUD flag byte” still lack identified meanings/consumers or exact dependency claims. Those natives nevertheless remain inside the purported 240 settled boundary contracts.

   **Correction:** settle these contracts or add them to the exclusions/dependency register and recalculate the count. Replace blanket “Implementable as verbatim pass-through” with the exact cleared request boundary for each operation; unnamed consumers do not provide executable semantics.

7. **High — VM-231; native 51: the replacement final-frame description loses required ordering.**
   **Addresses:** `00464b20`, `0058a940`, `00582560`, `00585b70`, `0058ba60`.

   “The actor is then observed holding the clip’s last frame” remains unconditional. The hold request enters deferred dispatch before the original animation reports completion; the original sequence’s next level can execute before hold admission. Admission, replacement or refusal can prevent the asserted hold.

   **Correction:** state the observable request/completion/admission ordering and condition the hold on acceptance without interference. Keep its implementation container unspecified.

8. **High — VM-231; §1.1: the general walk-arrival rule remains wrong.**
   **Addresses:** `00561040`, `00560460`, `00467a50`.

   “Walks on arrival within radius + 5 px” conflates ordinary movement completion with a separate approach test. The pinned navigation revision has **NAV-150**, not NAV-152; its approach condition is strict Chebyshev distance below tolerance plus 5. Ordinary arrival uses a directional projection.

   **Correction:** separate those completion conditions and cite their precise claims. Keep unread geometry and admission cases gated.

9. **Medium — §1.1, §9.4, provenance: valid hashes pin inaccurately described dependencies.**
   **Addresses:** documentary; behavior anchors `005758d0`, `00575e20`, `004d23d0`.

   The pinned navigation revision puts empty-result handling in **NAV-146**, explicitly saying “100 ticks”; NAV-142 concerns search-result selection. The pinned AI native summary is **§6.1**, and native 126 already agrees with the VM’s menacing/fleeing order. Review 22 upheld that reading.

   “None reviewed yet” is stale: the pinned revisions received reviews 20, 22 and 24. Their verdicts and partial clearances must be carried forward. The movement pin does not establish the claimed complete camera agreement. Campaign dependencies introduced for 165 and 195/196 remain unpinned.

   **Correction:** update claim references, descriptions, review dispositions and campaign pins; distinguish resolved facts from remaining gates.

10. **High — expression filter; VM-120, VM-210, native 102: recovered containers remain prescribed.**
    **Addresses:** `00578d80`, `00578f20`, `00577500`, `0058a940`.

    Remaining offending passages include:

    > “109 / 110 wrap it in a one-element sequence”

    > “launch one-element sequences the same way”

    > “through a one-element sequence executed on the next drain”

    **Correction:** specify synchronous message delivery, independent effect lifetime and damage-dispatch ordering without requiring the recovered sequence container. The earlier campaign-slot expressions are removed. The automated asset check passes; it does not clear these passages.

11. **Medium — identity block: review artifacts still lack an explicit session mapping.**
    **Address:** documentary.

    The committed review references and reviewed blobs are now retrievable, but declaring each filename a session reference does not supply the reviewer-session identity required by the template. Exposure is attributed specifically to review 21; reviews 14/17 lack individual exposure attribution.

    **Correction:** record actual session identities or a retrievable maintainer-held mapping, associate exposure with each, and append this review’s reviewed blob and exposure. This reviewer inspected VM/frame helpers, natives/wrappers, recording/dispatch, camera, actor/animation and campaign helpers; this context must not implement those subsystems.

**Implementation clearance now:** batch 1 may continue with revision 3’s cleared instruction, calling and native-result protocols; message payload, target, synchronous-delivery and dynamic-context rules; and individually settled, self-contained native effects. Revision 4 additionally settles the **default zero for an unwritten result slot**. The requirement to preserve both pending buffers is established; VM-088’s restoration wording is not cleared.

**Still waiting:** the complete snapshot/fault contract and acceptance trace; full recording/sequences and scheduler integration; camera conversion/interference, navigation, actor admission/priority, native-51 hold, speech/action completion, AI-event meanings and dependent campaign effects. Sections 4.2/4.3 and the additional gaps above remain outside faithful implementation clearance.

**Verdict: fix-then-clear.**