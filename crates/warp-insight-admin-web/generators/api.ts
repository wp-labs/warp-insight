import { pascalCase, camelCase } from "change-case";
import type { TsModel, TsInterface, TsInterfaceEntry } from "./types";

type WriteFile = (dir: string, filename: string, content: string) => void;
type EnsureDir = (...segments: string[]) => string;

function apiHookName(iface: TsInterface): string {
  return `use${pascalCase(iface.name)}`;
}

function generateApiHook(iface: TsInterface): string {
  const hookName = apiHookName(iface);
  const entries = iface.entries;

  // Collect types used (input params + output events)
  const inputTypes = new Set(entries.map((e) => e.input).filter(Boolean) as string[]);
  const outputEventTypes = new Set<string>();
  for (const entry of entries) {
    for (const o of entry.outputs) {
      outputEventTypes.add(o);
    }
  }
  const allTypeImports = new Set([...inputTypes, ...outputEventTypes]);

  let code = `import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";\n`;
  code += `import { useStore } from "../store";\n`;
  if (allTypeImports.size > 0) {
    code += `import type { ${[...allTypeImports].join(", ")} } from "../types";\n`;
  }
  code += "\n";

  for (const entry of entries) {
    const methodName = pascalCase(entry.name);
    const queryKey = camelCase(iface.name) + "." + camelCase(entry.name);

    if (entry.input) {
      code += `// Query: fetch ${entry.name}\n`;
      code += `export function use${methodName}Query(input: ${entry.input}) {\n`;
      code += `  return useQuery({\n`;
      code += `    queryKey: ["${queryKey}", input],\n`;
      code += `    queryFn: async () => {\n`;
      code += `      // TODO: replace with actual API call\n`;
      code += `      const response = await fetch("/api/${camelCase(iface.name)}/${camelCase(entry.name)}", {\n`;
      code += `        method: "POST",\n`;
      code += `        headers: { "Content-Type": "application/json" },\n`;
      code += `        body: JSON.stringify(input),\n`;
      code += `      });\n`;
      code += `      if (!response.ok) throw new Error("${entry.name} query failed");\n`;
      code += `      return response.json();\n`;
      code += `    },\n`;
      code += `  });\n`;
      code += `}\n\n`;
    }

    if (entry.outputs.length > 0) {
      code += `// Mutation: emit ${entry.name}\n`;
      code += `export function use${methodName}Mutation() {\n`;
      code += `  const queryClient = useQueryClient();\n`;
      const storeDispatch = entry.outputs
        .map((o) => `  const send${o} = useStore((s) => s.send${o});`)
        .join("\n");
      if (entry.outputs.length > 0) {
        code += storeDispatch + "\n";
      }
      code += `  return useMutation({\n`;
      code += `    mutationFn: async (payload: Record<string, unknown>) => {\n`;
      code += `      // TODO: replace with actual API call\n`;
      code += `      const response = await fetch("/api/${camelCase(iface.name)}/${camelCase(entry.name)}", {\n`;
      code += `        method: "POST",\n`;
      code += `        headers: { "Content-Type": "application/json" },\n`;
      code += `        body: JSON.stringify(payload),\n`;
      code += `      });\n`;
      code += `      if (!response.ok) throw new Error("${entry.name} mutation failed");\n`;
      code += `      return response.json();\n`;
      code += `    },\n`;
      code += `    onSuccess: (data) => {\n`;
      for (const o of entry.outputs) {
        code += `      // TODO: verify API response shape matches ${o}\n`;
        code += `      send${o}(data as unknown as ${o});\n`;
      }
      code += `      queryClient.invalidateQueries();\n`;
      code += `    },\n`;
      code += `  });\n`;
      code += `}\n\n`;
    }
  }

  return code;
}

export function generateApi(
  model: TsModel,
  writeFile: WriteFile,
  ensureDir: EnsureDir,
): void {
  const dir = ensureDir("src", "api");
  const allExports: string[] = [];

  for (const iface of model.interfaces) {
    const fileName = apiHookName(iface);
    writeFile(dir, `${fileName}.ts`, generateApiHook(iface));

    // Collect actual exported function names for the barrel export
    for (const entry of iface.entries) {
      if (entry.input) {
        allExports.push(`use${pascalCase(entry.name)}Query`);
      }
      if (entry.outputs.length > 0) {
        allExports.push(`use${pascalCase(entry.name)}Mutation`);
      }
    }
  }

  // Generate barrel export with all hook names from each file
  // Group by file
  const fileExports = new Map<string, string[]>();
  for (const iface of model.interfaces) {
    const fileName = apiHookName(iface);
    const hooks: string[] = [];
    for (const entry of iface.entries) {
      if (entry.input) hooks.push(`use${pascalCase(entry.name)}Query`);
      if (entry.outputs.length > 0) hooks.push(`use${pascalCase(entry.name)}Mutation`);
    }
    fileExports.set(fileName, hooks);
  }

  const exportLines: string[] = [];
  for (const [fileName, hooks] of fileExports) {
    exportLines.push(`export { ${hooks.join(", ")} } from "./${fileName}";`);
  }
  writeFile(dir, "index.ts", exportLines.join("\n") + "\n");
}
