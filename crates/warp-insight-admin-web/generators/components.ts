import { camelCase } from "change-case";
import type { TsModel, TsRegion, TsView } from "./types";

type WriteFile = (dir: string, filename: string, content: string) => void;
type EnsureDir = (...segments: string[]) => string;

function eventPropName(eventName: string): string {
  return "on" + eventName;
}

function isRouteRegion(region: TsRegion): boolean {
  return region.name.endsWith("Page") || region.name.endsWith("Screen");
}

function defaultRouteRegion(routeRegions: TsRegion[]): TsRegion | undefined {
  return (
    routeRegions.find((region) => /Home(Page|Screen)$/.test(region.name)) ??
    routeRegions[0]
  );
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

function routeLabel(region: TsRegion, defaultRegion?: TsRegion): string {
  if (defaultRegion && region.name === defaultRegion.name) {
    return "总览";
  }
  let base = region.name.replace(/(?:Page|Screen)$/, "");
  base = base.replace(/^Subsystem/, "");
  base = base.replace(/^Admin/, "");
  base = base.replace(/^Agent/, "");
  if (base === "ControlCenter") return "控制中心";
  if (base === "Install") return "安装";
  return base.replace(/([a-z0-9])([A-Z])/g, "$1 $2") || region.name;
}

function chineseLabel(name: string): string | null {
  const normalized = name.replace(/^Subsystem/, "");
  const labels: Record<string, string> = {
    TotalAgentMetric: "Agent 总数",
    OnlineAgentMetric: "在线 Agent 数",
    UnhealthyAgentMetric: "异常 Agent 数",
    LastSeenLagMetric: "上报延迟",
    NoAbnormalAgentPlaceholder: "当前没有异常 Agent",
    X86LinuxInstallCode: "X86 Linux 安装代码",
    ArmLinuxInstallCode: "Arm Linux 安装代码",
    PauseAgent: "暂停 Agent",
    UpgradeAgent: "升级 Agent",
    UpgradeAgentRemotely: "远程升级 Agent",
    DispatchReceipt: "派发回执",
    AdminOperator: "管理员",
    AdminOperatorAccess: "操作权限",
    AdminOperatorIdentity: "管理员身份",
    AgentRuntimeStatusView: "Agent 运行状态",
    InstanceId: "实例 ID",
    Version: "版本",
    Status: "状态",
    Health: "健康状态",
    LastSeenAt: "最后上报时间",
    TargetVersion: "目标版本",
    CreatedAt: "创建时间",
    DispatchId: "派发 ID",
    Http: "HTTP",
    GetAgentInstallCode: "获取安装代码",
    SubmitPauseAction: "提交暂停",
    SubmitUpgradeAction: "提交升级",
    RefreshRuntimeStatusAction: "刷新运行状态",
    AdminPauseAgent: "管理员暂停 Agent",
    AdminShowAgentRuntimeStatus: "管理员查看 Agent 运行状态",
    AdminUpgradeAgent: "管理员升级 Agent",
    AgentHealthBadge: "Agent 健康标识",
    AgentHealthStatusFilter: "健康状态筛选",
    AgentId: "Agent 标识",
    AgentInstanceText: "Agent 实例",
    AgentLastSeenAtText: "最后上报时间",
    AgentOnlineStatusBadge: "Agent 在线标识",
    AgentOnlineStatusFilter: "在线状态筛选",
    AgentStatusSearchInput: "Agent 状态搜索",
    AgentVersionFilter: "Agent 版本筛选",
    AgentVersionText: "Agent 版本",
    ArmLinuxAgentInstallCode: "Arm Linux 安装代码",
    PauseAgentInput: "暂停 Agent 输入",
    PauseAgentUsecaseMeta: "暂停 Agent 用例信息",
    PauseDispatchCreatedAtText: "暂停派发创建时间",
    PauseDispatchIdText: "暂停派发 ID",
    RuntimeStatusAgentInput: "运行状态 Agent 输入",
    ShowAgentRuntimeStatusUsecaseMeta: "查看运行状态用例信息",
    ShowAgentRuntimeStatus: "查看 Agent 运行状态",
    TargetVersionInput: "目标版本输入",
    UpgradeAgentInput: "升级 Agent 输入",
    UpgradeAgentRemotelyUsecaseMeta: "远程升级 Agent 用例信息",
    UpgradeDispatchCreatedAtText: "升级派发创建时间",
    UpgradeDispatchIdText: "升级派发 ID",
    X86LinuxAgentInstallCode: "X86 Linux 安装代码",
  };
  return labels[normalized] ?? null;
}

function displayLabel(name: string): string {
  const zh = chineseLabel(name);
  if (zh) return zh;
  let base = name.replace(/^Subsystem/, "");
  base = base.replace(/([a-z0-9])([A-Z])/g, "$1 $2");
  return base || name;
}

function defaultLeafContent(name: string): string {
  const label = displayLabel(name);
  if (name.includes("X86LinuxInstallCode")) {
    const command = "curl -fsSL http://127.0.0.1:3000/api/v1/agent/install/x86/install.sh | bash";
    return `      <div className={styles.label}>${label}</div>
      <code className={styles.code}>${command}</code>
      <button className={styles.copyButton} type="button" onClick={() => navigator.clipboard?.writeText("${command}")}>
        复制
      </button>
`;
  }
  if (name.includes("ArmLinuxInstallCode")) {
    const command = "curl -fsSL http://127.0.0.1:3000/api/v1/agent/install/arm/install.sh | bash";
    return `      <div className={styles.label}>${label}</div>
      <code className={styles.code}>${command}</code>
      <button className={styles.copyButton} type="button" onClick={() => navigator.clipboard?.writeText("${command}")}>
        复制
      </button>
`;
  }
  return `      <div className={styles.label}>${label}</div>\n`;
}

function pageTitle(name: string): string {
  if (/Home(Page|Screen)$/.test(name)) return "Agent 总览";
  if (/Install(Page|Screen)$/.test(name)) return "Agent 安装";
  if (/ControlCenter(Page|Screen)$/.test(name)) return "Agent 控制中心";
  return displayLabel(name.replace(/(?:Page|Screen)$/, ""));
}

function pageSummary(name: string): string {
  if (/Home(Page|Screen)$/.test(name)) {
    return "查看已接入 Agent 的在线状态、健康状态和需要处理的异常节点。";
  }
  if (/Install(Page|Screen)$/.test(name)) {
    return "复制对应架构的安装命令，在目标 Linux 主机上执行。";
  }
  if (/ControlCenter(Page|Screen)$/.test(name)) {
    return "面向已注册 Agent 派发常用运维控制操作。";
  }
  return "WarpInsight 管理工作台。";
}

// ── View detection ──

function findPrimaryView(regionName: string, model: TsModel): TsView | null {
  const suffixes = ["View", "Row", "Card", "Item", "Section", "Region", "Screen"];
  for (const suffix of suffixes) {
    const base = regionName.replace(new RegExp(`${suffix}$`), "");
    const match = model.types.views.find((v) => v.name === base);
    if (match) return match;
  }
  return model.types.views.find((v) => v.name === regionName) ?? null;
}

// ── Payload construction ──

function eventDefaultPayload(emitEvent: string, model: TsModel, viewPropName?: string, view?: TsView): string {
  const eventDef = model.types.events.find((e) => e.name === emitEvent);
  if (!eventDef || eventDef.fields.length === 0) return "{}";
  const fields = eventDef.fields
    .map((f) => {
      // 1. Direct field name match on view
      if (view && viewPropName) {
        const viewField = view.fields.find((vf) => vf.name === f.name);
        if (viewField) return `${f.name}: ${viewPropName}.${f.name}`;
      }
      // 2. Event field matches view prop name → use .id
      if (viewPropName && f.name === viewPropName) {
        return `${f.name}: ${viewPropName}.id`;
      }
      // 3. Fallback with defaults
      let dv = `""`;
      if (f.ts_type === "number") dv = "0";
      else if (f.ts_type === "boolean") dv = "false";
      else if (f.ts_type.endsWith("[]")) dv = "[]";
      return `${f.name}: ${dv}`;
    })
    .join(", ");
  return `{ ${fields} }`;
}

// ── Descendant interaction collection (for callback propagation) ──

function collectDescendantInteractions(
  region: TsRegion,
  model: TsModel,
  stopAtRegion?: string,
): { emit_event: string; handler_name: string }[] {
  const results: { emit_event: string; handler_name: string }[] = [];
  const seen = new Set<string>();

  function walk(r: TsRegion) {
    if (stopAtRegion && r.name === stopAtRegion) return;
    for (const ix of r.interactions) {
      const key = `${ix.handler_name}:${ix.emit_event}`;
      if (!seen.has(key)) {
        seen.add(key);
        results.push({ emit_event: ix.emit_event, handler_name: ix.handler_name });
      }
    }
    for (const c of r.contains) {
      const child = model.regions.find((cr) => cr.name === c.region);
      if (child) walk(child);
    }
    if (r.repeat) {
      const repeatR = model.regions.find((rr) => rr.name === r.repeat!.region);
      if (repeatR) walk(repeatR);
    }
  }

  for (const c of region.contains) {
    const child = model.regions.find((cr) => cr.name === c.region);
    if (child) walk(child);
  }
  return results;
}

function buildComponentCode(region: TsRegion, model: TsModel): string {
  const name = region.name;

  if (name.endsWith("TopNavigation")) {
    const routeRegions = model.regions.filter(isRouteRegion);
    const defaultRegion = defaultRouteRegion(routeRegions);
    const links = routeRegions
      .map((routeRegion) => {
        const path = routePath(routeRegion, defaultRegion);
        const label = routeLabel(routeRegion, defaultRegion);
        const end = path === "/" ? " end" : "";
        return `            <NavLink
              className={({ isActive }) =>
                isActive ? \`\${styles.link} \${styles.active}\` : styles.link
              }
              to="${path}"${end}
            >
              ${label}
            </NavLink>`;
      })
      .join("\n");
    return `import { NavLink } from "react-router-dom";
import styles from "./${name}.module.css";

interface ${name}Props {
  children?: React.ReactNode;
}

export function ${name}({ children }: ${name}Props) {
  return (
    <div className={styles.container}>
      {children ?? (
        <>
          <div className={styles.brand}>WarpInsight 管理台</div>
          <nav className={styles.links} aria-label="主导航">
${links}
          </nav>
        </>
      )}
    </div>
  );
}
`;
  }

  // Collect all component imports (children + repeat + slot fill components)
  const componentImports = new Set<string>();
  for (const c of region.contains) {
    componentImports.add(c.region);
    for (const sf of c.slot_fills) {
      if (!sf.component.startsWith('"')) {
        componentImports.add(sf.component);
      }
    }
  }
  if (region.repeat) {
    componentImports.add(region.repeat.region);
  }

  let code = `import styles from "./${name}.module.css";\n`;

  for (const child of componentImports) {
    code += `import { ${child} } from "./${child}";\n`;
  }

  // Collect all event types used by this region (own + repeat child's + descendants)
  const allEventTypes = new Set<string>();
  for (const ix of region.interactions) {
    allEventTypes.add(ix.emit_event);
  }
  if (region.repeat) {
    const repeatRegion = model.regions.find((r) => r.name === region.repeat!.region);
    if (repeatRegion) {
      for (const ix of repeatRegion.interactions) {
        allEventTypes.add(ix.emit_event);
      }
    }
  }
  const descendantIxs = collectDescendantInteractions(region, model);
  for (const ix of descendantIxs) {
    allEventTypes.add(ix.emit_event);
  }

  // Collect view types used in repeat item annotations
  const extraTypeImports = new Set<string>();
  if (region.repeat) {
    const repeatChildView = findPrimaryView(region.repeat.region, model);
    if (repeatChildView) {
      extraTypeImports.add(repeatChildView.name);
    }
  }

  const allTypeImports = new Set([...allEventTypes, ...extraTypeImports]);
  if (allTypeImports.size > 0) {
    code += `import type { ${[...allTypeImports].join(", ")} } from "../types";\n`;
  }

  code += "\n";

  // ── View detection ──
  const primaryView = findPrimaryView(name, model);
  const viewPropName = primaryView ? camelCase(primaryView.name) : null;
  const viewTypeName = primaryView?.name ?? "Record<string, unknown>";
  const hasInteractions = region.interactions.length > 0;
  const hasChildren = region.contains.length > 0;
  const hasRepeat = !!region.repeat;
  const hasContentArea = region.contains.some((c) => c.region === "ContentArea");
  const isLeaf = !hasChildren && !hasRepeat;
  const isRouteContainer = isRouteRegion(region);

  // Repeat child's view type (used for items prop and map callback)
  const repeatChildView = hasRepeat ? findPrimaryView(region.repeat!.region, model) : null;
  const repeatItemType = repeatChildView?.name ?? "Record<string, any>";

  // ── Props interface ──
  const seenProps = new Set<string>();
  const propLines: string[] = [];

  // Own interactions
  for (const ix of region.interactions) {
    const propName = eventPropName(ix.emit_event);
    if (!seenProps.has(propName)) {
      seenProps.add(propName);
      propLines.push(`  ${propName}?: (payload: ${ix.emit_event}) => void;`);
    }
  }

  // Repeat child's interactions (must be accepted as props to forward)
  if (region.repeat) {
    const repeatR = model.regions.find((r) => r.name === region.repeat!.region);
    if (repeatR) {
      for (const ix of repeatR.interactions) {
        const propName = eventPropName(ix.emit_event);
        if (!seenProps.has(propName)) {
          seenProps.add(propName);
          propLines.push(`  ${propName}?: (payload: ${ix.emit_event}) => void;`);
        }
      }
    }
  }

  // Descendant interactions (propagate callbacks from children to parent)
  for (const ix of descendantIxs) {
    const propName = eventPropName(ix.emit_event);
    if (!seenProps.has(propName)) {
      seenProps.add(propName);
      propLines.push(`  ${propName}?: (payload: ${ix.emit_event}) => void;`);
    }
  }

  // View data prop
  if (region.repeat) {
    propLines.push(`  items?: ${repeatItemType}[];`);
  } else if (primaryView && isLeaf) {
    propLines.push(`  ${viewPropName}?: ${viewTypeName};`);
  }

  // Only accept children if the component renders them (ContentArea or simple/leaf)
  const needsChildren = hasContentArea || isLeaf || (hasInteractions && isLeaf);

  // Function params (destructured)
  const functionParams: string[] = [];
  const ownPropNames = [...new Set(region.interactions.map((ix) => eventPropName(ix.emit_event)))];
  functionParams.push(...ownPropNames);

  if (region.repeat) {
    const repeatR = model.regions.find((r) => r.name === region.repeat!.region);
    if (repeatR) {
      const childForwards = repeatR.interactions.map((ix) => eventPropName(ix.emit_event));
      for (const cf of childForwards) {
        if (!functionParams.includes(cf)) {
          functionParams.push(cf);
        }
      }
    }
    functionParams.push("items");
  }
  // Descendant callback params
  for (const ix of descendantIxs) {
    const propName = eventPropName(ix.emit_event);
    if (!functionParams.includes(propName)) {
      functionParams.push(propName);
    }
  }
  // View data param for leaf components
  if (primaryView && isLeaf && !functionParams.includes(viewPropName!)) {
    functionParams.push(viewPropName!);
  }
  if (needsChildren) {
    functionParams.push("children");
  }

  // Props interface (duplicate from propLines, add children if needed)
  let ifaceBody = propLines.join("\n");
  if (ifaceBody) ifaceBody += "\n";
  ifaceBody += `  children?: React.ReactNode;`;
  code += `interface ${name}Props {\n${ifaceBody}\n}\n\n`;

  const params = `{ ${functionParams.join(", ")} }`;
  code += `export function ${name}(${params}: ${name}Props) {\n`;

  // Build DOM attribute lines — onClick handlers + non-standard wrapper comments
  const attrLines: string[] = [];
  for (const ix of region.interactions) {
    const propName = eventPropName(ix.emit_event);
    const payloadStr = eventDefaultPayload(ix.emit_event, model, viewPropName ?? undefined, primaryView ?? undefined);
    if (ix.handler_name === "onClick") {
      attrLines.push(`        onClick={() => ${propName}?.(${payloadStr} as unknown as ${ix.emit_event})}`);
    }
  }

  // Non-standard handlers: generate TODO comment inside JSX body, not as duplicate onClick
  const nonStandardHandlers = region.interactions.filter(
    (ix) => ix.handler_name !== "onClick"
  );
  let handlerComment = "";
  if (nonStandardHandlers.length > 0) {
    const names = nonStandardHandlers.map((ix) => `${ix.handler_name}(${ix.emit_event})`).join(", ");
    handlerComment = `\n      {/* TODO: wire ${names} — add onClick/onChange/etc to trigger these events */}`;
  }

  // ── Repeat region ──
  if (hasRepeat) {
    const { region: repeatRegion, item } = region.repeat;
    const repeatComp = model.regions.find((r) => r.name === repeatRegion);
    const repeatInteractions = repeatComp?.interactions ?? [];

    const seenEvents = new Set<string>();
    const uniqueRepeatInteractions = repeatInteractions.filter((ix) => {
      if (seenEvents.has(ix.emit_event)) return false;
      seenEvents.add(ix.emit_event);
      return true;
    });

    const repeatProps = uniqueRepeatInteractions
      .map((ix) => {
        const propName = eventPropName(ix.emit_event);
        return `              ${propName}={${propName}}`;
      })
      .join("\n");

    code += `  return (\n`;
    code += `    <div className={styles.container}`;
    if (attrLines.length > 0) {
      code += `\n${attrLines.join("\n")}`;
    }
    code += `>\n`;
    code += `      {(items ?? []).map((${item}: ${repeatItemType}) => (\n`;
    if (repeatProps) {
      code += `        <${repeatRegion} key={${item}.id}\n${repeatProps}\n        />\n`;
    } else {
      code += `        <${repeatRegion} key={${item}.id} />\n`;
    }
    code += `      ))}\n`;
    if (needsChildren) {
      code += `      {children}\n`;
    }
    code += `    </div>\n`;
    code += `  );\n`;
    code += `}\n`;
    return code;
  }

  // ── Build props string for a child component (its descendant callbacks) ──
  function childForwardProps(childRegionName: string): string {
    const childR = model.regions.find((r) => r.name === childRegionName);
    if (!childR) return "";
    // Only forward callbacks that the child doesn't already accept directly
    const childOwnEvents = new Set((childR.interactions ?? []).map((ix) => ix.emit_event));
    if (childR.repeat) {
      const childRepeatR = model.regions.find((r) => r.name === childR.repeat!.region);
      if (childRepeatR) {
        for (const ix of childRepeatR.interactions) {
          childOwnEvents.add(ix.emit_event);
        }
      }
    }
    const childDescIxs = collectDescendantInteractions(
      { name: childRegionName, interactions: childR.interactions, contains: childR.contains, repeat: childR.repeat } as TsRegion,
      model,
    );
    // Only forward descendant interactions that the child doesn't already own, deduplicated
    const seen = new Set<string>();
    const props: string[] = [];
    for (const ix of childDescIxs) {
      if (childOwnEvents.has(ix.emit_event)) continue;
      const propName = eventPropName(ix.emit_event);
      if (seen.has(propName)) continue;
      seen.add(propName);
      props.push(` ${propName}={${propName}}`);
    }
    return props.join("");
  }

  // ── Children region ──
  if (hasChildren) {
    const pageHeader = isRouteContainer
      ? `      <header className={styles.pageHeader}>
        <h1 className={styles.pageTitle}>${pageTitle(name)}</h1>
        <p className={styles.pageSummary}>${pageSummary(name)}</p>
      </header>\n`
      : "";
    let pageHeaderRendered = false;
    code += `  return (\n`;
    code += `    <div className={styles.container}`;
    if (attrLines.length > 0) {
      code += `\n${attrLines.join("\n")}`;
    }
    code += `>\n`;
    if (handlerComment) {
      code += handlerComment + "\n";
    }
    if (pageHeader && !region.contains.some((c) => c.region.endsWith("TopNavigation"))) {
      code += pageHeader;
      pageHeaderRendered = true;
    }
    for (const c of region.contains) {
      const fwdProps = childForwardProps(c.region);
      if (c.slot_fills.length > 0) {
        code += `      <${c.region}${fwdProps}>\n`;
        for (const sf of c.slot_fills) {
          const compName = sf.component;
          if (compName.startsWith('"')) {
            code += `        <span>{${compName}}</span>\n`;
          } else {
            code += `        <${compName} />\n`;
          }
        }
        code += `      </${c.region}>\n`;
      } else if (c.region === "ContentArea" && hasContentArea) {
        code += `      <${c.region}${fwdProps}>{children}</${c.region}>\n`;
      } else {
        code += `      <${c.region}${fwdProps} />\n`;
      }
      if (pageHeader && !pageHeaderRendered && c.region.endsWith("TopNavigation")) {
        code += pageHeader;
        pageHeaderRendered = true;
      }
    }
    code += `    </div>\n`;
    code += `  );\n`;
  } else if (hasInteractions || isLeaf) {
    code += `  return (\n`;
    code += `    <div className={styles.container}`;
    if (attrLines.length > 0) {
      code += `\n${attrLines.join("\n")}`;
    }
    code += `>\n`;
    if (handlerComment) {
      code += handlerComment + "\n";
    }
    if (isLeaf) {
      code += defaultLeafContent(name);
    }
    if (needsChildren) {
      code += `      {children}\n`;
    }
    code += `    </div>\n`;
    code += `  );\n`;
  } else {
    code += `  return (\n`;
    code += `    <div className={styles.container}>{children}</div>\n`;
    code += `  );\n`;
  }

  code += `}\n`;
  return code;
}

function generateCssModule(region: TsRegion): string {
  if (region.name.endsWith("TopNavigation")) {
    return `.container {
  display: flex;
  flex-direction: row;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  min-height: 56px;
  padding: 0 24px;
  border-bottom: 1px solid #d8dee8;
  background: #ffffff;
  color: #172033;
}

.brand {
  font-size: 15px;
  font-weight: 700;
  white-space: nowrap;
}

.links {
  display: flex;
  align-items: center;
  gap: 4px;
}

.link {
  min-width: 82px;
  padding: 7px 12px;
  border-radius: 6px;
  color: #526073;
  font-size: 14px;
  font-weight: 600;
  text-align: center;
  text-decoration: none;
}

.link:hover {
  background: #eef3f8;
  color: #1d2a3f;
}

.active {
  background: #1f6feb;
  color: #ffffff;
}
`;
  }

  const styles: Record<string, string> = {};
  const isLeaf = region.contains.length === 0 && !region.repeat;
  const isPage = isRouteRegion(region);

  if (region.orientation) {
    styles.display = "flex";
    switch (region.orientation) {
      case "vertical":
        styles.flexDirection = "column";
        break;
      case "horizontal":
        styles.flexDirection = "row";
        styles.alignItems = "center";
        styles.gap = "8px";
        break;
      case "grid":
        styles.flexDirection = "row";
        styles.flexWrap = "wrap";
        styles.gap = "8px";
        break;
    }
  }

  for (const lp of region.layout_props) {
    switch (lp) {
      case "scrollable":
        styles.overflowY = "auto";
        break;
      case "fillRemaining":
        styles.flex = "1";
        break;
      case "fixedBottom":
        styles.position = "fixed";
        styles.bottom = "0";
        styles.zIndex = "10";
        break;
    }
  }

  if (Object.keys(styles).length === 0) {
    styles.display = "flex";
  }

  if (isLeaf) {
    styles.minHeight = "44px";
    styles.padding = "10px 12px";
    styles.border = "1px solid #d8dee8";
    styles.borderRadius = "6px";
    styles.background = "#ffffff";
    styles.color = "#172033";
    styles.gap = "8px";
  }

  if (isPage) {
    styles.minHeight = "100%";
    styles.background = "#f5f7fb";
    styles.gap = "20px";
    styles.paddingBottom = "32px";
  }

  if (
    region.name.endsWith("StatusOverviewMetrics") ||
    region.name.endsWith("InstallCodeList") ||
    region.name.endsWith("ControlUsecaseBoard") ||
    region.name.endsWith("AbnormalAgentPanel") ||
    region.name.endsWith("AdminOperatorLane")
  ) {
    styles.width = "100%";
    styles.maxWidth = "1180px";
    styles.margin = "0 auto";
    styles.padding = "0 24px";
    styles.gap = "16px";
  }

  if (region.name.endsWith("InstallCodeList") || region.name.endsWith("ControlUsecaseBoard")) {
    styles.display = "grid";
    styles.gridTemplateColumns = "repeat(auto-fit, minmax(320px, 1fr))";
    styles.alignItems = "stretch";
  }

  if (region.name.endsWith("UsecaseCard") || region.name.endsWith("AgentInstallCode")) {
    styles.padding = "18px";
    styles.border = "1px solid #d8dee8";
    styles.borderRadius = "8px";
    styles.background = "#ffffff";
    styles.boxShadow = "0 1px 2px rgba(20, 32, 51, 0.06)";
    styles.gap = "12px";
  }

  const cssProps = Object.entries(styles)
    .map(([k, v]) => {
      const cssKey = k.replace(/([A-Z])/g, "-$1").toLowerCase();
      return `  ${cssKey}: ${v};`;
    })
    .join("\n");

  let css = `.container {\n${cssProps}\n}\n`;
  if (isPage) {
    css += `
.pageHeader {
  width: 100%;
  max-width: 1180px;
  margin: 0 auto;
  padding: 8px 24px 0;
}

.pageTitle {
  color: #172033;
  font-size: 24px;
  font-weight: 750;
  line-height: 1.25;
}

.pageSummary {
  max-width: 760px;
  margin-top: 6px;
  color: #5f6f86;
  font-size: 14px;
  line-height: 1.55;
}
`;
  }
  if (isLeaf) {
    css += `
.label {
  font-size: 13px;
  font-weight: 700;
}

.code {
  display: block;
  max-width: 100%;
  padding: 8px;
  border-radius: 4px;
  overflow-x: auto;
  background: #f3f6f9;
  color: #26364d;
  font-size: 12px;
}

.copyButton {
  align-self: flex-start;
  padding: 6px 10px;
  border: 1px solid #b9c5d6;
  border-radius: 4px;
  background: #ffffff;
  color: #1f3a5f;
  font-size: 12px;
  font-weight: 700;
  cursor: pointer;
}
`;
  }

  return css;
}

function generatePlaceholderComponent(name: string): string {
  return `import styles from "./${name}.module.css";

interface ${name}Props {
  children?: React.ReactNode;
}

export function ${name}({ children }: ${name}Props) {
  return (
    <div className={styles.container}>
${defaultLeafContent(name)}      {children}
    </div>
  );
}
`;
}

function generatePlaceholderCss(): string {
  return `.container {
  display: flex;
  flex-direction: column;
  min-height: 44px;
  padding: 10px 12px;
  border: 1px solid #d8dee8;
  border-radius: 6px;
  background: #ffffff;
  color: #172033;
  gap: 8px;
}

.label {
  font-size: 13px;
  font-weight: 700;
}

.code {
  display: block;
  max-width: 100%;
  padding: 8px;
  border-radius: 4px;
  overflow-x: auto;
  background: #f3f6f9;
  color: #26364d;
  font-size: 12px;
}

.copyButton {
  align-self: flex-start;
  padding: 6px 10px;
  border: 1px solid #b9c5d6;
  border-radius: 4px;
  background: #ffffff;
  color: #1f3a5f;
  font-size: 12px;
  font-weight: 700;
  cursor: pointer;
}
`;
}

export function generateComponents(
  model: TsModel,
  writeFile: WriteFile,
  ensureDir: EnsureDir,
): void {
  const dir = ensureDir("src", "components");

  const regionNames = new Set(model.regions.map((r) => r.name));
  const placeholderNames = new Set<string>();
  for (const region of model.regions) {
    for (const c of region.contains) {
      for (const sf of c.slot_fills) {
        if (!sf.component.startsWith('"') && !regionNames.has(sf.component)) {
          placeholderNames.add(sf.component);
        }
      }
    }
  }

  for (const name of placeholderNames) {
    writeFile(dir, `${name}.tsx`, generatePlaceholderComponent(name));
    writeFile(dir, `${name}.module.css`, generatePlaceholderCss());
  }

  const names: string[] = [];

  for (const region of model.regions) {
    const name = region.name;
    names.push(name);
    writeFile(dir, `${name}.tsx`, buildComponentCode(region, model));
    writeFile(dir, `${name}.module.css`, generateCssModule(region));
  }

  const allNames = [...names, ...placeholderNames];
  writeFile(
    dir,
    "index.ts",
    allNames.map((n) => `export { ${n} } from "./${n}";`).join("\n") + "\n",
  );
}
