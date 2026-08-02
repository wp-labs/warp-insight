interface SparklineProps {
  values: (number | undefined)[];
  width?: number;
  height?: number;
  color?: string;
}

/** Minimal dependency-free line sparkline. Renders nothing with fewer than 2 points. */
export function Sparkline({
  values,
  width = 120,
  height = 28,
  color = "#1f3a5f",
}: SparklineProps) {
  const valid = values.filter((v): v is number => v !== undefined && v !== null);
  if (valid.length < 2) {
    return null;
  }
  const min = Math.min(...valid);
  const max = Math.max(...valid);
  const span = max - min || 1;
  const step = width / (valid.length - 1);
  const points = valid
    .map((value, index) => {
      const x = index * step;
      const y = height - ((value - min) / span) * (height - 2) - 1;
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    })
    .join(" ");
  return (
    <svg width={width} height={height} aria-hidden="true" focusable="false">
      <polyline
        points={points}
        fill="none"
        stroke={color}
        strokeWidth="1.5"
        strokeLinejoin="round"
        strokeLinecap="round"
      />
    </svg>
  );
}
