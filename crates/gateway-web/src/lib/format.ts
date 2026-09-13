/**
 * 指标页通用格式化与阈值判定。
 *
 * 约定：
 * - 缺失值统一渲染为 "—"，绝不用 0 或空白冒充实测数据；
 * - 未知量纲的数值一律不做猜测性换算。
 */

export const EMPTY = "—";

export function formatKiB(kb?: number | null): string {
  if (kb === undefined || kb === null || !Number.isFinite(kb)) return EMPTY;
  const bytes = kb * 1024;
  const units: [number, string][] = [
    [1024 ** 4, "TB"],
    [1024 ** 3, "GB"],
    [1024 ** 2, "MB"],
    [1024, "KB"],
  ];
  for (const [scale, unit] of units) {
    if (Math.abs(bytes) >= scale) {
      const value = bytes / scale;
      return `${value.toFixed(value >= 100 ? 0 : 1)} ${unit}`;
    }
  }
  return `${bytes.toFixed(0)} B`;
}

export function formatPercent(
  value?: number | null,
  digits = 1,
): string {
  if (value === undefined || value === null || !Number.isFinite(value)) {
    return EMPTY;
  }
  return `${value.toFixed(digits)}%`;
}

export function formatNumber(value?: number | null, digits = 2): string {
  if (value === undefined || value === null || !Number.isFinite(value)) {
    return EMPTY;
  }
  return value.toFixed(digits);
}

/** 秒 → 人类可读时长，保留两级单位。 */
export function formatUptime(seconds?: number | null): string {
  if (seconds === undefined || seconds === null || !Number.isFinite(seconds)) {
    return EMPTY;
  }
  const total = Math.max(0, Math.floor(seconds));
  const days = Math.floor(total / 86400);
  const hours = Math.floor((total % 86400) / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  if (days > 0) return `${days} 天 ${hours} 小时`;
  if (hours > 0) return `${hours} 小时 ${minutes} 分钟`;
  if (minutes > 0) return `${minutes} 分钟`;
  return `${total} 秒`;
}

/**
 * 秒 → 相对时间描述（"3 天前"）。
 * 用于上报延迟：比"5456 分钟"这种原始数字更好读。
 */
export function formatRelativeSeconds(seconds?: number | null): {
  text: string;
  seconds: number | null;
} {
  if (seconds === undefined || seconds === null || !Number.isFinite(seconds)) {
    return { text: EMPTY, seconds: null };
  }
  const s = Math.max(0, Math.floor(seconds));
  if (s < 60) return { text: `${s} 秒`, seconds: s };
  if (s < 3600) return { text: `${Math.floor(s / 60)} 分钟`, seconds: s };
  if (s < 86400) {
    const h = Math.floor(s / 3600);
    const m = Math.floor((s % 3600) / 60);
    return { text: m > 0 ? `${h} 小时 ${m} 分` : `${h} 小时`, seconds: s };
  }
  const d = Math.floor(s / 86400);
  const h = Math.floor((s % 86400) / 3600);
  return { text: h > 0 ? `${d} 天 ${h} 小时` : `${d} 天`, seconds: s };
}

export type Severity = "ok" | "warn" | "crit" | "unknown";

/** 百分比型指标（内存/磁盘使用率）的分级阈值。 */
export function severityForUsage(percent?: number | null): Severity {
  if (percent === undefined || percent === null || !Number.isFinite(percent)) {
    return "unknown";
  }
  if (percent >= 90) return "crit";
  if (percent >= 75) return "warn";
  return "ok";
}

/** 1 分钟负载分级：按逻辑核数归一（缺核数时以 8 核为保守基准）。 */
export function severityForLoad(load?: number | null, cores = 8): Severity {
  if (load === undefined || load === null || !Number.isFinite(load)) {
    return "unknown";
  }
  const ratio = load / Math.max(1, cores);
  if (ratio >= 1) return "crit";
  if (ratio >= 0.7) return "warn";
  return "ok";
}

export const SEVERITY_LABEL: Record<Severity, string> = {
  ok: "正常",
  warn: "偏高",
  crit: "危急",
  unknown: "无数据",
};

/** 序列最后一点的取值，用于卡片上展示"当前值"。 */
export function lastValue(
  points?: [number, number][] | null,
): number | undefined {
  if (!points || points.length === 0) return undefined;
  return points[points.length - 1][1];
}
