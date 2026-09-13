import { create } from "zustand";
import type { CreateGatewayInstanceRequested, BindGatewayCustomerRequested, GetGatewayInitialConfigRequested, PublishWistAgentdRequested, PublishWarpGateWayRequested, CreateUpgradePlanRequested, ApproveUpgradePlanRequested } from "../types";
import type { GatewayStatusView, GatewayListView, GatewayInstance, GatewayCustomerBinding, GatewayInitialConfig, WistAgentdRelease, WarpGateWayRelease, UpgradePlan, UpgradePlanApproval, GlobalPolicyDispatch } from "../types";

type ScreenState = "Loading" | "Ready" | "Error";

interface AppState {
  // Screen state
  currentScreen: string;
  screenState: ScreenState;
  lastEvent: Record<string, unknown> | null;

  gatewayStatusView: GatewayStatusView | null;
  gatewayListView: GatewayListView | null;
  gatewayInstance: GatewayInstance | null;
  gatewayCustomerBinding: GatewayCustomerBinding | null;
  gatewayInitialConfig: GatewayInitialConfig | null;
  wistAgentdRelease: WistAgentdRelease | null;
  warpGateWayRelease: WarpGateWayRelease | null;
  upgradePlan: UpgradePlan | null;
  upgradePlanApproval: UpgradePlanApproval | null;
  globalPolicyDispatch: GlobalPolicyDispatch | null;

  // Actions
  sendCreateGatewayInstanceRequested: (payload: CreateGatewayInstanceRequested) => void;
  sendBindGatewayCustomerRequested: (payload: BindGatewayCustomerRequested) => void;
  sendGetGatewayInitialConfigRequested: (payload: GetGatewayInitialConfigRequested) => void;
  sendPublishWistAgentdRequested: (payload: PublishWistAgentdRequested) => void;
  sendPublishWarpGateWayRequested: (payload: PublishWarpGateWayRequested) => void;
  sendCreateUpgradePlanRequested: (payload: CreateUpgradePlanRequested) => void;
  sendApproveUpgradePlanRequested: (payload: ApproveUpgradePlanRequested) => void;
  setScreen: (screen: string) => void;
}

export const useStore = create<AppState>((set) => ({
  currentScreen: "ProductListScreen",
  screenState: "Loading" as ScreenState,
  lastEvent: null,

  gatewayStatusView: null,
  gatewayListView: null,
  gatewayInstance: null,
  gatewayCustomerBinding: null,
  gatewayInitialConfig: null,
  wistAgentdRelease: null,
  warpGateWayRelease: null,
  upgradePlan: null,
  upgradePlanApproval: null,
  globalPolicyDispatch: null,

  sendCreateGatewayInstanceRequested: (payload) => {
    // TODO: handle CreateGatewayInstanceRequested event and update relevant view state
    set((state) => ({ ...state, lastEvent: payload as unknown as Record<string, unknown> }));
  },

  sendBindGatewayCustomerRequested: (payload) => {
    // TODO: handle BindGatewayCustomerRequested event and update relevant view state
    set((state) => ({ ...state, lastEvent: payload as unknown as Record<string, unknown> }));
  },

  sendGetGatewayInitialConfigRequested: (payload) => {
    // TODO: handle GetGatewayInitialConfigRequested event and update relevant view state
    set((state) => ({ ...state, lastEvent: payload as unknown as Record<string, unknown> }));
  },

  sendPublishWistAgentdRequested: (payload) => {
    // TODO: handle PublishWistAgentdRequested event and update relevant view state
    set((state) => ({ ...state, lastEvent: payload as unknown as Record<string, unknown> }));
  },

  sendPublishWarpGateWayRequested: (payload) => {
    // TODO: handle PublishWarpGateWayRequested event and update relevant view state
    set((state) => ({ ...state, lastEvent: payload as unknown as Record<string, unknown> }));
  },

  sendCreateUpgradePlanRequested: (payload) => {
    // TODO: handle CreateUpgradePlanRequested event and update relevant view state
    set((state) => ({ ...state, lastEvent: payload as unknown as Record<string, unknown> }));
  },

  sendApproveUpgradePlanRequested: (payload) => {
    // TODO: handle ApproveUpgradePlanRequested event and update relevant view state
    set((state) => ({ ...state, lastEvent: payload as unknown as Record<string, unknown> }));
  },

  setScreen: (screen) => {
    set({ currentScreen: screen });
  },
}));

