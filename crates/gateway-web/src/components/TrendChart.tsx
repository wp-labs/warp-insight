import styles from "./TrendChart.module.css";

interface TrendChartSeries {
  name: string;
  color: string;
  points: [number, number][];
}

interface TrendChartProps {
  series: TrendChartSeries[];
  height?: number;
  valueFormatter?: (value: number) => string;
}

function formatTime(timestampMs: number): string {
  return new Intl.DateTimeFormat("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(timestampMs));
}

/** 依赖为零的多序列折线图：SVG polyline + 图例 + 时间范围标注。 */
export function TrendChart({
  series,
  height = 160,
  valueFormatter,
}: TrendChartProps) {
  const width = 600;
  const paddingLeft = 6;
  const paddingRight = 6;
  const paddingTop = 8;
  const paddingBottom = 22;

  const allPoints = series.flatMap((s) => s.points);
  if (allPoints.length < 2) {
    return <div className={styles.empty}>暂无趋势数据</div>;
  }

  const values = allPoints.map((p) => p[1]);
  let minValue = Math.min(...values);
  let maxValue = Math.max(...values);
  if (minValue === maxValue) {
    minValue -= 1;
    maxValue += 1;
  }
  const minTime = Math.min(...allPoints.map((p) => p[0]));
  const maxTime = Math.max(...allPoints.map((p) => p[0]));
  const timeSpan = maxTime - minTime || 1;

  const plotWidth = width - paddingLeft - paddingRight;
  const plotHeight = height - paddingTop - paddingBottom;

  const x = (timestamp: number) =>
    paddingLeft + ((timestamp - minTime) / timeSpan) * plotWidth;
  const y = (value: number) =>
    paddingTop + ((maxValue - value) / (maxValue - minValue)) * plotHeight;

  return (
    <div className={styles.chart}>
      <svg
        viewBox={`0 0 ${width} ${height}`}
        role="img"
        aria-label="指标趋势"
        preserveAspectRatio="none"
        className={styles.svg}
      >
        {series.map((s) =>
          s.points.length >= 2 ? (
            <polyline
              key={s.name}
              points={s.points
                .map(([ts, value]) => `${x(ts).toFixed(1)},${y(value).toFixed(1)}`)
                .join(" ")}
              fill="none"
              stroke={s.color}
              strokeWidth="1.5"
              strokeLinejoin="round"
              strokeLinecap="round"
            />
          ) : null,
        )}
        <text
          x={paddingLeft}
          y={height - 6}
          className={styles.axisLabel}
          textAnchor="start"
        >
          {formatTime(minTime)}
        </text>
        <text
          x={width - paddingRight}
          y={height - 6}
          className={styles.axisLabel}
          textAnchor="end"
        >
          {formatTime(maxTime)}
        </text>
      </svg>
      <div className={styles.legend}>
        {series.map((s) => (
          <span key={s.name} className={styles.legendItem}>
            <span
              className={styles.legendDot}
              style={{ backgroundColor: s.color }}
            />
            {s.name}
            {valueFormatter && s.points.length > 0
              ? ` ${valueFormatter(s.points[s.points.length - 1][1])}`
              : ""}
          </span>
        ))}
      </div>
    </div>
  );
}
