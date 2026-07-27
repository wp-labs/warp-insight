import * as fs from "fs";
import * as path from "path";
import type { TsModel } from "./generators/types";
import { generateScaffold } from "./generators/scaffold";
import { generateTypes } from "./generators/types";
import { generateComponents } from "./generators/components";
import { generateHooks } from "./generators/hooks";
import { generateApi } from "./generators/api";
import { generateStore } from "./generators/store";
import { generateApp } from "./generators/app";
import { generateIndexFiles } from "./generators/index";

const args = process.argv.slice(2);
if (args.length < 2) {
  console.error("Usage: npx tsx generate.ts <moju-ui-model.json> <output-dir>");
  process.exit(1);
}

const modelPath = args[0];
const outDir = args[1];

const model: TsModel = JSON.parse(fs.readFileSync(modelPath, "utf-8"));

const written: string[] = [];

function ensureDir(...segments: string[]): string {
  const dir = path.join(outDir, ...segments);
  fs.mkdirSync(dir, { recursive: true });
  return dir;
}

function writeFile(dir: string, filename: string, content: string): void {
  const fullDir = path.join(outDir, dir);
  fs.mkdirSync(fullDir, { recursive: true });
  const filePath = path.join(fullDir, filename);
  fs.writeFileSync(filePath, content);
  written.push(filePath);
}

generateScaffold(model, writeFile, ensureDir);
generateTypes(model, writeFile, ensureDir);
generateComponents(model, writeFile, ensureDir);
generateHooks(model, writeFile, ensureDir);
generateApi(model, writeFile, ensureDir);
generateStore(model, writeFile, ensureDir);
generateApp(model, writeFile, ensureDir);
generateIndexFiles(model, writeFile, ensureDir);

console.log(`Generated ${written.length} files in ${outDir}`);
