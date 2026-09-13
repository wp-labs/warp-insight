import styles from "./Sparkline.module.css";

interface SparklineProps {
  values: (number | undefined | null)[];
  width?: number;
  height?: number;
  color?: string;
  /** 每桶保留最大值做降采样，保留尖峰又不至于把折线糊成色块。 */
  maxBuckets?: number;
}

const VIEW_WIDTH = 160;

/**
 * 卡片内嵌迷你趋势线：零依赖、自适应宽度。
 * 数据点远多于像素宽度时按桶取最大值降采样——直接画 100 个点会让相邻
 * 折线互相覆盖，看上去像一块色块而不是趋势。
 */
export function Sparkline({
  values,
  height = 30,
  color = "var(--accent)",
  maxBuckets = 48,
}: SparklineProps) {
  const clean = values.map((value) =>
    value === undefined || value === null || !Number.isFinite(value)
      ? null
      : value,
  );
  const valid = clean.filter((value): value is number => value !== null);
  if (valid.length < 2) {
    return <div className={styles.empty} style={{ height }} aria-hidden="true" />;
  }

  const buckets: (number | null)[] = [];
  const bucketSize = Math.max(1, Math.ceil(clean.length / maxBuckets));
  for (let index = 0; index < clean.length; index += bucketSize) {
    const slice = clean.slice(index, index + bucketSize);
    const numbers = slice.filter((value): value is number => value !== null);
    buckets.push(numbers.length > 0 ? Math.max(...numbers) : null);
  }

  const min = Math.min(...valid);
  const max = Math.max(...valid);
  const span = max - min || Math.abs(max) * 0.05 || 1;
  const step = VIEW_WIDTH / Math.max(buckets.length - 1, 1);
  const y = (value: number) =>
    height - 2 - ((value - min) / span) * (height - 5);

  const segments: string[] = [];
  const areas: string[] = [];
  let current: string[] = [];
  let startIndex = 0;
  buckets.forEach((value, index) => {
    if (value === null) {
      if (current.length >= 2) {
        segments.push(current.join(" "));
        areas.push(
          `M ${(startIndex * step).toFixed(1)},${height} ` +
            current.map((point) => `L ${point}`).join(" ") +
            ` L ${((index - 1) * step).toFixed(1)},${height} Z`,
        );
      }
      current = [];
      startIndex = index + 1;
      return;
    }
    current.push(`${(index * step).toFixed(1)},${y(value).toFixed(1)}`);
  });
  if (current.length >= 2) {
    segments.push(current.join(" "));
    areas.push(
      `M ${(startIndex * step).toFixed(1)},${height} ` +
        current.map((point) => `L ${point}`).join(" ") +
        ` L ${((buckets.length - 1) * step).toFixed(1)},${height} Z`,
    );
  }

  const gradientId = `spark-${Math.random().toString(36).slice(2, 8)}`;

  return (
    <svg
      className={styles.svg}
      viewBox={`0 0 ${VIEW_WIDTH} ${height}`}
      height={height}
      preserveAspectRatio="none"
      aria-hidden="true"
      focusable="false"
    >
      <defs>
        <linearGradient id={gradientId} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor={color} stopOpacity="0.3" />
          <stop offset="100%" stopColor={color} stopOpacity="0" />
        </linearGradient>
      </defs>
      {areas.map((path, index) => (
        <path key={`area-${index}`} d={path} fill={`url(#${gradientId})`} />
      ))}
      {segments.map((points, index) => (
        <polyline
          key={`line-${index}`}
          points={points}
          fill="none"
          stroke={color}
          strokeWidth="1.4"
          strokeLinejoin="round"
          strokeLinecap="round"
          vectorEffect="non-scaling-stroke"
        />
      ))}
    </svg>
  );
}
