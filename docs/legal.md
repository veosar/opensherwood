# Legal position and rules

This document is the project's legal policy. It is not legal advice. Every contributor and every
AI agent working in this repository must follow the rules in the "Hard rules" section without exception.

## Summary

OpenSherwood is a clean-room, open-source reimplementation of the game engine behind
*Robin Hood: The Legend of Sherwood* (Spellbound Entertainment, 2002). It is a new program that
reads the data files of a copy of the game the player already owns. It does not contain, and must
never contain, any part of the original game: no executable code, no graphics, no sound, no text,
no maps, no scripts.

The rights to the game are active. The publisher is Microids (a Média-Participations company).
The game is sold on Steam (app 46560) and GOG. The engine and format knowledge documented here was
obtained by analysing the files of a legally purchased GOG copy for the purpose of interoperability.

## Hard rules

1. **No game assets in the repository, ever.** Not in commits, not in tests, not in CI artifacts,
   not in screenshots committed to `docs/`. The `.gitignore` blocks the known file extensions but
   the rule covers everything: bytes, pixels, text strings, audio, video, thumbnails, save games.
   Reference screenshots for visual tests are generated locally from the player's own copy and are
   never pushed. Only hashes, metrics and synthetic fixtures may be committed.
2. **Two-tier wall (ADR-0009).** The original executable is decompiled for interoperability by analysts in the
   private `re/` directory (git-ignored). Output of decompilers or disassemblers never enters the repository,
   nor do line-by-line paraphrases, the binary's identifiers or strings, value tables copied from it, or game
   text. What may be committed is *knowledge*: file format specifications, behaviour descriptions, constants,
   algorithms described in our own words, and tests that check our implementation against observable behaviour
   of the original. Implementers work only from those specifications and never read the decompilation of the
   subsystem they implement. Every spec records how the knowledge was obtained (see "Provenance").
3. **No trademarks in the project name.** The project is called OpenSherwood. The game title is used
   only descriptively ("an engine for the data files of Robin Hood: The Legend of Sherwood").
4. **GPLv3.** All code and documentation in this repository is licensed under the GNU General Public
   License version 3 (see `LICENSE`). Third-party dependencies must be GPLv3-compatible.
5. **The player supplies the data.** The engine locates an existing installation (GOG, Steam or a
   user-configured directory) and reads from it. It never downloads, copies or redistributes game data.
6. **Community tools are treated as closed source** unless their license says otherwise. Do not copy
   code from them. Reading their public documentation or using them as an oracle is fine.

## Provenance of format knowledge

Every specification in `docs/formats/` has a "Provenance" section that states which of the following
methods produced each part of the spec:

- **Observation**: hexdumps, statistics and experiments on data files (fully clean).
- **Behavioural testing**: running the original game with modified inputs or data and observing the result.
- **Decompilation** (the primary method since ADR-0009): reading the original executable in Ghidra to
  understand a behaviour completely. The specification is written in prose and mathematics in the analyst's
  own words, with function addresses as provenance; no decompiler output or transcribed pseudocode is
  committed. Whoever analyses a subsystem does not write its engine code; a separate session implements from
  the reviewed spec.
- **Community knowledge**: public documentation from the modding community (cite the URL).

## Necessity and disclosure (ADR-0009)

Decompilation is confined to what interoperability with the player's files requires, and each specification
records that necessity (interoperability target, information not otherwise available, scope read, stopping
condition). Publishing a specification is a separate decision from writing it: the maintainer approves each
specification for release, considering that the interoperability exception restricts onward disclosure and
the development of substantially similar expression (Directive 2009/24/EC art. 6(2), the Polish act art. 75
ust. 3); the expression filter of ADR-0009 is applied at review. The acquisition record of the analysed copy
and the store and publisher terms in force when it was acquired are kept privately by the maintainer (the
GOG user agreement restricts reverse engineering "except as permitted by applicable law"; the statutory
exceptions cannot be excluded by contract: Directive art. 8, the Polish act art. 76). Analysts act on behalf of
the maintainer, on the maintainer's lawfully acquired copy. This policy is not legal advice; the workflow is to
be assessed by counsel before a public release that includes decompilation-derived specifications.

## Legal basis

- European Union: Directive 2009/24/EC, Article 5(3) (observing, studying and testing a program one
  is entitled to use) and Article 6 (decompilation for interoperability). CJEU C-13/20 *Top System*
  (2021) confirmed decompilation for error correction under Article 5(1).
- Poland: Ustawa o prawie autorskim i prawach pokrewnych, art. 75 ust. 2-3 (analysis and decompilation
  for interoperability).
- United States: *Sega v. Accolade* (9th Cir. 1992), *Sony v. Connectix* (9th Cir. 2000);
  DMCA §1201(f) (reverse engineering for interoperability).

## If we receive a takedown or cease-and-desist

1. Do not delete history in panic. Take the specific item offline if it is a real asset leak.
2. Confirm in writing that the repository contains no game assets and no original code, and that the
   project only interoperates with data the user already owns.
3. Rename the project if the complaint is about the name.
4. Continue on the basis above.
