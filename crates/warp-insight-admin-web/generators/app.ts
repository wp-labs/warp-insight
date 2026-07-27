import { camelCase } from "change-case";
import type { TsModel, TsRegion } from "./types";

type WriteFile = (dir: string, filename: string, content: string) => void;
type EnsureDir = (...segments: string[]) => string;

function isRouteRegion(region: TsRegion): boolean {
  return region.name.endsWith("Page") || region.name.endsWith("Screen");
}

function defaultRouteRegion(routeRegions: TsRegion[]): TsRegion | undefined {
  const home = routeRegions.find((s) => /Home(Page|Screen)$/.test(s.name));
  if (home) return home;
  // Prefer ProductListScreen as the home screen for older generated examples.
  const prodList = routeRegions.find((s) => s.name === "ProductListScreen");
  if (prodList) return prodList;
  // Avoid confirmation/result screens as default
  const firstNonConfirm = routeRegions.find(
    (s) =>
      !s.name.endsWith("ConfirmationScreen") &&
      !s.name.endsWith("ConfirmationPage") &&
      !s.name.endsWith("ResultScreen") &&
      !s.name.endsWith("ResultPage")
  );
  if (firstNonConfirm) return firstNonConfirm;
  return routeRegions[0];
}

function routePath(region: TsRegion, defaultRegion?: TsRegion): string {
  if (defaultRegion && region.name === defaultRegion.name) {
    return "/";
  }
  let base = region.name.replace(/(?:Page|Screen)$/, "");
  base = base.replace(/^Subsystem/, "");
  base = base.replace(/^Admin/, "");
  base = base.replace(/^Agent/, "");
  if (base === "ControlCenter") {
    return "/control";
  }
  return "/" + camelCase(base || region.name);
}

function appShellRegion(model: TsModel): TsRegion | undefined {
  return model.regions.find((r) => r.name === "AppShell");
}

export function generateApp(
  model: TsModel,
  writeFile: WriteFile,
  ensureDir: EnsureDir,
): void {
  const routeRegions = model.regions.filter(isRouteRegion);
  const defaultRegion = defaultRouteRegion(routeRegions);
  const appShell = appShellRegion(model);

  // main.tsx
  writeFile(
    "src",
    "main.tsx",
    `import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { BrowserRouter } from "react-router-dom";
import { App } from "./App";
import "./index.css";

const queryClient = new QueryClient();

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <BrowserRouter>
        <App />
      </BrowserRouter>
    </QueryClientProvider>
  </StrictMode>,
);
`,
  );

  // Global CSS
  writeFile(
    "src",
    "index.css",
    `*,
*::before,
*::after {
  box-sizing: border-box;
  margin: 0;
  padding: 0;
}

html, body, #root {
  height: 100%;
  width: 100%;
}

body {
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
  -webkit-font-smoothing: antialiased;
  background: #f5f7fb;
  color: #172033;
}
`,
  );

  // App.tsx
  let appCode = `import { Routes, Route, Navigate } from "react-router-dom";\n\n`;

  // Import AppShell (it renders its own children)
  if (appShell) {
    appCode += `import { ${appShell.name} } from "./components/${appShell.name}";\n`;
  }

  // Import route regions (they import their own children)
  for (const region of routeRegions) {
    appCode += `import { ${region.name} } from "./components/${region.name}";\n`;
  }

  appCode += "\n";

  // Build App component
  appCode += `export function App() {\n`;

  if (appShell) {
    appCode += `  return (\n`;
    appCode += `    <${appShell.name}>\n`;
    appCode += `      <Routes>\n`;
    for (const region of routeRegions) {
      appCode += `        <Route path="${routePath(region, defaultRegion)}" element={<${region.name} />} />\n`;
    }
    const defaultRoute = routePath(
      defaultRegion ?? routeRegions[0] ?? { name: "ProductListScreen" } as TsRegion,
      defaultRegion,
    );
    appCode += `        <Route path="*" element={<Navigate to="${defaultRoute}" replace />} />\n`;
    appCode += `      </Routes>\n`;
    appCode += `    </${appShell.name}>\n`;
    appCode += `  );\n`;
  } else {
    appCode += `  return (\n`;
    appCode += `    <Routes>\n`;
    for (const region of routeRegions) {
      appCode += `      <Route path="${routePath(region, defaultRegion)}" element={<${region.name} />} />\n`;
    }
    const defaultRoute = routePath(
      defaultRegion ?? routeRegions[0] ?? { name: "ProductListScreen" } as TsRegion,
      defaultRegion,
    );
    appCode += `      <Route path="*" element={<Navigate to="${defaultRoute}" replace />} />\n`;
    appCode += `    </Routes>\n`;
    appCode += `  );\n`;
  }

  appCode += `}\n`;

  writeFile("src", "App.tsx", appCode);
}
