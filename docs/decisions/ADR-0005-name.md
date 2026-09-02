# ADR-0005: Project name "OpenSherwood"

Date: 2026-09-02. Status: accepted; revisit with a trademark search before the first public release.

## History

1. "Greenwood" (first proposal): generic, collides with an existing JavaScript framework. Rejected.
2. "Locksley" (used for the first hours): neutral folklore codename in the Julius / Exult style. Codex advised
   against thematic names; the maintainer asked for the `Open<Game>` convention of OpenMW / OpenJK / OpenRCT2 instead.
3. "Yewglass" (Codex's suggestion): distinctive but meaningless; rejected by the maintainer.
4. **"OpenSherwood"** (chosen, 2026-09-02).

## Reasoning

- It follows the naming convention the community recognises for engine reimplementations (OpenMW = Morrowind,
  OpenJK = Jedi Knight, OpenRCT2, OpenTTD, OpenXcom, OpenRA, OpenLoco). Those projects use an abbreviation or
  a part of the game's name nominatively, to say which game's data the engine reads; none reproduces a full
  registered title.
- "Sherwood" is a geographic name (Sherwood Forest) used by many unrelated games and companies, which makes
  exclusive rights in the word alone weak. The registered title is the composite
  "Robin Hood – The Legend of Sherwood", which we do not reproduce.
- No GitHub, crates.io or domain collisions were found for "OpenSherwood".

## Risk and mitigation

Using a word from the title carries more risk than a neutral codename. Mitigations: an explicit non-affiliation
disclaimer in the README; the game title used only descriptively; no logos or artwork of the game; a formal
trademark search (EUIPO, WIPO, USPTO) on the release checklist; and readiness to rename (repository, crate prefix
`opensherwood-`, environment variable `OPENSHERWOOD_GAME_DIR`) if the rights holder objects. Renaming is a mechanical
search-and-replace, as the two earlier renames showed.
