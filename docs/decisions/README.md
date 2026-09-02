# Architecture decision records

One file per decision, numbered, never rewritten (write a new ADR that supersedes an old one).
`reviews/` holds the full text of design reviews between the agents that led to these decisions.

| ADR | Decision |
|---|---|
| [0001](ADR-0001-rust.md) | Implementation language: Rust |
| [0002](ADR-0002-presentation.md) | CPU compositor is authoritative; presentation through winit + wgpu |
| [0003](ADR-0003-clean-room-roles.md) | Analyst / implementer separation for reverse engineering |
| [0004](ADR-0004-protocol.md) | JSON-RPC 2.0 over stdio, canonical input events, replay and hash schema from M0 |
| [0005](ADR-0005-name.md) | Project name: OpenSherwood |
| [0006](ADR-0006-scripting.md) | SCB VM first; one Lua 5.1 interpreter everywhere for mods |
| [0007](ADR-0007-roadmap.md) | Vertical-slice roadmap, tutorial mission first |
