import styles from "./MetricTile.module.css";
import type { Severity } from "../lib/format";

type Tone = Severity | "accent";

interface MetricTileProps {
  label: string;
  value: string;
  hint?: string;
  tone?: Tone;
  loading?: boolean;
}

const TONE_CLASS: Record<Tone, string> = {
  accent: "toneAccent",
  ok: "toneOk",
  warn: "toneWarn",
  crit: "toneCrit",
  unknown: "toneUnknown",
};

/** 总览 KPI 磁贴：左侧色条表达状态，数值等宽，副文案补充判断依据。 */
export function MetricTile({
  label,
  value,
  hint,
  tone = "unknown",
  loading,
}: MetricTileProps) {
  return (
    <div
      className={`${styles.tile} ${styles[TONE_CLASS[tone]]}`}
      data-loading={loading ? "true" : undefined}
    >
      <span className={styles.label}>{label}</span>
      <span className={styles.value}>{loading ? "—" : value}</span>
      {hint ? <span className={styles.hint}>{hint}</span> : null}
    </div>
  );
}
