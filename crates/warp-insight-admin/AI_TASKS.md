# AI Implementation Tasks

## Context

This crate was generated from a MoJu design and is intended as a compileable implementation skeleton.

- Domain: `Global`
- Target: `Global<bin,http>`
- Profile: `HttpRust`
- Crate: `warp-insight-admin`
- Scaffold: `api, route, dto, handler`
- Source MoJu model: `/Users/zuowenjian/devspace/rust/x-topology/warp-insight/moju`
- MoJu skills: `/Users/zuowenjian/devspace/rust/x-topology/moju-sys/moju-skills`
- MoJu model summary: `MOJU_MODEL.md`

## Read First

1. Read the relevant long-lived MoJu skills from the `moju-skills` directory, especially `moju-model-understanding.md`.
2. Read the source MoJu model directory listed above. It is the source of truth.
3. Read `MOJU_MODEL.md` as a navigation summary only.

## Goal

Complete the generated service implementation while preserving the generated module layout and MoJu metadata.

## Interface Tasks

- No interface bindings were generated for this target.

## Flow Tasks

- No flow skeletons were generated for this target.

## Storage Tasks

- No storage bindings were generated for this target.

## Config Tasks

- No runtime config contract was generated for this target.

## Capability Tasks

- No capability clients were generated for this target.

## Do Not

- Do not edit MoJu source files unless explicitly requested.
- Do not remove `.moju-gen.json` or MoJu metadata comments/attributes.
- Do not replace the generated module layout without updating this task file.

## Acceptance Criteria

- `cargo check` passes in this crate.
- Generated route, flow, storage, and capability skeletons are implemented or explicitly left with reviewed TODOs.
- Response mappings preserve the statuses declared in `binding.mju`.
