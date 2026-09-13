# warp-insight

This repository currently contains the design docs and a minimal Rust workspace skeleton
for the first implementation wave.

Workspace layout (`warp-insight` workspace):

- `crates/warp-gateway`
  Admin WEB backend skeleton for install links, Agent status, and remote upgrades.
- `crates/gateway-web`
  Browser WEB frontend for the WarpGateWay console.
- `crates/wist-upgrader`
  Upgrade helper skeleton.
- `crates/wist-gateway`
  Southbound gateway/server skeleton.
- `crates/warp-insight-control`
  Control-center core skeleton.

Independent top-level crates (moved out of this workspace for independent release):

- `wist-contracts` — shared contract types and versioned schema objects.
- `wist-validate` — static validators for plans, results, config, and state.
- `wist-shared` — shared errors, IDs, paths, and common runtime helpers.
- `wist-metrics` — metrics runtime model.
- `wist-agentd` — edge daemon (binary).

The current code is intentionally minimal and is meant to anchor the module boundaries
defined under `doc/design`.
