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
2. **Clean room.** Output of decompilers or disassemblers never enters the repository. Analysis of the
   original executable is done in the private `re/` directory (git-ignored). What may be committed is
   *knowledge*: file format specifications, behaviour descriptions, constants, algorithms described
   in our own words, and tests that check our implementation against observable behaviour of the
   original. Every spec file in `docs/formats/` records how the knowledge was obtained (see
   "Provenance").
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
- **Static analysis notes**: reading the original executable in a disassembler to understand a
  behaviour. The notes are rewritten in prose and pseudocode in the analyst's own words; no
  decompiler output is committed. Contributors who do this must not also write the corresponding
  engine code from that output in the same sitting; write the spec first, then implement from the spec.
- **Community knowledge**: public documentation from the modding community (cite the URL).

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
