# ADR-0001: Implementation language is Rust

Date: 2026-09-02. Status: accepted (Claude proposed, Codex reviewed and agreed).

## Context

The human's initial research suggested C++20 + SDL3, following the ecosystem of Julius / OpenRCT2 / DevilutionX.
The project is developed mostly by AI agents that must build, test and iterate autonomously on Windows first and
Linux / macOS / Android later.

## Decision

Rust, stable toolchain pinned in `rust-toolchain.toml`, `Cargo.lock` committed, edition 2024.

## Reasons

- Early work is dominated by parsers for partially understood binary formats. Bounds-checked slices, explicit
  integer conversions, structured errors and fuzz targets shorten the loop and avoid silent memory corruption.
- One tool (`cargo`) for dependencies, build, test, bench, fuzz and cross-compilation. No CMake / vcpkg drift for
  agents to repair.
- No need for ABI compatibility with the original MSVC 6 executable; the oracle is a separate, Windows-only tool.
- Deterministic simulation benefits from restricted mutability and conspicuous non-deterministic containers.

## Rules

- No `unsafe` in formats, core, script, protocol or render crates. FFI / platform code only in the app crate.
- No general-purpose game engine or ECS framework. Stable generational entity ids, typed arenas, explicit systems,
  deterministic iteration order.
- Explicit bounded binary readers; offsets must stay visible in parser code.
- `cargo nextest`, fuzz targets and `sccache` are welcome early; Android and web are later validation targets,
  never M0 gates.

## Consequences

Contributors from the C++ reimplementation scene need to learn Rust; we accept that. C/C++ dependencies
(Lua, compression) are consumed through vendored crates with a C compiler present on all CI targets.
