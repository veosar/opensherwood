# ADR-0007: Vertical-slice roadmap

Date: 2026-09-02. Status: accepted. The living roadmap with task lists is `docs/roadmap.md`.

Replace the horizontal "decode every format, then render, then move" plan with feasibility gates followed by a
tutorial-mission vertical slice (`EmbTut_FoC_EC`), then archetype coverage, then the full campaign.

Principles: decode only what the next slice needs; read-only parsers first (writers are an editor milestone);
exact pixel equality for directly decoded backgrounds, perceptual comparison only for composed scenes;
original-save compatibility is not on the campaign critical path; campaign automation acts through canonical
player input, using privileged observation only for planning.
