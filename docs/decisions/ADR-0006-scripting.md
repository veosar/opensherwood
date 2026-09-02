# ADR-0006: Scripting

Date: 2026-09-02. Status: accepted.

1. The retail campaign runs on the original `.scb` bytecode through our own VM (`opensherwood-script`). This is the
   only way to play the original missions unchanged.
2. Modding uses Lua. One Lua 5.1-compatible interpreter (vendored PUC Lua through `mlua`) on every platform, so
   mods behave identically everywhere. A JIT may be offered later as an explicitly non-authoritative option.
3. The Lua API follows the naming of the community Spellforge `api.lua` where practical, so existing community
   missions are portable, but Lua is added only after the native gameplay API is stable (milestone M5+).
4. Script-visible state is part of the canonical state hash.
