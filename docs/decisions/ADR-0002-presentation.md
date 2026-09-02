# ADR-0002: CPU compositor is authoritative; winit + wgpu present it

Date: 2026-09-02. Status: accepted.

## Decision

The renderer is a deterministic CPU compositor producing an RGBA8 (or, if the oracle shows the original
composites in 16 bit, RGB565) framebuffer. Screenshots, hashes and golden comparisons are taken from this buffer,
never from GPU readback. A narrow `Presenter` trait uploads the buffer as a non-sRGB texture and draws it with
nearest-neighbour sampling; the first implementation uses `winit` + `wgpu`. No `softbuffer` fallback until wgpu is
shown to be insufficient somewhere.

Widescreen, shaders, upscaling and colour management are separate presentation modes added later; the reference
mode stays pixel-exact.

## Input rules

The engine preserves absolute pointer trajectories, button transition order, wheel events, physical key identity,
window-to-logical coordinate transforms and multiple motion events inside one simulation tick, because drag paths
and gestures may be gameplay input in this game.

## Why not SDL3

SDL3 is viable (Android, Emscripten, GPU API). `winit` + `wgpu` keeps the single-Cargo workflow, covers DX12 /
Vulkan / Metal / WebGPU and Android, and removes a C build dependency. Input fidelity is decided by our event model,
not by the windowing library.
