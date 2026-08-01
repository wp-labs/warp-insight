import { execFileSync } from "node:child_process";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";

const sentinelToken = "bundle-leak-sentinel-admin-token";
const forbiddenFragments = [
  sentinelToken,
  "VITE_WARP_INSIGHT_ADMIN_TOKEN",
  "install-test-admin-token",
  "localStorage",
  "sessionStorage",
  "warpInsightAdminToken",
];

execFileSync("npm", ["run", "build"], {
  cwd: process.cwd(),
  env: {
    ...process.env,
    VITE_WARP_INSIGHT_ADMIN_TOKEN: sentinelToken,
  },
  stdio: "inherit",
});

const leaked = bundledFiles("dist")
  .map((file) => ({
    file,
    content: readFileSync(file, "utf8"),
  }))
  .flatMap(({ file, content }) =>
    forbiddenFragments
      .filter((fragment) => content.includes(fragment))
      .map((fragment) => `${file}: ${fragment}`),
  );

if (leaked.length > 0) {
  throw new Error(
    `admin token material leaked into bundle:\n${leaked.join("\n")}`,
  );
}

function bundledFiles(root) {
  return readdirSync(root).flatMap((entry) => {
    const path = join(root, entry);
    const stat = statSync(path);
    if (stat.isDirectory()) return bundledFiles(path);
    if (path.endsWith(".js") || path.endsWith(".html")) return [path];
    return [];
  });
}
