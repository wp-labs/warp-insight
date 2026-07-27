# AI Implementation Tasks

## Context

This project was generated from a MoJu design and is intended as a working UI implementation skeleton.

- Domain: `Global`
- Target: `Global<bin,http>`
- Profile: `warp-insight-admin-web`
- Language: typescript
- Framework: react

## Read First

1. Read `moju-ui-model.json` — it describes the complete UI model (types, regions, flows, interfaces).
2. Read the generated source under `src/` — all components, hooks, and API layers are wired from the model.
3. The source MoJu model is the source of truth; generated code reflects one projection of it.

## Goal

Complete the generated UI implementation while preserving the generated module layout and MoJu metadata.

## Component Tasks

- Implement `Subsystem.AbnormalAgentCard` component — contains 5 child region(s).
- Implement `Subsystem.AbnormalAgentCardGrid` component — leaf component. (repeat region)
- Implement `Subsystem.AbnormalAgentPanel` component — contains 2 child region(s).
- Implement `Subsystem.AdminHomePage` component — contains 3 child region(s).
- Implement `Subsystem.AdminOperatorAccess` component — leaf component.
- Implement `Subsystem.AdminOperatorIdentity` component — leaf component.
- Implement `Subsystem.AdminOperatorLane` component — contains 2 child region(s).
- Implement `Subsystem.AdminTopNavigation` component — leaf component.
- Implement `Subsystem.AgentControlCenterPage` component — contains 3 child region(s).
- Implement `Subsystem.AgentControlUsecaseBoard` component — contains 2 child region(s).
- Implement `Subsystem.AgentHealthBadge` component — leaf component.
- Implement `Subsystem.AgentHealthStatusFilter` component — leaf component.
- Implement `Subsystem.AgentInstallCodeList` component — contains 2 child region(s).
- Implement `Subsystem.AgentInstallPage` component — contains 2 child region(s).
- Implement `Subsystem.AgentInstanceText` component — leaf component.
- Implement `Subsystem.AgentLastSeenAtText` component — leaf component.
- Implement `Subsystem.AgentOnlineStatusBadge` component — leaf component.
- Implement `Subsystem.AgentOnlineStatusFilter` component — leaf component.
- Implement `Subsystem.AgentRuntimeStatusResult` component — contains 5 child region(s).
- Implement `Subsystem.AgentStatusOverviewMetrics` component — contains 4 child region(s).
- Implement `Subsystem.AgentStatusSearchInput` component — leaf component.
- Implement `Subsystem.AgentVersionFilter` component — leaf component.
- Implement `Subsystem.AgentVersionText` component — leaf component.
- Implement `Subsystem.ArmLinuxAgentInstallCode` component — leaf component.
- Implement `Subsystem.LastSeenLagMetric` component — leaf component.
- Implement `Subsystem.NoAbnormalAgentPlaceholder` component — leaf component.
- Implement `Subsystem.OnlineAgentMetric` component — leaf component.
- Implement `Subsystem.PauseAgentForm` component — handles 1 interaction(s), contains 2 child region(s).
- Implement `Subsystem.PauseAgentInput` component — leaf component.
- Implement `Subsystem.PauseAgentUsecaseCard` component — contains 3 child region(s).
- Implement `Subsystem.PauseAgentUsecaseMeta` component — leaf component.
- Implement `Subsystem.PauseDispatchCreatedAtText` component — leaf component.
- Implement `Subsystem.PauseDispatchIdText` component — leaf component.
- Implement `Subsystem.PauseDispatchReceiptResult` component — contains 2 child region(s).
- Implement `Subsystem.RefreshRuntimeStatusAction` component — leaf component.
- Implement `Subsystem.RuntimeStatusAgentInput` component — leaf component.
- Implement `Subsystem.ShowAgentRuntimeStatusForm` component — handles 1 interaction(s), contains 2 child region(s).
- Implement `Subsystem.ShowAgentRuntimeStatusUsecaseCard` component — contains 3 child region(s).
- Implement `Subsystem.ShowAgentRuntimeStatusUsecaseMeta` component — leaf component.
- Implement `Subsystem.SubmitPauseAction` component — leaf component.
- Implement `Subsystem.SubmitUpgradeAction` component — leaf component.
- Implement `Subsystem.TargetVersionInput` component — leaf component.
- Implement `Subsystem.TotalAgentMetric` component — leaf component.
- Implement `Subsystem.UnhealthyAgentMetric` component — leaf component.
- Implement `Subsystem.UpgradeAgentInput` component — leaf component.
- Implement `Subsystem.UpgradeAgentRemotelyForm` component — handles 1 interaction(s), contains 3 child region(s).
- Implement `Subsystem.UpgradeAgentRemotelyUsecaseCard` component — contains 3 child region(s).
- Implement `Subsystem.UpgradeAgentRemotelyUsecaseMeta` component — leaf component.
- Implement `Subsystem.UpgradeDispatchCreatedAtText` component — leaf component.
- Implement `Subsystem.UpgradeDispatchIdText` component — leaf component.
- Implement `Subsystem.UpgradeDispatchReceiptResult` component — contains 2 child region(s).
- Implement `Subsystem.X86LinuxAgentInstallCode` component — leaf component.

## Flow Tasks

- No flow hooks were generated.

## API Tasks

- No API hooks were generated.

## Store Tasks

- Review `src/store/index.ts` — the Zustand store skeleton holds event handlers and view data.
- Wire store actions to the API layer and component event callbacks.

## Do Not

- Do not edit MoJu source files unless explicitly requested.
- Do not remove `// @moju generated` headers from generated files.
- Do not replace the generated module layout without updating this task file.

## Acceptance Criteria

- `npm run build` (or `tsc --noEmit && vite build`) passes with zero errors.
- All generated components render without runtime errors in the browser.
- Flows and API hooks are implemented or explicitly left with reviewed TODOs.
