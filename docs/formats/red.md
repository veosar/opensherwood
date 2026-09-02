# `.red` per-level text index (`Text/RHLevel??.red`)

Status: **stub**.

57 files of 64 bytes (`RHLevelAA.red` .. `RHLevelVO.red`). Content is 16 u32 values, most of them ids in the
`Level.res` TEXT/PIC/WAVE id space (`0x0F4240` = 1000000 ..) and small counts. Likely layout: title id, goal id,
briefing id, briefing picture id, debriefing ids, dialogue list id + count, popup text id + count, short briefing ids.
The console command `LEVEL TEXT DG|DB|PT|SB` (dialogues, debriefings, popup texts, short briefings) lists those
categories.

## Provenance

Observation.
