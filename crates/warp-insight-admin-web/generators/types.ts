import * as path from "path";

// ── JSON model types ──

export interface TsField {
  name: string;
  ts_type: string;
  unique: boolean;
  optional: boolean;
}

export interface TsState {
  name: string;
  values: string[];
}

export interface TsVariantArm {
  kind: string;
  payload: { fields: TsField[] } | null;
}

export interface TsVariant {
  name: string;
  arms: TsVariantArm[];
}

export interface TsView {
  name: string;
  fields: TsField[];
}

export interface TsEventDef {
  name: string;
  fields: TsField[];
}

export interface TsMessageDef {
  name: string;
  role: string;
  base_struct: string | null;
  fields: TsField[];
}

export interface TsSlotFill {
  slot_name: string;
  component: string;
}

export interface TsContainsClause {
  region: string;
  prop_name: string;
  slot_fills: TsSlotFill[];
}

export interface TsInteraction {
  op: string;
  handler_name: string;
  emit_event: string;
}

export interface TsRepeatClause {
  region: string;
  item: string;
  data_path: string;
  separator: string | null;
}

export interface TsRegion {
  name: string;
  orientation: string | null;
  contains: TsContainsClause[];
  when: string | null;
  layout_props: string[];
  interactions: TsInteraction[];
  repeat: TsRepeatClause | null;
}

export interface TsHandler {
  event: string;
  when_is: string | null;
  when_arm: string | null;
  actions: string[];
}

export interface TsStep {
  name: string;
  guard_event: string | null;
  handlers: TsHandler[];
  actions: string[];
}

export interface TsFlow {
  name: string;
  trigger: string;
  steps: TsStep[];
}

export interface TsInterfaceEntry {
  name: string;
  input: string | null;
  outputs: string[];
  calls: string | null;
}

export interface TsInterface {
  name: string;
  entries: TsInterfaceEntry[];
}

export interface TsModule {
  name: string;
  layer: string;
  owns: string[];
  depends: string[];
  implements: string[];
  responsible_for: string[];
}

export interface TsProfile {
  language: string;
  framework: string;
  deps: string[];
  dev_deps: string[];
}

export interface TsTypes {
  states: TsState[];
  variants: TsVariant[];
  views: TsView[];
  events: TsEventDef[];
  messages: TsMessageDef[];
}

export interface TsModel {
  domain: string;
  crate_name: string;
  types: TsTypes;
  regions: TsRegion[];
  flows: TsFlow[];
  interfaces: TsInterface[];
  modules: TsModule[];
  profile: TsProfile;
}

// ── codegen helpers ──

type WriteFile = (dir: string, filename: string, content: string) => void;
type EnsureDir = (...segments: string[]) => string;

function toPascalCase(str: string): string {
  return str[0].toUpperCase() + str.slice(1);
}

function fieldsToInterface(fields: TsField[]): string {
  if (fields.length === 0) return "";
  return fields
    .map((f) => {
      const opt = f.optional ? "?" : "";
      return `  ${f.name}${opt}: ${f.ts_type};`;
    })
    .join("\n");
}

// ── cross-file import resolution ──

function typeFileName(typeName: string, model: TsTypes): string | null {
  if (model.states.some((s) => s.name === typeName)) return "./enums";
  if (model.variants.some((v) => v.name === typeName)) return "./variants";
  if (model.views.some((v) => v.name === typeName)) return "./views";
  if (model.events.some((e) => e.name === typeName)) return "./events";
  if (model.messages.some((m) => m.name === typeName)) return "./messages";
  return null;
}

function collectFieldTypeImports(fields: TsField[], modelTypes: TsTypes, currentCategory: string): Map<string, string[]> {
  const imports = new Map<string, string[]>();
  const knownPrimitives = new Set(["string", "number", "boolean", "any", "void", "Date", "unknown", "Record<string, unknown>"]);
  for (const f of fields) {
    const typeName = f.ts_type.replace(/\[\]$/, "");
    if (knownPrimitives.has(typeName)) continue;
    const source = typeFileName(typeName, modelTypes);
    if (source && source !== `./${currentCategory}`) {
      const existing = imports.get(source) ?? [];
      if (!existing.includes(typeName)) {
        existing.push(typeName);
        imports.set(source, existing);
      }
    }
  }
  return imports;
}

function formatImports(imports: Map<string, string[]>, forceNewline = true): string {
  let result = "";
  for (const [source, types] of imports) {
    result += `import type { ${types.join(", ")} } from "${source}";\n`;
  }
  if (result && forceNewline) result += "\n";
  return result;
}

// ── generators ──

function generateEnums(states: TsState[]): string {
  if (states.length === 0) return "// No state enums defined\n";
  return states
    .map((s) => {
      const values = s.values.map((v) => `"${v}"`).join(" | ");
      return `export type ${s.name} = ${values};`;
    })
    .join("\n\n");
}

function generateVariants(variants: TsVariant[]): string {
  if (variants.length === 0) return "// No variant types defined\n";
  return variants
    .map((v) => {
      const arms = v.arms
        .map((arm) => {
          if (arm.payload && arm.payload.fields.length > 0) {
            const body = arm.payload.fields
              .map((f) => `    ${f.name}: ${f.ts_type};`)
              .join("\n");
            return `  | { kind: "${arm.kind}";\n${body} }`;
          }
          return `  | { kind: "${arm.kind}" }`;
        })
        .join("\n");
      return `export type ${v.name} =\n${arms};`;
    })
    .join("\n\n");
}

function generateEventTypes(events: TsEventDef[], modelTypes: TsTypes): string {
  if (events.length === 0) return "// No event types defined\n";
  const allImports = new Map<string, string[]>();
  const knownPrimitives = new Set(["string", "number", "boolean", "any", "void", "Date", "unknown", "Record<string, unknown>"]);
  for (const e of events) {
    for (const f of e.fields) {
      const typeName = f.ts_type.replace(/\[\]$/, "");
      if (knownPrimitives.has(typeName)) continue;
      const source = typeFileName(typeName, modelTypes);
      if (source && source !== "./events") {
        const existing = allImports.get(source) ?? [];
        if (!existing.includes(typeName)) {
          existing.push(typeName);
          allImports.set(source, existing);
        }
      }
    }
  }
  let result = formatImports(allImports);
  result += events
    .map((e) => {
      const body = fieldsToInterface(e.fields);
      if (!body) return `export interface ${e.name} {}`;
      return `export interface ${e.name} {\n${body}\n}`;
    })
    .join("\n\n");
  const unionMembers = events.map((e) => e.name).join(" | ");
  return result + `\n\nexport type AppEvent = ${unionMembers};\n`;
}

function generateMessageTypes(messages: TsMessageDef[], modelTypes: TsTypes): string {
  if (messages.length === 0) return "// No message types defined\n";

  const viewNames = new Set(modelTypes.views.map((v) => v.name));
  const allImports = new Map<string, string[]>();

  const addImport = (typeName: string) => {
    const source = typeFileName(typeName, modelTypes);
    if (source && source !== "./messages") {
      const existing = allImports.get(source) ?? [];
      if (!existing.includes(typeName)) {
        existing.push(typeName);
        allImports.set(source, existing);
      }
    }
  };

  const lines: string[] = [];

  for (const m of messages) {
    const ext = m.base_struct ? ` extends ${m.base_struct}` : "";
    const body = fieldsToInterface(m.fields);
    const roleComment = `/** @role ${m.role} */`;

    if (m.base_struct && viewNames.has(m.base_struct)) {
      addImport(m.base_struct);
    }

    for (const f of m.fields) {
      const typeName = f.ts_type.replace(/\[\]$/, "");
      const knownPrimitives = new Set(["string", "number", "boolean", "any", "void", "Date", "unknown", "Record<string, unknown>"]);
      if (!knownPrimitives.has(typeName)) {
        addImport(typeName);
      }
    }

    if (!body) {
      lines.push(`${roleComment}\nexport interface ${m.name}${ext} {}`);
    } else {
      lines.push(`${roleComment}\nexport interface ${m.name}${ext} {\n${body}\n}`);
    }
  }

  const result = formatImports(allImports);
  return result + lines.join("\n\n");
}

function generateViewTypes(views: TsView[], modelTypes: TsTypes): string {
  if (views.length === 0) return "// No view types defined\n";
  const allImports = new Map<string, string[]>();
  const knownPrimitives = new Set(["string", "number", "boolean", "any", "void", "Date", "unknown", "Record<string, unknown>"]);
  for (const v of views) {
    for (const f of v.fields) {
      const typeName = f.ts_type.replace(/\[\]$/, "");
      if (knownPrimitives.has(typeName)) continue;
      const source = typeFileName(typeName, modelTypes);
      if (source && source !== "./views") {
        const existing = allImports.get(source) ?? [];
        if (!existing.includes(typeName)) {
          existing.push(typeName);
          allImports.set(source, existing);
        }
      }
    }
  }
  let result = formatImports(allImports);
  result += views
    .map((v) => {
      const body = fieldsToInterface(v.fields);
      if (!body) return `export interface ${v.name} {}`;
      return `export interface ${v.name} {\n${body}\n}`;
    })
    .join("\n\n");
  return result;
}

export function generateTypes(
  model: TsModel,
  writeFile: WriteFile,
  ensureDir: EnsureDir,
): void {
  const dir = ensureDir("src", "types");

  const types = model.types;
  writeFile(dir, "enums.ts", generateEnums(types.states));
  writeFile(dir, "variants.ts", generateVariants(types.variants));
  writeFile(dir, "events.ts", generateEventTypes(types.events, types));
  writeFile(dir, "messages.ts", generateMessageTypes(types.messages, types));
  writeFile(dir, "views.ts", generateViewTypes(types.views, types));

  writeFile(
    dir,
    "index.ts",
    `export type * from "./enums";
export type * from "./variants";
export type * from "./events";
export type * from "./messages";
export type * from "./views";
`,
  );
}
