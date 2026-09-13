import { useMemo, useState } from "react";
import styles from "./SubsystemPipelinePage.module.css";
import { MetricTile } from "./MetricTile";
import { RateLimitNotice } from "./RateLimitNotice";
import { TrendChart } from "./TrendChart";
import { isRateLimitedError } from "../api";
import type { PipelineGroup, PipelineNode } from "../api";
import { usePipelineTopology } from "../hooks";
import {
  formatAxis,
  formatClock,
  formatCount,
  formatRate,
  formatRelativeSeconds,
  formatShare,
} from "../lib/format";

type LayerKey = "sources" | "parses" | "sinks";

interface Selection {
  id: string;
  label: string;
  layer: LayerKey;
  series: [number, number][];
  totalSeries: [number, number][];
}

/**
 * 把多条序列按时间戳逐点求和。
 * 同一层的序列来自同一次 `query_range`，时间戳天然对齐；仍以时间戳为键累加，
 * 不依赖下标对齐。同阶段内跨节点求和是合法口径（Σ各来源 = 接入吞吐）。
 */
function sumPoints(seriesList: [number, number][][]): [number, number][] {
  const totals = new Map<number, number>();
  for (const points of seriesList) {
    for (const [timestamp, value] of points) {
      totals.set(timestamp, (totals.get(timestamp) ?? 0) + value);
    }
  }
  return [...totals.entries()].sort((a, b) => a[0] - b[0]);
}

/** 窗口内是否一条非零采样都没有（静默项），免得把平线误读成图表坏了。 */
function isAllZero(target: Selection): boolean {
  return (
    target.series.every((point) => point[1] === 0) &&
    target.totalSeries.every((point) => point[1] === 0)
  );
}

/** 可选时间窗。步长固定 1 分钟 —— 数据面写入 VM 的粒度实测就是 60 秒。 */
const WINDOWS = [
  { seconds: 900, label: "近 15 分钟" },
  { seconds: 1800, label: "近 30 分钟" },
  { seconds: 3600, label: "近 1 小时" },
  { seconds: 21600, label: "近 6 小时" },
];

export function SubsystemPipelinePage() {
  const [windowSeconds, setWindowSeconds] = useState(1800);
  const [selected, setSelected] = useState<Selection | null>(null);
  const [collapsedGroups, setCollapsedGroups] = useState<Set<string>>(new Set());

  const { data, isLoading, isError, error } = usePipelineTopology(windowSeconds);

  const summary = data?.summary;
  const lossTone = (summary?.lossRate ?? 0) > 0.1 ? "crit" : "ok";

  // 数据面写入 VM 的粒度是 60 秒。超过 3 个周期没有新采样点，就基本可以断定
  // 是采集通道断了，而不是真的「没有流量」——两者在页面上都表现为空白，
  // 不看采样时效就分不清，会让人以为规则/来源没配好。
  // 选中的节点在窗口内是否一条非零采样都没有（静默节点），
  // 免得把「平线」误读成图表坏了。
  const selectedHasNoData = selected !== null && isAllZero(selected);

  // 未选中任何条目时，「接入与输出」节内那张图默认画**来源汇总**
  // （所有来源的合计速率与合计累计量）—— 空着不如给一个有意义的总量。
  const sourceAggregate = useMemo<Selection | null>(() => {
    const sources = data?.sources ?? [];
    if (sources.length === 0) return null;
    return {
      id: "aggregate:sources",
      label: "来源汇总",
      layer: "sources",
      series: sumPoints(sources.map((node) => node.series)),
      totalSeries: sumPoints(sources.map((node) => node.totalSeries)),
    };
  }, [data]);

  const sectionTarget = selected ?? sourceAggregate;

  /** 再点一次已选中的条目 = 取消选择，回到默认的来源汇总。 */
  const handleSelect = (next: Selection) => {
    setSelected((prev) => (prev && prev.id === next.id ? null : next));
  };

  const sampleAgeSeconds =
    data && data.latestSampleAt !== undefined
      ? Math.max(0, data.generatedAt - data.latestSampleAt)
      : null;
  const isStale = sampleAgeSeconds !== null && sampleAgeSeconds > 180;
  const hasNoSample = data !== undefined && data.latestSampleAt === undefined;

  function toggleGroup(id: string) {
    setCollapsedGroups((previous) => {
      const next = new Set(previous);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  function setAllCollapsed(groups: PipelineGroup[], collapsed: boolean) {
    setCollapsedGroups((previous) => {
      const next = new Set(previous);
      for (const group of groups) {
        if (collapsed) next.add(group.id);
        else next.delete(group.id);
      }
      return next;
    });
  }

  return (
    <div className={styles.container}>
      <header className={styles.pageHeader}>
        <h1 className={styles.pageTitle}>数据采集</h1>
        <p className={styles.pageSummary}>
          数据面（wparse）三层的实时吞吐：来源接入 → 规则解析 → 落存储。
          点击任意节点，下方会画出它的速率与累计量曲线。
        </p>
        <p className={styles.pageNote}>
          聚合粒度 1 分钟 —— 数据面写入 VictoriaMetrics 的间隔实测为 60 秒，
          指标按 2 个周期（120 秒）计算速率。所以这里是「近一分钟的平均吞吐」，不是逐秒瞬时值。
        </p>
      </header>

      {isStale || hasNoSample ? (
        <div className={styles.staleBanner}>
          <strong>指标通道已停止上报</strong>
          <span>
            {hasNoSample
              ? "所选时间窗内没有任何采样点，页面内容不代表当前状态。"
              : `最后采样 ${formatClock(data?.latestSampleAt)}（${formatRelativeSeconds(sampleAgeSeconds).text}前），页面内容已不代表当前状态。`}
            {" "}这通常意味着数据面的 VictoriaMetrics 推送失败（数据面日志关键字{" "}
            <code>VictoriaMetric periodic push failed</code>），重启数据面即可恢复；
            通道正常时是看不出解析规则是空的。
          </span>
        </div>
      ) : null}

      {isError ? (
        isRateLimitedError(error) ? (
          <RateLimitNotice error={error} />
        ) : (
          <div className={styles.errorBanner}>
            无法获取采集指标。请确认数据面（wparse）与 VictoriaMetrics 都在运行，
            并在左下角设置 Admin Token。
          </div>
        )
      ) : null}

      <section className={styles.summary}>
        <MetricTile
          label="入流速率"
          value={formatRate(summary?.ingressRate)}
          hint="所有来源合计；这是采集实际收到的量"
          tone="accent"
          loading={isLoading}
        />
        <MetricTile
          label="解析速率"
          value={formatRate(summary?.parseRate)}
          hint="进入解析规则的速率（含未通过的行）"
          tone="unknown"
          loading={isLoading}
        />
        <MetricTile
          label="落存储速率"
          value={formatRate(summary?.egressRate)}
          hint="各输出口写入速率合计；同一分组写多个目标会重复计入"
          tone="ok"
          loading={isLoading}
        />
        <MetricTile
          label="未落存储"
          value={formatRate(summary?.lossRate)}
          hint={
            summary
              ? `占入流 ${formatShare(summary.lossRate, summary.ingressRate)}，期望 ≤0.1 e/s`
              : "miss / residue / error 三路合计"
          }
          tone={lossTone}
          loading={isLoading}
        />
      </section>

      {isLoading ? (
        <div className={styles.skeletonWrap}>
          <div className={styles.skeletonPair}>
            <div className={styles.skeletonColumn} />
            <div className={styles.skeletonColumn} />
          </div>
          <div className={styles.skeletonColumn} />
        </div>
      ) : (
        <>
          <section className={styles.section}>
            <header className={styles.sectionHead}>
              <h2 className={styles.sectionTitle}>接入与输出</h2>
              <p className={styles.sectionNote}>
                来源接入与落盘出口；两者数量通常稳定。
              </p>
            </header>
            <div className={`${styles.board} ${styles.boardPair}`}>
              <PipelineLayer
                layer="sources"
                title="来源层"
                accent="source"
                items={data?.sources ?? []}
                canCollapse={false}
                emptyText="没有来源在推数据"
                renderItem={(node, silent) => (
                  <NodeRow
                    key={node.id}
                    node={node}
                    layer="sources"
                    silent={silent}
                    selected={selected?.id === `sources:${node.id}`}
                    onSelect={handleSelect}
                  />
                )}
              />

              <PipelineLayer
                layer="sinks"
                title="输出层"
                accent="sink"
                items={data?.sinks ?? []}
                canCollapse
                emptyText="没有输出分组在接收数据"
                onExpandAll={() => setAllCollapsed(data?.sinks ?? [], false)}
                onCollapseAll={() => setAllCollapsed(data?.sinks ?? [], true)}
                renderItem={(group, silent) => (
                  <GroupCard
                    key={group.id}
                    group={group}
                    layer="sinks"
                    unitLabel="个出口"
                    silent={silent}
                    expanded={!collapsedGroups.has(group.id)}
                    onToggle={() => toggleGroup(group.id)}
                    selected={selected}
                    onSelect={handleSelect}
                  />
                )}
              />
            </div>
            <div className={styles.sectionChart}>
              <div className={styles.chartHead}>
                <span className={styles.chartTitle}>
                  {selected ? "选中项" : "来源汇总"}
                </span>
                <span className={styles.chartNote}>
                  {selected
                    ? `${selected.label} · 速率与累计量`
                    : "所有来源的合计；点上方任意条目即可切成单项（再点一次回到汇总）"}
                </span>
              </div>
              <SelectionCharts
                selected={sectionTarget}
                windowSeconds={windowSeconds}
                emptyText="暂无来源数据。"
              />
            </div>
          </section>

          <section className={styles.section}>
            <header className={styles.sectionHead}>
              <h2 className={styles.sectionTitle}>解析规则</h2>
              <p className={styles.sectionNote}>
                按可用宽度自动分栏；规则多时在节内滚动。
              </p>
            </header>
            <div className={`${styles.board} ${styles.boardSingle}`}>
              <PipelineLayer
                layer="parses"
                title="PARSE 层"
                accent="parse"
                items={data?.parses ?? []}
                canCollapse
                emptyText="没有规则在解析数据"
                onExpandAll={() => setAllCollapsed(data?.parses ?? [], false)}
                onCollapseAll={() => setAllCollapsed(data?.parses ?? [], true)}
                renderItem={(group, silent) => (
                  <GroupCard
                    key={group.id}
                    group={group}
                    layer="parses"
                    unitLabel="条规则"
                    silent={silent}
                    expanded={!collapsedGroups.has(group.id)}
                    onToggle={() => toggleGroup(group.id)}
                    selected={selected}
                    onSelect={handleSelect}
                  />
                )}
              />
            </div>
          </section>
        </>
      )}

      <section className={styles.detail}>
        <div className={styles.detailHead}>
          <div className={styles.detailTitleGroup}>
            <h2 className={styles.detailTitle}>节点详情</h2>
            <span className={styles.detailTarget}>
              {selected ? selected.label : "未选择节点 —— 点上方任意节点查看曲线"}
            </span>
            {selectedHasNoData ? (
              <span className={styles.detailIdleNote}>窗口内无数据（全程 0）</span>
            ) : null}
          </div>
          <div className={styles.detailControls}>
            <select
              className={styles.windowSelect}
              value={windowSeconds}
              onChange={(event) => setWindowSeconds(Number(event.target.value))}
              aria-label="时间窗"
            >
              {WINDOWS.map((item) => (
                <option key={item.seconds} value={item.seconds}>
                  {item.label}
                </option>
              ))}
            </select>
          </div>
        </div>

        <div className={styles.detailBody}>
          <SelectionCharts
            selected={selected}
            windowSeconds={windowSeconds}
            emptyText={
              data?.parses.length || data?.sinks.length
                ? "选择任意来源 / 规则 / 出口，即可查看它的速率与累计量曲线。"
                : "暂无节点可选。"
            }
          />
        </div>
      </section>
    </div>
  );
}

function LayerColumn({
  title,
  accent,
  count,
  canCollapse,
  onExpandAll,
  onCollapseAll,
  tabs,
  children,
}: {
  layer: LayerKey;
  title: string;
  accent: "source" | "parse" | "sink";
  count: number;
  canCollapse: boolean;
  onExpandAll: () => void;
  onCollapseAll: () => void;
  /** 列头与列表之间的固定区域（活跃 / 静默 tab），不随列表滚动。 */
  tabs?: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <div className={`${styles.column} ${styles[`column_${accent}`]}`}>
      <div className={styles.columnHead}>
        <span className={styles.columnDot} aria-hidden="true" />
        <h2 className={styles.columnTitle}>{title}</h2>
        <span className={styles.columnCount}>{count}</span>
        {canCollapse ? (
          <div className={styles.columnActions}>
            <button type="button" className={styles.linkButton} onClick={onExpandAll}>
              全部展开
            </button>
            <button type="button" className={styles.linkButton} onClick={onCollapseAll}>
              全部收起
            </button>
          </div>
        ) : null}
      </div>
      {tabs}
      <div className={styles.columnBody}>{children}</div>
    </div>
  );
}

function GroupCard({
  group,
  layer,
  unitLabel,
  silent,
  expanded,
  onToggle,
  selected,
  onSelect,
}: {
  group: PipelineGroup;
  layer: LayerKey;
  unitLabel: string;
  silent: boolean;
  expanded: boolean;
  onToggle: () => void;
  selected: Selection | null;
  onSelect: (selection: Selection) => void;
}) {
  const isLoss = group.kind === "loss";
  // 只有真的在丢数据时才标红；0 值的损失分组属于健康状态。
  const isLossActive = isLoss && group.rate > 0;
  const selectionId = `${layer}:${group.id}`;
  const isSelected = selected?.id === selectionId;
  const classes = [styles.group];
  if (isLoss) classes.push(styles.groupLoss);
  if (isLossActive) classes.push(styles.groupLossActive);
  if (silent) classes.push(styles.silentItem);

  return (
    <div className={classes.join(" ")}>
      <button
        type="button"
        className={styles.groupHead}
        onClick={onToggle}
        aria-expanded={expanded}
      >
        <span className={expanded ? styles.caretOpen : styles.caret} aria-hidden="true">
          ▸
        </span>
        <span className={styles.groupName}>{group.label}</span>
        {isLoss ? (
          <span className={isLossActive ? styles.lossBadge : styles.lossBadgeIdle}>
            未落存储
          </span>
        ) : null}
      </button>
      <div className={styles.groupStats}>
        {silent ? <IdleBadge series={group.series} /> : null}
        <span className={styles.rate}>{formatRate(group.rate)}</span>
        <span className={styles.total}>{formatCount(group.total)}</span>
        <span className={styles.childCount}>
          {group.children.length} {unitLabel}
        </span>
        <button
          type="button"
          className={isSelected ? styles.plotButtonActive : styles.plotButton}
          onClick={() => {
            onSelect({
              id: selectionId,
              label: group.label,
              layer,
              series: group.series,
              totalSeries: group.totalSeries,
            });
          }}
        >
          曲线
        </button>
      </div>

      {expanded ? (
        <div className={styles.children}>
          {group.children.map((node) => (
            <NodeRow
              key={node.id}
              node={node}
              layer={layer}
              parentId={group.id}
              selected={selected?.id === `${layer}:${node.id}`}
              onSelect={onSelect}
            />
          ))}
        </div>
      ) : null}
    </div>
  );
}

function NodeRow({
  node,
  layer,
  parentId,
  silent,
  selected,
  onSelect,
}: {
  node: PipelineNode;
  layer: LayerKey;
  parentId?: string;
  silent?: boolean;
  selected: boolean;
  onSelect: (selection: Selection) => void;
}) {
  const classes = [styles.node];
  if (selected) classes.push(styles.nodeSelected);
  if (silent) classes.push(styles.silentItem);

  return (
    <div className={classes.join(" ")}>
      <button
        type="button"
        className={styles.nodeBody}
        onClick={() =>
          onSelect({
            id: `${layer}:${node.id}`,
            label: parentId ? `${parentId} / ${node.label}` : node.label,
            layer,
            series: node.series,
            totalSeries: node.totalSeries,
          })
        }
      >
        <span className={styles.nodeName}>{node.label}</span>
        <span className={styles.nodeStats}>
          {silent ? <IdleBadge series={node.series} /> : null}
          <span className={styles.rate}>{formatRate(node.rate)}</span>
          <span className={styles.total}>{formatCount(node.total)}</span>
        </span>
      </button>
      <span className={styles.srOnly}>{`${layer}:${node.id}`}</span>
    </div>
  );
}

/**
 * 选中项的速率 / 累计量两图（「接入与输出」节与底部「节点详情」共用）。
 *
 * 速率与累计量是两种读法（一个看当下、一个看总量），用分段控件切换会把另一半藏掉，
 * 所以固定并排各占一格。
 */
function SelectionCharts({
  selected,
  windowSeconds,
  emptyText,
}: {
  selected: Selection | null;
  windowSeconds: number;
  emptyText: string;
}) {
  if (!selected) {
    return <div className={styles.detailEmpty}>{emptyText}</div>;
  }
  return (
    <div className={styles.detailCharts}>
      <ChartPanel
        chartKey={`${selected.id}:rate:${windowSeconds}`}
        title="速率"
        note="近一分钟平均吞吐"
        name={selected.label}
        color="var(--series-1)"
        points={selected.series}
        valueFormatter={formatRate}
      />
      <ChartPanel
        chartKey={`${selected.id}:total:${windowSeconds}`}
        title="累计量"
        note="进程启动至今累计"
        name={selected.label}
        color="var(--series-2)"
        points={selected.totalSeries}
        valueFormatter={formatCount}
      />
    </div>
  );
}

/**
 * 详情区的一格图：标题 + 一行口径说明 + 曲线。
 * 速率与累计量各占一格，不再用分段控件切换（切换会把另一半藏起来）。
 */
function ChartPanel({
  chartKey,
  title,
  note,
  name,
  color,
  points,
  valueFormatter,
}: {
  chartKey: string;
  title: string;
  note: string;
  name: string;
  color: string;
  points: [number, number][];
  valueFormatter: (value: number) => string;
}) {
  return (
    <div className={styles.chartPanel}>
      <div className={styles.chartHead}>
        <span className={styles.chartTitle}>{title}</span>
        <span className={styles.chartNote}>{note}</span>
      </div>
      <TrendChart
        key={chartKey}
        series={[{ name, color, points }]}
        height={180}
        axisWidth={64}
        valueFormatter={valueFormatter}
        axisFormatter={formatAxis}
      />
    </div>
  );
}

function EmptyLayer({ text }: { text: string }) {
  return <div className={styles.empty}>{text}</div>;
}

/**
 * 速率 > 0 视为活跃；否则为静默（已配置但当前没有流量）。
 * 静默不等于故障 —— miss/residue/error 静默反而是健康状态 —— 但它一定值得一眼看见，
 * 所以不做隐藏，只降一级视觉权重并单独成段。
 */
function isActiveRate(rate: number): boolean {
  return rate > 0;
}

/** 序列里最后一个非零采样点（毫秒）；全零或空序列返回 null。 */
function lastActiveMs(series: [number, number][]): number | null {
  for (let index = series.length - 1; index >= 0; index -= 1) {
    if (series[index][1] > 0) return series[index][0];
  }
  return null;
}

/** 静默项上标明「什么时候还有过流量」，用来区分「刚停」和「从来没动过」。 */
function IdleBadge({ series }: { series: [number, number][] }) {
  const lastMs = lastActiveMs(series);
  return (
    <span className={styles.idleBadge}>
      {lastMs === null
        ? "窗口内无数据"
        : `最后活跃 ${formatClock(lastMs / 1000).slice(0, 5)}`}
    </span>
  );
}

type ActivityKey = "active" | "silent";

/** 速率 > 0 视为活跃；静默 = 已配置但当前窗口没有流量。 */
function splitByActivity<T extends { rate: number }>(items: T[]) {
  return {
    active: items.filter((item) => isActiveRate(item.rate)),
    silent: items.filter((item) => !isActiveRate(item.rate)),
  };
}

/**
 * 活跃 / 静默 切换（tab 形态，一次只显示一组）。
 * 数量为 0 的一侧保留但禁用 —— 「静默 0」本身就是一条有用的健康信息，
 * 直接不渲染会让这个控件在层与层之间长得不一样。
 */
function ActivityTabs({
  activeCount,
  silentCount,
  value,
  onChange,
}: {
  activeCount: number;
  silentCount: number;
  value: ActivityKey;
  onChange: (value: ActivityKey) => void;
}) {
  const entries: { key: ActivityKey; label: string; count: number }[] = [
    { key: "active", label: "活跃", count: activeCount },
    { key: "silent", label: "静默", count: silentCount },
  ];
  return (
    <div className={styles.activityTabs} role="tablist" aria-label="活跃 / 静默">
      {entries.map((entry) => {
        const disabled = entry.count === 0;
        const selected = !disabled && entry.key === value;
        return (
          <button
            key={entry.key}
            type="button"
            role="tab"
            aria-selected={selected}
            disabled={disabled}
            className={selected ? styles.activityTabActive : styles.activityTab}
            onClick={() => onChange(entry.key)}
          >
            {entry.label}
            <span className={styles.activityTabCount}>{entry.count}</span>
          </button>
        );
      })}
    </div>
  );
}

/**
 * 一层（来源 / PARSE / 输出）：列头 + 活跃静默 tab + 条目列表。
 * tab 状态按层独立保存，层与层之间互不影响。
 */
function PipelineLayer<T extends { id: string; rate: number; series: [number, number][] }>({
  layer,
  title,
  accent,
  items,
  canCollapse,
  emptyText,
  onExpandAll,
  onCollapseAll,
  renderItem,
}: {
  layer: LayerKey;
  title: string;
  accent: "source" | "parse" | "sink";
  items: T[];
  canCollapse: boolean;
  emptyText: string;
  onExpandAll?: () => void;
  onCollapseAll?: () => void;
  renderItem: (item: T, silent: boolean) => React.ReactNode;
}) {
  const [tab, setTab] = useState<ActivityKey>("active");
  const { active, silent } = splitByActivity(items);
  // 该层没有活跃项时（例如全部静默）自动落到静默页，避免开出一个空白列表。
  const effectiveTab: ActivityKey = active.length === 0 && silent.length > 0 ? "silent" : tab;
  const visible = effectiveTab === "active" ? active : silent;

  return (
    <LayerColumn
      layer={layer}
      title={title}
      accent={accent}
      count={items.length}
      canCollapse={canCollapse}
      onExpandAll={onExpandAll ?? (() => undefined)}
      onCollapseAll={onCollapseAll ?? (() => undefined)}
      tabs={
        items.length > 0 ? (
          <ActivityTabs
            activeCount={active.length}
            silentCount={silent.length}
            value={effectiveTab}
            onChange={setTab}
          />
        ) : null
      }
    >
      {visible.map((item) => renderItem(item, effectiveTab === "silent"))}
      {items.length === 0 ? <EmptyLayer text={emptyText} /> : null}
    </LayerColumn>
  );
}
