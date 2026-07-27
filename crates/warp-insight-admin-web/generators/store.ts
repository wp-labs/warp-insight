import { camelCase } from "change-case";
import type { TsModel } from "./types";

type WriteFile = (dir: string, filename: string, content: string) => void;
type EnsureDir = (...segments: string[]) => string;

function isListView(viewName: string, model: TsModel): boolean {
  // A view is a list view if any region repeats over it or its corresponding region
  for (const region of model.regions) {
    if (!region.repeat) continue;
    const repeatRegion = region.repeat.region;
    // Check if the repeat region's name maps to this view (e.g., ProductCardView -> ProductCard)
    const viewCamel = camelCase(viewName);
    const regionCamel = camelCase(repeatRegion.replace(/View$/, "").replace(/Row$/, ""));
    if (regionCamel === viewCamel || repeatRegion === viewName) return true;
  }
  return false;
}

function screenNames(model: TsModel): Set<string> {
  const screens = new Set<string>();
  for (const region of model.regions) {
    if (region.name.endsWith("Screen")) {
      screens.add(region.name);
    }
  }
  return screens;
}

// ── Store action helpers ──

function inferAction(eventName: string): "add" | "remove" | "update" | null {
  if (eventName.startsWith("Add") || eventName.endsWith("Clicked")) return "add";
  if (eventName.endsWith("Removed")) return "remove";
  if (eventName.endsWith("Updated") || eventName.endsWith("Selected") || eventName.endsWith("Changed")) return "update";
  return null;
}

function viewForEvent(eventName: string, model: TsModel): { viewName: string; camelName: string; isList: boolean; action: "add" | "remove" | "update" | null } | null {
  // Match event name substring against view names
  for (const view of model.types.views) {
    // e.g., AddToCartClicked → CartLineItem? No, better match: event contains view name
    if (eventName.toLowerCase().includes(view.name.toLowerCase())) {
      const camelName = camelCase(view.name);
      return { viewName: view.name, camelName, isList: isListView(view.name, model), action: inferAction(eventName) };
    }
  }
  // Try matching event field names against view field names
  const eventDef = model.types.events.find((e) => e.name === eventName);
  if (!eventDef) return null;
  for (const view of model.types.views) {
    const viewFieldNames = new Set(view.fields.map((f) => f.name));
    for (const ef of eventDef.fields) {
      if (viewFieldNames.has(ef.name)) {
        const camelName = camelCase(view.name);
        return { viewName: view.name, camelName, isList: isListView(view.name, model), action: inferAction(eventName) };
      }
    }
  }
  return null;
}

export function generateStore(
  model: TsModel,
  writeFile: WriteFile,
  ensureDir: EnsureDir,
): void {
  const dir = ensureDir("src", "store");

  // Collect all event types that flow through the app
  const allEvents = new Set<string>();
  for (const region of model.regions) {
    for (const ix of region.interactions) {
      allEvents.add(ix.emit_event);
    }
  }
  for (const flow of model.flows) {
    for (const step of flow.steps) {
      for (const h of step.handlers) {
        allEvents.add(h.event);
      }
    }
  }

  // Collect all view types
  const viewTypes = model.types.views.map((v) => v.name);
  const screens = screenNames(model);

  // Only import ScreenState (used in the state type), not all enums
  const hasScreenState = model.types.states.some((s) => s.name === "ScreenState");
  let code = `import { create } from "zustand";\n`;
  code += `import type { ${[...allEvents].join(", ")} } from "../types";\n`;
  if (viewTypes.length > 0) {
    code += `import type { ${viewTypes.join(", ")} } from "../types";\n`;
  }
  if (hasScreenState) {
    code += `import type { ScreenState } from "../types";\n`;
  }
  code += "\n";
  if (!hasScreenState) {
    code += `type ScreenState = "Loading" | "Ready" | "Error";\n\n`;
  }

  // Store state interface
  code += `interface AppState {\n`;
  code += `  // Screen state\n`;
  code += `  currentScreen: string;\n`;
  code += `  screenState: ScreenState;\n`;
  code += `  lastEvent: Record<string, unknown> | null;\n\n`;

  // Data stores for views — arrays for list views, singletons otherwise
  for (const view of model.types.views) {
    const camelName = camelCase(view.name);
    if (isListView(view.name, model)) {
      code += `  ${camelName}: ${view.name}[];\n`;
    } else {
      code += `  ${camelName}: ${view.name} | null;\n`;
    }
  }
  code += "\n";

  // Actions (event senders)
  code += `  // Actions\n`;
  for (const evt of allEvents) {
    const camelEvt = camelCase(evt);
    code += `  send${evt}: (payload: ${evt}) => void;\n`;
  }
  code += `  setScreen: (screen: string) => void;\n`;
  code += `}\n\n`;

  // Store implementation
  const defaultScreen = [...screens][0] ?? "ProductListScreen";
  code += `export const useStore = create<AppState>((set) => ({\n`;
  code += `  currentScreen: "${defaultScreen}",\n`;
  code += `  screenState: "Loading" as ScreenState,\n`;
  code += `  lastEvent: null,\n\n`;

  // Default data
  for (const view of model.types.views) {
    const camelName = camelCase(view.name);
    if (isListView(view.name, model)) {
      code += `  ${camelName}: [],\n`;
    } else {
      code += `  ${camelName}: null,\n`;
    }
  }
  code += "\n";

  // Event handlers
  for (const evt of allEvents) {
    const camelEvt = camelCase(evt);
    if (evt === "ScreenChanged") {
      code += `  sendScreenChanged: (payload) => {\n`;
      code += `    set({ currentScreen: payload.to });\n`;
      code += `  },\n\n`;
    } else if (evt === "ScreenDataLoaded") {
      code += `  sendScreenDataLoaded: (payload) => {\n`;
      code += `    set({ currentScreen: payload.screen, screenState: "Ready" });\n`;
      code += `  },\n\n`;
    } else {
      const matched = viewForEvent(evt, model);
      if (matched && matched.action) {
        const { camelName, viewName, isList, action } = matched;
        code += `  send${evt}: (payload) => {\n`;
        if (isList && action === "add") {
          code += `    set((state) => ({\n`;
          code += `      ${camelName}: [...state.${camelName}, payload as unknown as ${viewName}],\n`;
          code += `      lastEvent: payload as unknown as Record<string, unknown>,\n`;
          code += `    }));\n`;
        } else if (isList && action === "remove") {
          code += `    set((state) => ({\n`;
          code += `      ${camelName}: state.${camelName}.filter((item) => (item as unknown as Record<string, unknown>).id !== (payload as unknown as Record<string, unknown>).id),\n`;
          code += `      lastEvent: payload as unknown as Record<string, unknown>,\n`;
          code += `    }));\n`;
        } else if (!isList && (action === "update" || action === "add")) {
          code += `    set((state) => ({\n`;
          code += `      ${camelName}: { ...state.${camelName}, ...payload as unknown as Record<string, unknown> } as ${viewName},\n`;
          code += `      lastEvent: payload as unknown as Record<string, unknown>,\n`;
          code += `    }));\n`;
        } else {
          code += `    set((state) => ({ ...state, lastEvent: payload as unknown as Record<string, unknown> }));\n`;
        }
        code += `  },\n\n`;
      } else {
        code += `  send${evt}: (payload) => {\n`;
        code += `    // TODO: handle ${evt} event and update relevant view state\n`;
        code += `    set((state) => ({ ...state, lastEvent: payload as unknown as Record<string, unknown> }));\n`;
        code += `  },\n\n`;
      }
    }
  }

  // setScreen
  code += `  setScreen: (screen) => {\n`;
  code += `    set({ currentScreen: screen });\n`;
  code += `  },\n`;
  code += `}));\n`;

  writeFile(dir, "index.ts", code + "\n");
}
