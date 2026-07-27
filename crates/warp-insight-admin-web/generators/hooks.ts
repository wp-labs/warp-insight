import { pascalCase } from "change-case";
import type { TsModel, TsFlow, TsStep } from "./types";

type WriteFile = (dir: string, filename: string, content: string) => void;
type EnsureDir = (...segments: string[]) => string;

function flowHookName(flow: TsFlow): string {
  return `use${pascalCase(flow.name)}`;
}

function stepStateName(step: TsStep): string {
  return step.name;
}

function generateFlowHook(flow: TsFlow): string {
  const hookName = flowHookName(flow);
  const stepNames = flow.steps.map((s) => s.name);
  const initialState = stepNames[0] ?? "Idle";

  // Collect all event types used across handlers
  const allEvents = new Set<string>();
  const allActions = new Set<string>();
  for (const step of flow.steps) {
    for (const h of step.handlers) {
      allEvents.add(h.event);
      for (const a of h.actions) {
        if (a.startsWith("goto ")) {
          allActions.add(a.replace("goto ", ""));
        }
      }
    }
  }

  let code = `import { useReducer, useCallback } from "react";\n`;
  if (allEvents.size > 0) {
    code += `import type { ${[...allEvents].join(", ")} } from "../types";\n`;
  }
  code += "\n";

  // State type
  const stepUnion = stepNames.map((s) => `"${s}"`).join(" | ");
  code += `type Step = ${stepUnion};\n\n`;

  // Action type for reducer
  code += `type Action =\n`;
  const transitionEntries = new Set<string>();
  for (const step of flow.steps) {
    for (const h of step.handlers) {
      if (h.event) {
        transitionEntries.add(h.event);
      }
    }
  }
  if (transitionEntries.size > 0) {
    code += [...transitionEntries]
      .map((evt) => `  | { type: "${evt}"; payload: ${evt} }`)
      .join("\n");
    code += `\n`;
  }
  code += `  | { type: "reset" };\n\n`;

  // Reducer
  code += `function reducer(state: Step, action: Action): Step {\n`;
  code += `  if (action.type === "reset") return "${initialState}";\n`;
  code += `  switch (state) {\n`;

  for (const step of flow.steps) {
    code += `    case "${step.name}":\n`;
    const handlers = step.handlers;
    if (handlers.length === 0) {
      code += `      return state;\n`;
    } else {
      // Group handlers by event to avoid duplicate case labels
      const byEvent = new Map<string, typeof handlers>();
      for (const h of handlers) {
        const list = byEvent.get(h.event) ?? [];
        list.push(h);
        byEvent.set(h.event, list);
      }

      if (byEvent.size > 0) {
        code += `      switch (action.type) {\n`;
        for (const [eventName, eventHandlers] of byEvent) {
          code += `        case "${eventName}":\n`;
          let lastHadGuaranteedReturn = false;
          for (let i = 0; i < eventHandlers.length; i++) {
            const h = eventHandlers[i];
            const isLast = i === eventHandlers.length - 1;
            const prefix = i === 0 ? "          " : "          else ";
            if (h.when_is && h.when_arm) {
              const whenField = h.when_is;
              const whenArm = h.when_arm;
              code += `${prefix}if (action.payload.${whenField} === "${whenArm}") {\n`;
              for (const action of h.actions) {
                if (action.startsWith("goto ")) {
                  const target = action.replace("goto ", "");
                  code += `            return "${target}";\n`;
                }
              }
              code += `          }\n`;
              lastHadGuaranteedReturn = false;
            } else {
              if (i > 0) {
                code += `${prefix}{\n`;
                for (const action of h.actions) {
                  if (action.startsWith("goto ")) {
                    const target = action.replace("goto ", "");
                    code += `            return "${target}";\n`;
                  }
                }
                code += `          }\n`;
              } else {
                for (const action of h.actions) {
                  if (action.startsWith("goto ")) {
                    const target = action.replace("goto ", "");
                    code += `            return "${target}";\n`;
                  }
                }
              }
              lastHadGuaranteedReturn = i > 0 || (eventHandlers.length === 1 && !h.when_is);
            }
          }
          // Only emit fallback return if the last handler wasn't an unconditional else/return
          if (!lastHadGuaranteedReturn) {
            code += `          return state;\n`;
          }
        }
        code += `        default:\n          return state;\n`;
        code += `      }\n`;
      } else {
        code += `      return state;\n`;
      }
    }
  }

  code += `    default:\n      return state;\n`;
  code += `  }\n`;
  code += `}\n\n`;

  // Hook function
  code += `export function ${hookName}() {\n`;
  code += `  const [step, dispatch] = useReducer(reducer, "${initialState}");\n\n`;

  // Dispatch helpers for each event
  for (const evt of transitionEntries) {
    const camelEvt = evt.charAt(0).toLowerCase() + evt.slice(1);
    code += `  const ${camelEvt} = useCallback((payload: ${evt}) => {\n`;
    code += `    dispatch({ type: "${evt}", payload });\n`;
    code += `  }, []);\n\n`;
  }

  // Reset
  code += `  const reset = useCallback(() => {\n`;
  code += `    dispatch({ type: "reset" });\n`;
  code += `  }, []);\n\n`;

  code += `  return { step,\n`;
  for (const evt of transitionEntries) {
    const camelEvt = evt.charAt(0).toLowerCase() + evt.slice(1);
    code += `    ${camelEvt},\n`;
  }
  code += `    reset,\n`;
  code += `  };\n`;
  code += `}\n`;

  return code;
}

export function generateHooks(
  model: TsModel,
  writeFile: WriteFile,
  ensureDir: EnsureDir,
): void {
  const dir = ensureDir("src", "hooks");
  const names: string[] = [];

  for (const flow of model.flows) {
    const name = flowHookName(flow);
    names.push(name);
    writeFile(dir, `${name}.ts`, generateFlowHook(flow));
  }

  const exports = names.map((n) => `export { ${n} } from "./${n}";`).join("\n");
  writeFile(dir, "index.ts", exports + "\n");
}
