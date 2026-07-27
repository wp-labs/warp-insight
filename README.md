# warp-insight

This repository currently contains the design docs and a minimal Rust workspace skeleton
for the first implementation wave.

Workspace layout:

- `crates/warp-insight-contracts`
  Shared contract types and versioned schema objects.
- `crates/warp-insight-validate`
  Static validators for plans, results, config, and state.
- `crates/warp-insight-shared`
  Shared errors, IDs, paths, and common runtime helpers.
- `crates/warp-insight-admin`
  Admin WEB backend skeleton for install links, Agent status, and remote upgrades.
- `crates/warp-insight-admin-web`
  Browser WEB frontend for the WarpInsightAdmin console.
- `crates/warp-insightd`
  Edge daemon skeleton.
- `crates/warp-insight-exec`
  ActionPlan runtime skeleton.
- `crates/warp-insight-upgrader`
  Upgrade helper skeleton.
- `crates/warp-insight-gateway`
  Southbound gateway/server skeleton.
- `crates/warp-insight-control`
  Control-center core skeleton.

The current code is intentionally minimal and is meant to anchor the module boundaries
defined under `doc/design`.
