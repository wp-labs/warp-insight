import type { TsModel } from "./types";

type WriteFile = (dir: string, filename: string, content: string) => void;
type EnsureDir = (...segments: string[]) => string;

export function generateScaffold(
  model: TsModel,
  writeFile: WriteFile,
  ensureDir: EnsureDir,
): void {
  const domain = model.domain.toLowerCase();

  // package.json
  const pkg = {
    name: model.crate_name.replace(/_/g, "-"),
    private: true,
    version: "0.0.0",
    type: "module",
    scripts: {
      dev: "vite",
      build: "tsc -b && vite build",
      preview: "vite preview",
    },
    dependencies: {
      react: "^18.3.0",
      "react-dom": "^18.3.0",
      "react-router-dom": "^6.26.0",
      "@tanstack/react-query": "^5.51.0",
      "change-case": "^5.4.4",
      zustand: "^4.5.0",
    },
    devDependencies: {
      ...Object.fromEntries(
        model.profile.dev_deps.map((d) => {
          if (d === "typescript") return [d, "^5.5.0"];
          if (d === "@types/react") return [d, "^18.3.0"];
          if (d === "@types/react-dom") return [d, "^18.3.0"];
          if (d === "vite") return [d, "^5.4.0"];
          if (d === "@vitejs/plugin-react") return [d, "^4.3.0"];
          if (d === "prettier") return [d, "^3.3.0"];
          if (d === "tsx") return [d, "^4.19.0"];
          return [d, "^1.0.0"];
        }),
      ),
      tsx: "^4.19.0",
    },
  };

  writeFile(".", "package.json", JSON.stringify(pkg, null, 2) + "\n");

  // tsconfig.json
  writeFile(
    ".",
    "tsconfig.json",
    JSON.stringify(
      {
        compilerOptions: {
          target: "ES2020",
          useDefineForClassFields: true,
          lib: ["ES2020", "DOM", "DOM.Iterable"],
          module: "ESNext",
          skipLibCheck: true,
          moduleResolution: "bundler",
          allowImportingTsExtensions: true,
          isolatedModules: true,
          moduleDetection: "force",
          noEmit: true,
          jsx: "react-jsx",
          strict: true,
          noUnusedLocals: true,
          noUnusedParameters: false,
          noFallthroughCasesInSwitch: true,
          forceConsistentCasingInFileNames: true,
        },
        include: ["src"],
      },
      null,
      2,
    ) + "\n",
  );

  // vite.config.ts
  writeFile(
    ".",
    "vite.config.ts",
    `import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  server: {
    proxy: {
      "/api": {
        target: "http://localhost:3000",
        changeOrigin: true,
      },
    },
  },
});
`,
  );

  // index.html
  writeFile(
    ".",
    "index.html",
    `<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>${model.domain}</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
`,
  );

  // vite-env.d.ts — type declarations for CSS modules and assets
  const envDir = ensureDir("src");
  writeFile(
    envDir,
    "vite-env.d.ts",
    `/// <reference types="vite/client" />

declare module "*.module.css" {
  const classes: { readonly [key: string]: string };
  export default classes;
}
`,
  );
}
