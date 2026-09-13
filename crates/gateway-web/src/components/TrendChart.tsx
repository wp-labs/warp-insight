import { useEffect, useMemo, useRef, useState } from "react";
import styles from "./TrendChart.module.css";

export interface TrendChartSeries {
  name: string;
  color: string;
  points: [number, number][];
}

interface TrendChartProps {
  series: TrendChartSeries[];
  height?: number;
  valueFormatter?: (value: number) => string;
  /** 单序列时填充面积，多序列只描线，避免互相遮挡。 */
  filled?: boolean;
  /** 小于该宽度时隐藏 Y 轴刻度，给折线让出空间。 */
  compactBelow?: number;
}

const AXIS_LEFT = 46;
const AXIS_RIGHT = 10;
const AXIS_TOP = 10;
const AXIS_BOTTOM = 24;

function formatTime(ts: number, withDate: boolean): string {
  return new Intl.DateTimeFormat("zh-CN", {
    ...(withDate ? { month: "2-digit", day: "2-digit" } : {}),
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  }).format(new Date(ts));
}

/** 把区间切成"好看"的刻度步长：1 / 2 / 2.5 / 5 × 10^n。 */
function niceStep(rawStep: number): number {
  if (rawStep <= 0) return 1;
  const exponent = Math.floor(Math.log10(rawStep));
  const magnitude = 10 ** exponent;
  const normalized = rawStep / magnitude;
  const factor =
    normalized <= 1 ? 1 : normalized <= 2 ? 2 : normalized <= 2.5 ? 2.5 : normalized <= 5 ? 5 : 10;
  return factor * magnitude;
}

interface Bubble {
  x: number;
  y: number;
  ts: number;
  time: string;
  rows: { name: string; color: string; text: string }[];
}

export function TrendChart({
  series,
  height = 190,
  valueFormatter = (value) => value.toFixed(2),
  filled,
  compactBelow = 460,
}: TrendChartProps) {
  const wrapRef = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(0);
  const [hoverX, setHoverX] = useState<number | null>(null);
  // 稳定的渐变 id：悬停重渲染时不必重建 <defs>。
  const gradientId = useRef(`trend-${Math.random().toString(36).slice(2, 9)}`);

  // 用实测像素宽度作为 viewBox 宽度：不做缩放，文字与命中判定都按 1:1 计算。
  useEffect(() => {
    const node = wrapRef.current;
    if (!node) return;
    const observer = new ResizeObserver(([entry]) => {
      setWidth(Math.round(entry.contentRect.width));
    });
    observer.observe(node);
    return () => observer.disconnect();
  }, []);

  const usable = series.filter((s) => s.points.length > 0);
  const allPoints = useMemo(
    () => usable.flatMap((s) => s.points),
    [series], // eslint-disable-line react-hooks/exhaustive-deps
  );

  const model = useMemo(() => {
    if (width < 80 || allPoints.length < 2) return null;

    const values = allPoints.map((p) => p[1]).filter(Number.isFinite);
    if (values.length < 2) return null;

    let minValue = Math.min(...values);
    let maxValue = Math.max(...values);
    if (minValue === maxValue) {
      const pad = Math.abs(minValue) * 0.1 || 1;
      minValue -= pad;
      maxValue += pad;
    }
    const span = maxValue - minValue;
    const step = niceStep(span / 4);
    const tickMin = Math.floor(minValue / step) * step;
    const tickMax = Math.ceil(maxValue / step) * step;

    const minTime = Math.min(...allPoints.map((p) => p[0]));
    const maxTime = Math.max(...allPoints.map((p) => p[0]));
    const timeSpan = maxTime - minTime || 1;
    const withDate = timeSpan > 24 * 3600 * 1000;

    const plotWidth = width - AXIS_LEFT - AXIS_RIGHT;
    const plotHeight = height - AXIS_TOP - AXIS_BOTTOM;

    const x = (ts: number) =>
      AXIS_LEFT + ((ts - minTime) / timeSpan) * plotWidth;
    const y = (value: number) =>
      AXIS_TOP + ((tickMax - value) / (tickMax - tickMin)) * plotHeight;

    // 刻度按 tickMin + i*step 生成，而不是反复累加：
    // 累加会让 98.6 + 3*0.1 变成 98.90000000000002，与相邻刻度在
    // 小数位截断后渲染成同一个标签。
    const tickCount = Math.max(
      1,
      Math.min(12, Math.round((tickMax - tickMin) / step)),
    );
    const ticks = Array.from({ length: tickCount + 1 }, (_, index) =>
      Number((tickMin + index * step).toFixed(6)),
    );

    // 刻度文案去重：量程很窄时（例如磁盘 98.6%→99.0%），调用方给的
    // 1 位小数格式会把相邻刻度渲染成同一串字符，这里按步长补足精度。
    const rawLabels = ticks.map((tick) => valueFormatter(tick));
    const hasDuplicate = new Set(rawLabels).size !== rawLabels.length;
    const adaptiveDecimals = Number.isInteger(step)
      ? 0
      : step >= 1
        ? 1
        : Math.min(4, Math.ceil(-Math.log10(step)) + 1);
    const tickLabel = hasDuplicate
      ? (tick: number) => tick.toFixed(adaptiveDecimals)
      : valueFormatter;

    const paths = usable.map((s) => {
      // 逐点比较相邻时间戳：连续区段内做折线，遇到时间断档就断线，
      // 避免把"采集缺失"画成一条骗人的斜坡。
      const segments: string[] = [];
      let current: string[] = [];
      let previousTs: number | null = null;
      const gapLimit = Math.max(timeSpan / 40, 60_000);
      const sorted = [...s.points].sort((a, b) => a[0] - b[0]);
      for (const [ts, value] of sorted) {
        if (!Number.isFinite(ts) || !Number.isFinite(value)) continue;
        if (previousTs !== null && ts - previousTs > gapLimit && current.length) {
          segments.push(current.join(" "));
          current = [];
        }
        current.push(`${x(ts).toFixed(1)},${y(value).toFixed(1)}`);
        previousTs = ts;
      }
      if (current.length) segments.push(current.join(" "));

      const last = sorted[sorted.length - 1];
      return {
        ...s,
        segments,
        areaPath:
          sorted.length >= 2
            ? `M ${x(sorted[0][0]).toFixed(1)},${(AXIS_TOP + plotHeight).toFixed(1)} ` +
              sorted
                .map(([ts, value]) => `L ${x(ts).toFixed(1)},${y(value).toFixed(1)}`)
                .join(" ") +
              ` L ${x(last[0]).toFixed(1)},${(AXIS_TOP + plotHeight).toFixed(1)} Z`
            : null,
      };
    });

    return {
      minTime,
      maxTime,
      withDate,
      plotWidth,
      plotHeight,
      x,
      y,
      ticks,
      tickLabel,
      paths,
    };
  }, [allPoints, height, usable, valueFormatter, width]);

  const bubble: Bubble | null = useMemo(() => {
    if (!model || hoverX === null) return null;
    const times = allPoints.map((p) => p[0]);
    const targetTime =
      model.minTime +
      ((hoverX - AXIS_LEFT) / model.plotWidth) *
        (model.maxTime - model.minTime);
    let nearest = times[0];
    for (const t of times) {
      if (Math.abs(t - targetTime) < Math.abs(nearest - targetTime)) nearest = t;
    }

    const rows = model.paths.flatMap((s) => {
      const point = s.points.find((p) => p[0] === nearest);
      return point
        ? [
            {
              name: s.name,
              color: s.color,
              text: valueFormatter(point[1]),
            },
          ]
        : [];
    });
    if (rows.length === 0) return null;

    const anchor = model.paths.flatMap((s) =>
      s.points.filter((p) => p[0] === nearest),
    )[0];

    return {
      x: model.x(nearest),
      y: anchor ? model.y(anchor[1]) : AXIS_TOP,
      ts: nearest,
      time: formatTime(nearest, model.withDate),
      rows,
    };
  }, [allPoints, hoverX, model, valueFormatter]);

  if (!model) {
    return (
      <div className={styles.chart} ref={wrapRef}>
        <div className={styles.empty}>暂无趋势数据</div>
      </div>
    );
  }

  const showAxis = width >= compactBelow;
  const plotBottom = AXIS_TOP + model.plotHeight;
  const bubbleLeft = bubble
    ? Math.min(Math.max(bubble.x + 12, 4), Math.max(width - 168, 4))
    : 0;

  return (
    <div className={styles.chart} ref={wrapRef}>
      <div className={styles.plot}>
        <svg
          className={styles.svg}
          width={width}
          height={height}
          viewBox={`0 0 ${width} ${height}`}
          role="img"
          aria-label="指标趋势"
        >
          <defs>
            <linearGradient id={gradientId.current} x1="0" y1="0" x2="0" y2="1">
              <stop
                offset="0%"
                stopColor={usable[0]?.color ?? "var(--series-1)"}
                stopOpacity="0.28"
              />
              <stop
                offset="100%"
                stopColor={usable[0]?.color ?? "var(--series-1)"}
                stopOpacity="0"
              />
            </linearGradient>
          </defs>

          {model.ticks.map((tick, tickIndex) => {
            const ty = model.y(tick);
            const label = model.tickLabel(tick);
            if (ty < AXIS_TOP - 1 || ty > plotBottom + 1) return null;
            // 兜底：标签与前一条重复时只保留网格线，避免轴上出现两行同值。
            const showLabel =
              showAxis &&
              (tickIndex === 0 ||
                label !== model.tickLabel(model.ticks[tickIndex - 1]));
            return (
              <g key={tick}>
                <line
                  x1={AXIS_LEFT}
                  y1={ty}
                  x2={width - AXIS_RIGHT}
                  y2={ty}
                  className={styles.gridLine}
                />
                {showLabel ? (
                  <text
                    x={AXIS_LEFT - 8}
                    y={ty + 3.5}
                    className={styles.axisLabel}
                    textAnchor="end"
                  >
                    {label}
                  </text>
                ) : null}
              </g>
            );
          })}

          {/* 时间轴：起点 / 中点 / 终点 */}
          {[model.minTime, (model.minTime + model.maxTime) / 2, model.maxTime].map(
            (t, index) => (
              <text
                key={t}
                x={model.x(t)}
                y={height - 7}
                className={styles.axisLabel}
                textAnchor={index === 0 ? "start" : index === 2 ? "end" : "middle"}
              >
                {formatTime(t, model.withDate)}
              </text>
            ),
          )}

          {(filled ?? model.paths.length === 1) &&
            model.paths.map((s) =>
              s.areaPath ? (
                <path
                  key={`area-${s.name}`}
                  d={s.areaPath}
                  fill={`url(#${gradientId.current})`}
                  stroke="none"
                />
              ) : null,
            )}

          {model.paths.map((s) =>
            s.segments.map((segment, index) => (
              <polyline
                key={`line-${s.name}-${index}`}
                points={segment}
                fill="none"
                stroke={s.color}
                strokeWidth="1.8"
                strokeLinejoin="round"
                strokeLinecap="round"
              />
            )),
          )}

          {bubble ? (
            <g>
              <line
                x1={bubble.x}
                y1={AXIS_TOP}
                x2={bubble.x}
                y2={plotBottom}
                className={styles.crosshair}
              />
              {model.paths.flatMap((s) => {
                const point = s.points.find((p) => p[0] === bubble.ts);
                return point ? (
                  <circle
                    key={`dot-${s.name}`}
                    cx={bubble.x}
                    cy={model.y(point[1])}
                    r="3.2"
                    fill={s.color}
                    stroke="var(--bg-elevated)"
                    strokeWidth="1.4"
                  />
                ) : null;
              })}
            </g>
          ) : null}

          {/* 命中层：整块绘图区捕获指针，避免只在线条上才响应 */}
          <rect
            x={AXIS_LEFT}
            y={AXIS_TOP}
            width={Math.max(model.plotWidth, 0)}
            height={Math.max(model.plotHeight, 0)}
            fill="transparent"
            onPointerMove={(event) => {
              const box = event.currentTarget.getBoundingClientRect();
              setHoverX(event.clientX - box.left + AXIS_LEFT);
            }}
            onPointerLeave={() => setHoverX(null)}
          />
        </svg>

        {bubble ? (
          <div
            className={styles.bubble}
            style={{ left: bubbleLeft, top: Math.max(bubble.y - 14, 2) }}
          >
            <div className={styles.bubbleTime}>{bubble.time}</div>
            {bubble.rows.map((row) => (
              <div key={row.name} className={styles.bubbleRow}>
                <span className={styles.legendDot} style={{ background: row.color }} />
                <span className={styles.bubbleName}>{row.name}</span>
                <span className={styles.bubbleValue}>{row.text}</span>
              </div>
            ))}
          </div>
        ) : null}
      </div>

      <div className={styles.legend}>
        {model.paths.map((s) => {
          const last = s.points[s.points.length - 1];
          return (
            <span key={s.name} className={styles.legendItem}>
              <span className={styles.legendDot} style={{ background: s.color }} />
              {s.name}
              {last ? (
                <strong className={styles.legendValue}>
                  {valueFormatter(last[1])}
                </strong>
              ) : null}
            </span>
          );
        })}
      </div>
    </div>
  );
}
