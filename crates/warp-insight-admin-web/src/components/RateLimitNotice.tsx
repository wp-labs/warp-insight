import { useEffect, useState } from "react";
import { isRateLimitedError } from "../api";
import styles from "./RateLimitNotice.module.css";

interface RateLimitNoticeProps {
  error: unknown;
}

function retryAfterSeconds(error: unknown): number | null {
  if (!isRateLimitedError(error)) return null;
  return error.retryAfterSeconds ?? 60;
}

/** Live countdown of the per-IP rate-limit block surfaced by HTTP 429. */
export function RateLimitNotice({ error }: RateLimitNoticeProps) {
  const seconds = retryAfterSeconds(error);
  const [remaining, setRemaining] = useState<number | null>(seconds);

  useEffect(() => {
    setRemaining(seconds);
    if (seconds === null || seconds <= 0) return;
    const timer = window.setInterval(() => {
      setRemaining((current) =>
        current === null || current <= 0 ? current : current - 1,
      );
    }, 1000);
    return () => window.clearInterval(timer);
  }, [seconds]);

  if (remaining === null) return null;

  return (
    <div className={styles.banner} role="alert">
      认证失败次数过多，当前 IP 已被临时封禁，请等待 {Math.max(remaining, 0)} 秒后重试。
    </div>
  );
}
